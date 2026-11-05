//! Reading an HPROF heap dump.
//!
//! A dump is read in two passes over the bytes: the first finds the strings and the classes with their
//! fields, because a class can be written after its instances; the second reads every object and the
//! references it holds. Every length is checked against what is left, so a damaged or hostile file ends
//! in an error, never in a panic.

use std::collections::HashMap;
use std::fmt;

/// The biggest dump that is read.
pub const MAX_INPUT: usize = 1 << 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The file does not start like an HPROF file.
    NotHprof,
    /// The file ends in the middle of a record.
    Truncated,
    /// A value says something impossible, such as an id size of 3.
    Invalid(&'static str),
    TooLarge,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotHprof => write!(f, "this is not a Java heap dump (.hprof)"),
            Error::Truncated => write!(f, "the heap dump is cut short"),
            Error::Invalid(what) => write!(f, "the heap dump is damaged: {what}"),
            Error::TooLarge => write!(f, "the heap dump is larger than {} MB", MAX_INPUT / (1024 * 1024)),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// Why an object is kept alive by the virtual machine itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootKind {
    Unknown,
    JniGlobal,
    JniLocal,
    JavaFrame,
    NativeStack,
    StickyClass,
    ThreadBlock,
    MonitorUsed,
    ThreadObject,
    InternedString,
    Finalizing,
    Debugger,
    VmInternal,
    JniMonitor,
}

impl RootKind {
    pub fn label(self) -> &'static str {
        match self {
            RootKind::Unknown => "unknown root",
            RootKind::JniGlobal => "JNI global reference",
            RootKind::JniLocal => "JNI local reference",
            RootKind::JavaFrame => "Java stack frame",
            RootKind::NativeStack => "native stack",
            RootKind::StickyClass => "system class",
            RootKind::ThreadBlock => "thread block",
            RootKind::MonitorUsed => "monitor",
            RootKind::ThreadObject => "thread",
            RootKind::InternedString => "interned string",
            RootKind::Finalizing => "finalizer",
            RootKind::Debugger => "debugger",
            RootKind::VmInternal => "virtual machine",
            RootKind::JniMonitor => "JNI monitor",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Class,
    Instance,
    ObjectArray,
    PrimitiveArray,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    /// The HPROF type: 2 is an object, the others are primitives.
    pub ty: u8,
}

#[derive(Debug, Clone)]
pub struct Class {
    pub id: u64,
    pub name: String,
    pub super_id: u64,
    pub instance_size: u32,
    /// Instance fields declared by this class (not its parents).
    pub fields: Vec<Field>,
    /// Static fields that hold an object: name and the id of the object.
    pub static_refs: Vec<(String, u64)>,
}

#[derive(Debug, Clone, Copy)]
pub struct Object {
    pub id: u64,
    /// Index into [`Heap::classes`]; for primitive arrays, the class of that array type.
    pub class: u32,
    pub shallow: u32,
    pub heap: u8,
    pub kind: Kind,
    /// Where the record's body starts in the file, for reading its fields again.
    pub offset: u32,
}

/// A heap dump, read.
pub struct Heap<'a> {
    pub id_size: usize,
    pub classes: Vec<Class>,
    pub objects: Vec<Object>,
    /// Objects the references of each object point to, as indices into `objects`.
    pub edge_start: Vec<u32>,
    pub edges: Vec<u32>,
    pub roots: Vec<(u32, RootKind)>,
    /// Names of the heaps (`app`, `zygote`, `image`), by the id objects carry. Empty for a plain Java dump.
    pub heap_names: HashMap<u8, String>,
    /// The dump was cut short; what was read before is still used.
    pub truncated: bool,
    data: &'a [u8],
    class_index: HashMap<u64, u32>,
    object_index: HashMap<u64, u32>,
}

struct Cursor<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn left(&self) -> usize {
        self.data.len() - self.at
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if n > self.left() {
            return Err(Error::Truncated);
        }
        let s = &self.data[self.at..self.at + n];
        self.at += n;
        Ok(s)
    }
    fn skip(&mut self, n: usize) -> Result<()> {
        self.take(n).map(|_| ())
    }
    fn u1(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u2(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u4(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn uint(&mut self, size: usize) -> Result<u64> {
        let b = self.take(size)?;
        Ok(b.iter().fold(0u64, |a, &x| (a << 8) | u64::from(x)))
    }
}

/// The size in bytes of a value of an HPROF type.
pub fn type_size(ty: u8, id_size: usize) -> Option<usize> {
    Some(match ty {
        2 => id_size,
        4 | 8 => 1,
        5 | 9 => 2,
        6 | 10 => 4,
        7 | 11 => 8,
        _ => return None,
    })
}

fn primitive_name(ty: u8) -> &'static str {
    match ty {
        4 => "boolean[]",
        5 => "char[]",
        6 => "float[]",
        7 => "double[]",
        8 => "byte[]",
        9 => "short[]",
        10 => "int[]",
        11 => "long[]",
        _ => "unknown[]",
    }
}

/// `java/lang/String` and `[Ljava/lang/String;` as people write them.
pub fn readable_class(raw: &str) -> String {
    let mut name = raw.replace('/', ".");
    let mut dims = 0;
    while let Some(rest) = name.strip_prefix('[') {
        dims += 1;
        name = rest.to_string();
    }
    if dims > 0 {
        name = match name.as_str() {
            "Z" => "boolean".into(),
            "C" => "char".into(),
            "F" => "float".into(),
            "D" => "double".into(),
            "B" => "byte".into(),
            "S" => "short".into(),
            "I" => "int".into(),
            "J" => "long".into(),
            other => other.strip_prefix('L').and_then(|o| o.strip_suffix(';')).unwrap_or(other).to_string(),
        };
        name.push_str(&"[]".repeat(dims));
    }
    name
}

struct RawClass {
    id: u64,
    super_id: u64,
    instance_size: u32,
    fields: Vec<(u64, u8)>,
    statics: Vec<(u64, u8, u64)>,
}

impl<'a> Heap<'a> {
    pub fn read(data: &'a [u8]) -> Result<Heap<'a>> {
        if data.len() > MAX_INPUT {
            return Err(Error::TooLarge);
        }
        let nul = data.iter().take(40).position(|&b| b == 0).ok_or(Error::NotHprof)?;
        if !data[..nul].starts_with(b"JAVA PROFILE 1.0") {
            return Err(Error::NotHprof);
        }
        let mut c = Cursor { data, at: nul + 1 };
        let id_size = c.u4()? as usize;
        if !matches!(id_size, 4 | 8) {
            return Err(Error::Invalid("the id size is not 4 or 8"));
        }
        c.skip(8)?; // the time stamp
        let body = c.at;

        let mut heap = Heap {
            id_size,
            classes: Vec::new(),
            objects: Vec::new(),
            edge_start: Vec::new(),
            edges: Vec::new(),
            roots: Vec::new(),
            heap_names: HashMap::new(),
            truncated: false,
            data,
            class_index: HashMap::new(),
            object_index: HashMap::new(),
        };
        let names = heap.first_pass(body)?;
        heap.second_pass(body, &names)?;
        Ok(heap)
    }

    /// Strings, loaded classes and class layouts.
    fn first_pass(&mut self, from: usize) -> Result<HashMap<u64, String>> {
        let id = self.id_size;
        let mut strings: HashMap<u64, String> = HashMap::new();
        let mut load: HashMap<u64, u64> = HashMap::new(); // class id to name string id
        let mut raw: Vec<RawClass> = Vec::new();
        let mut c = Cursor { data: self.data, at: from };
        while c.left() >= 9 {
            let (tag, _time, len) = (c.u1()?, c.u4()?, c.u4()? as usize);
            let end = c.at.checked_add(len).filter(|&e| e <= self.data.len());
            let Some(end) = end else {
                self.truncated = true;
                break;
            };
            let mut r = Cursor { data: &self.data[..end], at: c.at };
            match tag {
                0x01 => {
                    let sid = r.uint(id)?;
                    let text = r.take(r.left())?;
                    strings.insert(sid, String::from_utf8_lossy(text).into_owned());
                }
                0x02 => {
                    r.skip(4)?;
                    let class = r.uint(id)?;
                    r.skip(4)?;
                    load.insert(class, r.uint(id)?);
                }
                0x0c | 0x1c => {
                    self.truncated |= self.scan_segment(&mut r, &mut raw).is_err();
                }
                _ => {}
            }
            c.at = end;
        }

        // Class names and field names are known now.
        let name_of = |sid: u64| strings.get(&sid).cloned().unwrap_or_default();
        for rc in raw {
            let name =
                load.get(&rc.id).map(|&s| readable_class(&name_of(s))).unwrap_or_else(|| format!("class@{:x}", rc.id));
            let class = Class {
                id: rc.id,
                name,
                super_id: rc.super_id,
                instance_size: rc.instance_size,
                fields: rc.fields.iter().map(|&(n, ty)| Field { name: name_of(n), ty }).collect(),
                static_refs: rc.statics.iter().filter(|s| s.1 == 2).map(|&(n, _, v)| (name_of(n), v)).collect(),
            };
            self.class_index.insert(class.id, self.classes.len() as u32);
            self.classes.push(class);
        }
        // One class per primitive array type, so those arrays have a class to be counted under.
        for ty in [4u8, 5, 6, 7, 8, 9, 10, 11] {
            let id = u64::MAX - u64::from(ty);
            self.class_index.insert(id, self.classes.len() as u32);
            self.classes.push(Class {
                id,
                name: primitive_name(ty).to_string(),
                super_id: 0,
                instance_size: 0,
                fields: Vec::new(),
                static_refs: Vec::new(),
            });
        }
        Ok(strings)
    }

    /// Reads the class dumps of one heap segment, and skips the rest of it.
    fn scan_segment(&self, r: &mut Cursor, raw: &mut Vec<RawClass>) -> Result<()> {
        let id = self.id_size;
        while r.left() > 0 {
            let tag = r.u1()?;
            if tag == 0x20 {
                let class = r.uint(id)?;
                r.skip(4)?;
                let super_id = r.uint(id)?;
                r.skip(id * 5)?; // loader, signers, domain, two reserved
                let instance_size = r.u4()?;
                for _ in 0..r.u2()? {
                    r.skip(2)?;
                    let ty = r.u1()?;
                    r.skip(type_size(ty, id).ok_or(Error::Invalid("a constant has no type"))?)?;
                }
                let mut statics = Vec::new();
                for _ in 0..r.u2()? {
                    let name = r.uint(id)?;
                    let ty = r.u1()?;
                    let size = type_size(ty, id).ok_or(Error::Invalid("a static field has no type"))?;
                    statics.push((name, ty, r.uint(size.min(8))?));
                    r.skip(size.saturating_sub(8))?;
                }
                let mut fields = Vec::new();
                for _ in 0..r.u2()? {
                    fields.push((r.uint(id)?, r.u1()?));
                }
                raw.push(RawClass { id: class, super_id, instance_size, fields, statics });
            } else {
                self.skip_subrecord(tag, r)?;
            }
        }
        Ok(())
    }

    /// Skips one heap sub-record that is not a class dump.
    fn skip_subrecord(&self, tag: u8, r: &mut Cursor) -> Result<()> {
        let id = self.id_size;
        match tag {
            0xff | 0x05 | 0x07 | 0x89 | 0x8a | 0x8b | 0x8d | 0x90 => r.skip(id),
            0x01 => r.skip(id * 2),
            0x02 | 0x03 | 0x8e => r.skip(id + 8),
            0x04 | 0x06 => r.skip(id + 4),
            0x08 => r.skip(id + 8),
            0xfe => r.skip(4 + id),
            0x21 => {
                r.skip(id + 4 + id)?;
                let n = r.u4()? as usize;
                r.skip(n)
            }
            0x22 => {
                r.skip(id + 4)?;
                let n = r.u4()? as usize;
                r.skip(id)?;
                r.skip(n.checked_mul(id).ok_or(Error::Invalid("an array is too long"))?)
            }
            0x23 => {
                r.skip(id + 4)?;
                let n = r.u4()? as usize;
                let ty = r.u1()?;
                r.skip(
                    n.checked_mul(type_size(ty, id).ok_or(Error::Invalid("an array has no type"))?)
                        .ok_or(Error::Invalid("an array is too long"))?,
                )
            }
            0xc3 => r.skip(id + 4 + 4 + 1),
            _ => Err(Error::Invalid("an unknown heap record")),
        }
    }

    /// Objects, roots and references.
    fn second_pass(&mut self, from: usize, names: &HashMap<u64, String>) -> Result<()> {
        let mut pending: Vec<u64> = Vec::new(); // reference ids, in object order
        let mut starts: Vec<u32> = Vec::new();
        let mut root_ids: Vec<(u64, RootKind)> = Vec::new();
        let mut heap_id: u8 = 0;
        let mut c = Cursor { data: self.data, at: from };

        // The class objects come first, with the references of their static fields.
        for i in 0..self.classes.len() {
            let cl = &self.classes[i];
            if cl.id > u64::MAX - 16 {
                continue; // the stand-ins for primitive array classes are not objects
            }
            let idx = self.objects.len() as u32;
            self.object_index.insert(cl.id, idx);
            starts.push(pending.len() as u32);
            pending.extend(cl.static_refs.iter().map(|s| s.1));
            self.objects.push(Object { id: cl.id, class: i as u32, shallow: 0, heap: 0, kind: Kind::Class, offset: 0 });
        }

        while c.left() >= 9 {
            let (tag, _time, len) = (c.u1()?, c.u4()?, c.u4()? as usize);
            let Some(end) = c.at.checked_add(len).filter(|&e| e <= self.data.len()) else {
                self.truncated = true;
                break;
            };
            if tag == 0x0c || tag == 0x1c {
                let mut r = Cursor { data: &self.data[..end], at: c.at };
                if self.read_segment(&mut r, &mut pending, &mut starts, &mut root_ids, &mut heap_id).is_err() {
                    self.truncated = true;
                }
            }
            c.at = end;
        }

        // References are ids until every object is known; then they become indices.
        starts.push(pending.len() as u32);
        let n = self.objects.len();
        let mut edge_start = Vec::with_capacity(n + 1);
        let mut edges = Vec::with_capacity(pending.len());
        for w in starts.windows(2) {
            edge_start.push(edges.len() as u32);
            for &target in &pending[w[0] as usize..w[1] as usize] {
                if let Some(&t) = self.object_index.get(&target) {
                    edges.push(t);
                }
            }
        }
        edge_start.push(edges.len() as u32);
        self.edge_start = edge_start;
        self.edges = edges;
        for (rid, kind) in root_ids {
            if let Some(&i) = self.object_index.get(&rid) {
                self.roots.push((i, kind));
            }
        }
        // The heap names were string ids until now.
        for name in self.heap_names.values_mut() {
            if let Some(sid) = name.strip_prefix('#').and_then(|v| v.parse::<u64>().ok()) {
                *name = names.get(&sid).cloned().unwrap_or_default();
            }
        }
        Ok(())
    }

    fn read_segment(
        &mut self,
        r: &mut Cursor,
        pending: &mut Vec<u64>,
        starts: &mut Vec<u32>,
        roots: &mut Vec<(u64, RootKind)>,
        heap: &mut u8,
    ) -> Result<()> {
        let id = self.id_size;
        while r.left() > 0 {
            let tag = r.u1()?;
            let root = match tag {
                0xff => Some(RootKind::Unknown),
                0x01 => Some(RootKind::JniGlobal),
                0x02 => Some(RootKind::JniLocal),
                0x03 => Some(RootKind::JavaFrame),
                0x04 => Some(RootKind::NativeStack),
                0x05 => Some(RootKind::StickyClass),
                0x06 => Some(RootKind::ThreadBlock),
                0x07 => Some(RootKind::MonitorUsed),
                0x08 => Some(RootKind::ThreadObject),
                0x89 => Some(RootKind::InternedString),
                0x8a => Some(RootKind::Finalizing),
                0x8b => Some(RootKind::Debugger),
                0x8d => Some(RootKind::VmInternal),
                0x8e => Some(RootKind::JniMonitor),
                _ => None,
            };
            if let Some(kind) = root {
                let obj = r.uint(id)?;
                roots.push((obj, kind));
                self.skip_subrecord_rest(tag, r)?;
                continue;
            }
            match tag {
                0x20 => {
                    // Class dumps were read by the first pass.
                    r.skip(id + 4 + id + id * 5)?;
                    let _size = r.u4()?;
                    for _ in 0..r.u2()? {
                        r.skip(2)?;
                        let ty = r.u1()?;
                        r.skip(type_size(ty, id).ok_or(Error::Invalid("a constant has no type"))?)?;
                    }
                    for _ in 0..r.u2()? {
                        r.skip(id)?;
                        let ty = r.u1()?;
                        r.skip(type_size(ty, id).ok_or(Error::Invalid("a static field has no type"))?)?;
                    }
                    let n = r.u2()? as usize;
                    r.skip(n * (id + 1))?;
                }
                0x21 => {
                    let obj = r.uint(id)?;
                    r.skip(4)?;
                    let class_id = r.uint(id)?;
                    let n = r.u4()? as usize;
                    let offset = r.at;
                    let data = r.take(n)?;
                    let class =
                        *self.class_index.get(&class_id).ok_or(Error::Invalid("an instance of an unknown class"))?;
                    let shallow = self.classes[class as usize].instance_size;
                    let idx = self.objects.len() as u32;
                    self.object_index.insert(obj, idx);
                    starts.push(pending.len() as u32);
                    self.instance_refs(class, data, pending);
                    self.objects.push(Object {
                        id: obj,
                        class,
                        shallow,
                        heap: *heap,
                        kind: Kind::Instance,
                        offset: offset as u32,
                    });
                }
                0x22 => {
                    let obj = r.uint(id)?;
                    r.skip(4)?;
                    let n = r.u4()? as usize;
                    let class_id = r.uint(id)?;
                    let bytes = n.checked_mul(id).ok_or(Error::Invalid("an array is too long"))?;
                    let offset = r.at;
                    let data = r.take(bytes)?;
                    let class = self.class_index.get(&class_id).copied().unwrap_or(0);
                    let idx = self.objects.len() as u32;
                    self.object_index.insert(obj, idx);
                    starts.push(pending.len() as u32);
                    pending.extend(
                        data.chunks_exact(id)
                            .map(|b| b.iter().fold(0u64, |a, &x| (a << 8) | u64::from(x)))
                            .filter(|&v| v != 0),
                    );
                    self.objects.push(Object {
                        id: obj,
                        class,
                        shallow: (16 + bytes) as u32,
                        heap: *heap,
                        kind: Kind::ObjectArray,
                        offset: offset as u32,
                    });
                }
                0x23 => {
                    let obj = r.uint(id)?;
                    r.skip(4)?;
                    let n = r.u4()? as usize;
                    let ty = r.u1()?;
                    let bytes = n
                        .checked_mul(type_size(ty, id).ok_or(Error::Invalid("an array has no type"))?)
                        .ok_or(Error::Invalid("an array is too long"))?;
                    r.skip(bytes)?;
                    self.add_primitive_array(obj, ty, bytes, *heap, starts, pending.len());
                }
                0xc3 => {
                    let obj = r.uint(id)?;
                    r.skip(4)?;
                    let n = r.u4()? as usize;
                    let ty = r.u1()?;
                    let bytes = n.saturating_mul(type_size(ty, id).unwrap_or(1));
                    self.add_primitive_array(obj, ty, bytes, *heap, starts, pending.len());
                }
                0xfe => {
                    *heap = r.u4()? as u8;
                    let name = r.uint(id)?;
                    // The name is a string id; it is resolved by the caller through `heap_names`.
                    self.heap_names.insert(*heap, format!("#{name}"));
                }
                other => self.skip_subrecord(other, r)?,
            }
        }
        Ok(())
    }

    fn add_primitive_array(
        &mut self,
        obj: u64,
        ty: u8,
        bytes: usize,
        heap: u8,
        starts: &mut Vec<u32>,
        pending_len: usize,
    ) {
        let class = self.class_index.get(&(u64::MAX - u64::from(ty))).copied().unwrap_or(0);
        let idx = self.objects.len() as u32;
        self.object_index.insert(obj, idx);
        starts.push(pending_len as u32);
        self.objects.push(Object {
            id: obj,
            class,
            shallow: (16 + bytes) as u32,
            heap,
            kind: Kind::PrimitiveArray,
            offset: 0,
        });
    }

    /// What is left of a root sub-record after its first id.
    fn skip_subrecord_rest(&self, tag: u8, r: &mut Cursor) -> Result<()> {
        let id = self.id_size;
        match tag {
            0x01 => r.skip(id),
            0x02 | 0x03 | 0x8e => r.skip(8),
            0x04 | 0x06 => r.skip(4),
            0x08 => r.skip(8),
            _ => Ok(()),
        }
    }

    /// The class and its parents, nearest first.
    pub fn chain(&self, class: u32) -> Vec<u32> {
        let mut out = vec![class];
        let mut cur = &self.classes[class as usize];
        while cur.super_id != 0 && out.len() < 64 {
            match self.class_index.get(&cur.super_id) {
                Some(&i) if !out.contains(&i) => {
                    out.push(i);
                    cur = &self.classes[i as usize];
                }
                _ => break,
            }
        }
        out
    }

    /// Reference ids held by an instance's fields. The `referent` of a `java.lang.ref.Reference` is
    /// left out: weak, soft and phantom references do not keep their target alive.
    fn instance_refs(&self, class: u32, data: &[u8], out: &mut Vec<u64>) {
        let id = self.id_size;
        let mut at = 0usize;
        for ci in self.chain(class) {
            let cl = &self.classes[ci as usize];
            for f in &cl.fields {
                let Some(size) = type_size(f.ty, id) else { return };
                if at + size > data.len() {
                    return;
                }
                if f.ty == 2 {
                    let v = data[at..at + size].iter().fold(0u64, |a, &x| (a << 8) | u64::from(x));
                    let weak = f.name == "referent" && cl.name == "java.lang.ref.Reference";
                    if v != 0 && !weak {
                        out.push(v);
                    }
                }
                at += size;
            }
        }
    }

    /// The fields of an instance, with their raw values, from the class up its parents.
    pub fn fields_of(&self, obj: u32) -> Vec<(&Class, &Field, u64)> {
        let o = &self.objects[obj as usize];
        if o.kind != Kind::Instance {
            return Vec::new();
        }
        let id = self.id_size;
        let mut at = o.offset as usize;
        let mut out = Vec::new();
        for ci in self.chain(o.class) {
            let cl = &self.classes[ci as usize];
            for f in &cl.fields {
                let Some(size) = type_size(f.ty, id) else { return out };
                let Some(bytes) = self.data.get(at..at + size) else { return out };
                out.push((cl, f, bytes.iter().take(8).fold(0u64, |a, &x| (a << 8) | u64::from(x))));
                at += size;
            }
        }
        out
    }

    pub fn class_of(&self, obj: u32) -> &Class {
        &self.classes[self.objects[obj as usize].class as usize]
    }

    /// Whether an object's class is `name` or has it as a parent.
    pub fn is_a(&self, obj: u32, name: &str) -> bool {
        self.chain(self.objects[obj as usize].class).iter().any(|&c| self.classes[c as usize].name == name)
    }

    pub fn object_index_of(&self, id: u64) -> Option<u32> {
        self.object_index.get(&id).copied()
    }

    /// The name of the heap an object is in (`app`, `zygote`, `image`), when the dump says.
    pub fn heap_of(&self, obj: u32) -> Option<&str> {
        self.heap_names.get(&self.objects[obj as usize].heap).map(String::as_str)
    }

    pub fn successors(&self, obj: u32) -> &[u32] {
        &self.edges[self.edge_start[obj as usize] as usize..self.edge_start[obj as usize + 1] as usize]
    }
}
