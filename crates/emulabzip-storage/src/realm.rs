//! A reader for Realm files, enough to list their classes, columns and rows.
//!
//! Realm has no public description of its file format, so this follows what the files of Realm Core 14 (file format 24)
//! look like on disk, written by the official `realm` package and read back here. A file is a tree of *arrays*: eight
//! bytes of header (`AAAA`, flags, a 24-bit count) and then the elements, packed to 0 to 64 bits or as fixed-size
//! slots, or as raw bytes. An element is a plain number, a number with its low bit set (a "tagged" number), or a
//! reference (an even offset into the file). The top reference in the file header leads to the names and the tables of
//! the classes; each table has a spec (column types, names and attributes) and a cluster tree holding its objects,
//! with one array per column in each cluster.
//!
//! What it reads: integers, booleans, strings (the three layouts Realm uses for them), binary data, dates, floats,
//! doubles, ObjectIds, links (as the key of the object they point at) and lists of any of those. What it does not:
//! sets, dictionaries, mixed values, decimals and UUIDs are named and left out. Anything it does not understand about a
//! file is an error, never a guess, and a damaged file cannot make it panic or read outside the file.

use emulabzip_core::{Error, Result};
use serde::Serialize;
use serde_json::{Value, json};

const HEADER: usize = 8;
const MAX_DEPTH: usize = 8;
const MAX_NODES: usize = 4_000_000;
/// The file format versions this reader has been checked against.
const FORMATS: std::ops::RangeInclusive<u8> = 20..=24;

fn bad(detail: impl Into<String>) -> Error {
    Error::parse("Realm file", detail.into())
}

/// Whether the bytes begin like a Realm file.
pub fn is_realm(bytes: &[u8]) -> bool {
    bytes.len() >= 24 && &bytes[16..20] == b"T-DB"
}

/// One array of the file.
#[derive(Clone, Copy)]
struct Node<'a> {
    data: &'a [u8],
    at: usize,
    size: usize,
    /// Bits (0 to 64) when `wtype` is 0; bytes per element when it is 1.
    width: usize,
    wtype: u8,
    has_refs: bool,
    inner: bool,
    ctx: bool,
}

fn node(data: &[u8], reference: u64) -> Result<Node<'_>> {
    let at = usize::try_from(reference).map_err(|_| bad("a reference is too big"))?;
    if at < 24 || at % 8 != 0 || at + HEADER > data.len() {
        return Err(bad(format!("a reference ({reference}) points outside the file")));
    }
    if &data[at..at + 4] != b"AAAA" {
        return Err(bad(format!("there is no array at {reference}")));
    }
    let flags = data[at + 4];
    let size = (usize::from(data[at + 5]) << 16) | (usize::from(data[at + 6]) << 8) | usize::from(data[at + 7]);
    let wtype = (flags >> 3) & 3;
    let w = flags & 7;
    let width_units = (1usize << w) >> 1;
    let n = Node {
        data,
        at,
        size,
        width: width_units,
        wtype,
        has_refs: flags & 0x40 != 0,
        inner: flags & 0x80 != 0,
        ctx: flags & 0x20 != 0,
    };
    let bytes = match wtype {
        0 => (size * width_units).div_ceil(8),
        1 => size * width_units,
        _ => size,
    };
    if at + HEADER + bytes > data.len() {
        return Err(bad(format!("the array at {reference} runs past the end of the file")));
    }
    Ok(n)
}

impl<'a> Node<'a> {
    fn body(&self) -> &'a [u8] {
        &self.data[self.at + HEADER..]
    }

    /// Element `i` of an array of numbers.
    fn int(&self, i: usize) -> i64 {
        if i >= self.size || self.wtype != 0 {
            return 0;
        }
        let b = self.body();
        match self.width {
            0 => 0,
            1 | 2 | 4 => {
                let bits = self.width;
                i64::from((b[(i * bits) / 8] >> ((i * bits) % 8)) & ((1u8 << bits) - 1))
            }
            8 => i64::from(b[i] as i8),
            16 => i64::from(i16::from_le_bytes([b[i * 2], b[i * 2 + 1]])),
            32 => i64::from(i32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().expect("4"))),
            _ => i64::from_le_bytes(b[i * 8..i * 8 + 8].try_into().expect("8")),
        }
    }

    fn ints(&self) -> Vec<i64> {
        (0..self.size).map(|i| self.int(i)).collect()
    }

    /// Element `i` as a reference. 0 is none; an odd number is a tagged number, not a reference.
    fn reference(&self, i: usize) -> u64 {
        let v = self.int(i);
        if v > 0 && v % 2 == 0 { v as u64 } else { 0 }
    }

    /// A fixed-size slot of an array stored with a width in bytes.
    fn slot(&self, i: usize) -> &'a [u8] {
        if i >= self.size || self.wtype != 1 {
            return &[];
        }
        &self.body()[i * self.width..(i + 1) * self.width]
    }

    /// The bytes of a blob.
    fn blob(&self) -> &'a [u8] {
        &self.body()[..self.size]
    }
}

fn child<'a>(data: &'a [u8], parent: Node<'a>, i: usize) -> Result<Option<Node<'a>>> {
    match parent.reference(i) {
        0 => Ok(None),
        r => Ok(Some(node(data, r)?)),
    }
}

/// What a column holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Int,
    Bool,
    String,
    Binary,
    Date,
    Float,
    Double,
    ObjectId,
    Link,
    LinkList,
    BackLink,
    Other(i64),
}

impl Kind {
    fn of(code: i64) -> Kind {
        match code {
            0 => Kind::Int,
            1 => Kind::Bool,
            2 => Kind::String,
            4 => Kind::Binary,
            8 => Kind::Date,
            9 => Kind::Float,
            10 => Kind::Double,
            12 => Kind::Link,
            13 => Kind::LinkList,
            14 => Kind::BackLink,
            15 => Kind::ObjectId,
            other => Kind::Other(other),
        }
    }

    fn name(self) -> String {
        match self {
            Kind::Int => "int".into(),
            Kind::Bool => "bool".into(),
            Kind::String => "string".into(),
            Kind::Binary => "binary".into(),
            Kind::Date => "date".into(),
            Kind::Float => "float".into(),
            Kind::Double => "double".into(),
            Kind::ObjectId => "objectId".into(),
            Kind::Link => "link".into(),
            Kind::LinkList => "linkList".into(),
            Kind::BackLink => "backlink".into(),
            Kind::Other(6) => "mixed".into(),
            Kind::Other(11) => "decimal".into(),
            Kind::Other(16) => "typedLink".into(),
            Kind::Other(17) => "uuid".into(),
            Kind::Other(n) => format!("type{n}"),
        }
    }

    fn supported(self) -> bool {
        !matches!(self, Kind::Other(_) | Kind::BackLink)
    }
}

const ATTR_NULLABLE: i64 = 16;
const ATTR_LIST: i64 = 32;
const ATTR_DICTIONARY: i64 = 64;
const ATTR_SET: i64 = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    pub name: String,
    /// `int`, `string`, `date`, `link`…
    pub kind: String,
    pub nullable: bool,
    /// `list`, `set` or `dictionary` for a collection.
    pub collection: Option<String>,
    /// The reader can show this column's values.
    pub supported: bool,
    /// Its place among the arrays of a cluster.
    #[serde(skip)]
    index: usize,
    #[serde(skip)]
    code: i64,
    #[serde(skip)]
    attrs: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    pub name: String,
    pub rows: u64,
    pub columns: Vec<Column>,
    #[serde(skip)]
    top: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Realm {
    /// The file format version of the file.
    pub format: u8,
    pub tables: Vec<Table>,
}

fn string_list(data: &[u8], reference: u64, nullable: bool) -> Result<Vec<Option<String>>> {
    let n = node(data, reference)?;
    let text = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    if !n.has_refs {
        // Short strings: a fixed slot each, the last byte of which says how much of the slot is unused.
        if n.wtype != 1 {
            return Err(bad("a string array has an unexpected layout"));
        }
        if n.width == 0 {
            return Ok(vec![if nullable { None } else { Some(String::new()) }; n.size]);
        }
        return Ok((0..n.size)
            .map(|i| {
                let slot = n.slot(i);
                let last = usize::from(*slot.last()?);
                (last < n.width).then(|| text(&slot[..n.width - 1 - last]))
            })
            .collect());
    }
    if !n.ctx {
        // Medium strings: end offsets, one blob with each string followed by a zero, and a list of which are null.
        let offsets = child(data, n, 0)?.ok_or_else(|| bad("a string array has no offsets"))?;
        let blob = child(data, n, 1)?;
        let nulls = child(data, n, 2)?;
        let blob = blob.map_or(&[][..], |b| b.blob());
        let mut start = 0usize;
        let mut out = Vec::with_capacity(offsets.size);
        for i in 0..offsets.size {
            let end = usize::try_from(offsets.int(i)).map_err(|_| bad("a string ends before it starts"))?;
            let is_null = nulls.is_some_and(|nl| nl.int(i) != 0);
            let bytes = blob.get(start..end).ok_or_else(|| bad("a string runs past its blob"))?;
            out.push((!is_null).then(|| text(bytes.strip_suffix(&[0]).unwrap_or(bytes))));
            start = end;
        }
        return Ok(out);
    }
    // Long strings: each has a blob of its own, and none at all for a null.
    (0..n.size)
        .map(|i| match child(data, n, i)? {
            None => Ok(None),
            Some(b) => {
                let bytes = b.blob();
                Ok(Some(text(bytes.strip_suffix(&[0]).unwrap_or(bytes))))
            }
        })
        .collect()
}

fn binary_list(data: &[u8], reference: u64) -> Result<Vec<Option<Vec<u8>>>> {
    let n = node(data, reference)?;
    if !n.has_refs {
        return Err(bad("a binary array has an unexpected layout"));
    }
    if !n.ctx {
        let offsets = child(data, n, 0)?.ok_or_else(|| bad("a binary array has no offsets"))?;
        let blob = child(data, n, 1)?.map_or(&[][..], |b| b.blob());
        let nulls = child(data, n, 2)?;
        let mut start = 0usize;
        let mut out = Vec::with_capacity(offsets.size);
        for i in 0..offsets.size {
            let end = usize::try_from(offsets.int(i)).map_err(|_| bad("a value ends before it starts"))?;
            let is_null = nulls.is_some_and(|nl| nl.int(i) != 0);
            let bytes = blob.get(start..end).ok_or_else(|| bad("a value runs past its blob"))?;
            out.push((!is_null).then(|| bytes.to_vec()));
            start = end;
        }
        return Ok(out);
    }
    (0..n.size).map(|i| Ok(child(data, n, i)?.map(|b| b.blob().to_vec()))).collect()
}

/// An array of numbers, with the first element taken as the marker for null when the column can be null.
fn int_list(data: &[u8], reference: u64, nullable: bool) -> Result<Vec<Option<i64>>> {
    let n = node(data, reference)?;
    let all = n.ints();
    if !nullable {
        return Ok(all.into_iter().map(Some).collect());
    }
    let Some((&marker, rest)) = all.split_first() else { return Ok(Vec::new()) };
    Ok(rest.iter().map(|v| (*v != marker).then_some(*v)).collect())
}

fn iso(seconds: i64, nanos: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let rem = seconds.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let frac = if nanos != 0 { format!(".{:09}", nanos).trim_end_matches('0').to_string() } else { String::new() };
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}{frac}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

fn float_value(bits: u64, wide: bool, nullable: bool) -> Value {
    let (f, null_bits) = if wide {
        (f64::from_bits(bits), 0x7FF8_0000_0000_00AAu64)
    } else {
        (f64::from(f32::from_bits(bits as u32)), 0x7FC0_00AAu64)
    };
    if nullable && bits == null_bits {
        return Value::Null;
    }
    if f.is_nan() {
        return json!("NaN");
    }
    if !wide {
        // A float shown with the digits that were written, not the ones a double adds.
        let text = format!("{}", f32::from_bits(bits as u32));
        return text.parse::<f64>().ok().and_then(serde_json::Number::from_f64).map_or(Value::Null, Value::Number);
    }
    serde_json::Number::from_f64(f).map_or_else(|| json!(if f > 0.0 { "Infinity" } else { "-Infinity" }), Value::Number)
}

/// The values of one column in one cluster, `rows` of them.
fn column_values(data: &[u8], reference: u64, col: &Column, rows: usize) -> Result<Vec<Value>> {
    let nullable = col.attrs & ATTR_NULLABLE != 0;
    let kind = Kind::of(col.code);
    let opt = |v: Option<Value>| v.unwrap_or(Value::Null);
    let values: Vec<Value> = match kind {
        Kind::Int => int_list(data, reference, nullable)?.into_iter().map(|v| opt(v.map(Value::from))).collect(),
        Kind::Bool => {
            int_list(data, reference, nullable)?.into_iter().map(|v| opt(v.map(|x| Value::Bool(x != 0)))).collect()
        }
        Kind::String => {
            string_list(data, reference, nullable)?.into_iter().map(|v| opt(v.map(Value::String))).collect()
        }
        Kind::Binary => binary_list(data, reference)?
            .into_iter()
            .map(|v| {
                opt(v.map(|b| {
                    let head: String = b.iter().take(16).map(|x| format!("{x:02x}")).collect();
                    json!(format!("<{} bytes: {head}{}>", b.len(), if b.len() > 16 { "…" } else { "" }))
                }))
            })
            .collect(),
        Kind::Date => {
            let n = node(data, reference)?;
            let secs = int_list(data, n.reference(0), true)?;
            let nanos = node(data, n.reference(1))?.ints();
            secs.into_iter()
                .enumerate()
                .map(|(i, s)| opt(s.map(|s| json!(iso(s, nanos.get(i).copied().unwrap_or(0))))))
                .collect()
        }
        Kind::Float | Kind::Double => {
            let n = node(data, reference)?;
            let wide = kind == Kind::Double;
            let width = if wide { 8 } else { 4 };
            if n.wtype != 1 || n.width != width {
                return Err(bad("a floating point column has an unexpected layout"));
            }
            (0..n.size)
                .map(|i| {
                    let s = n.slot(i);
                    let bits = if wide {
                        u64::from_le_bytes(s.try_into().expect("8"))
                    } else {
                        u64::from(u32::from_le_bytes(s.try_into().expect("4")))
                    };
                    float_value(bits, wide, nullable)
                })
                .collect()
        }
        Kind::ObjectId => {
            let n = node(data, reference)?;
            let bytes = n.blob_or_slots();
            let prefix = bytes.len().saturating_sub(12 * rows);
            if rows == 0 || bytes.len() < 12 * rows || prefix > 64 {
                return Err(bad("an ObjectId column has an unexpected layout"));
            }
            (0..rows)
                .map(|i| {
                    let is_null = nullable && prefix > 0 && bytes[i / 8] >> (i % 8) & 1 == 1;
                    if is_null {
                        Value::Null
                    } else {
                        json!(
                            bytes[prefix + i * 12..prefix + (i + 1) * 12]
                                .iter()
                                .map(|b| format!("{b:02x}"))
                                .collect::<String>()
                        )
                    }
                })
                .collect()
        }
        Kind::Link => int_list(data, reference, false)?
            .into_iter()
            .map(|v| match v {
                Some(k) if k > 0 => json!(format!("#{}", k - 1)),
                _ => Value::Null,
            })
            .collect(),
        Kind::LinkList => {
            let n = node(data, reference)?;
            (0..n.size)
                .map(|i| match child(data, n, i) {
                    Ok(Some(list)) => json!(format!("{} links", list.size)),
                    _ => json!("0 links"),
                })
                .collect()
        }
        _ => return Ok(vec![json!(format!("<{}>", kind.name())); rows]),
    };
    Ok(values)
}

impl Node<'_> {
    /// The raw bytes of an array stored as slots (width in bytes) or as a blob.
    fn blob_or_slots(&self) -> &[u8] {
        match self.wtype {
            1 => &self.body()[..self.size * self.width],
            2 => &self.body()[..self.size],
            _ => &[],
        }
    }
}

/// A list column: each row has a reference to the list, or none for an empty one.
fn list_values(data: &[u8], reference: u64, col: &Column, rows: usize) -> Result<Vec<Value>> {
    let n = node(data, reference)?;
    let element = Column { attrs: col.attrs & !(ATTR_LIST | ATTR_SET | ATTR_DICTIONARY), ..col.clone() };
    (0..rows)
        .map(|i| {
            let Some(leaf) = child(data, n, i)? else { return Ok(json!([])) };
            if leaf.inner {
                return Ok(json!("<a long list>"));
            }
            Ok(Value::Array(column_values(data, leaf_ref(&leaf), &element, leaf.size)?))
        })
        .collect()
}

fn leaf_ref(n: &Node<'_>) -> u64 {
    n.at as u64
}

/// The clusters of a table's tree, in order.
fn leaves(data: &[u8], reference: u64, depth: usize, budget: &mut usize) -> Result<Vec<u64>> {
    if depth > MAX_DEPTH {
        return Err(bad("the tree of a table is too deep"));
    }
    if *budget == 0 {
        return Err(bad("the file has too many arrays"));
    }
    *budget -= 1;
    let n = node(data, reference)?;
    if !n.inner {
        return Ok(vec![reference]);
    }
    let mut out = Vec::new();
    // An inner node holds, after its bookkeeping, the references of its children.
    for i in 3..n.size {
        let r = n.reference(i);
        if r != 0 {
            out.extend(leaves(data, r, depth + 1, budget)?);
        }
    }
    Ok(out)
}

fn cluster_rows(data: &[u8], reference: u64) -> Result<usize> {
    let n = node(data, reference)?;
    let first = n.int(0);
    if first & 1 == 1 {
        return Ok((first >> 1) as usize);
    }
    match child(data, n, 0)? {
        Some(keys) => Ok(keys.size),
        None => Ok(0),
    }
}

fn read_table(data: &[u8], name: &str, top: u64) -> Result<Table> {
    let t = node(data, top)?;
    let spec = child(data, t, 0)?.ok_or_else(|| bad(format!("`{name}` has no spec")))?;
    let types = child(data, spec, 0)?.ok_or_else(|| bad(format!("`{name}` has no column types")))?.ints();
    let names = match child(data, spec, 1)? {
        Some(_) => string_list(data, spec.reference(1), false)?,
        None => Vec::new(),
    };
    let attrs = child(data, spec, 2)?.map(|a| a.ints()).unwrap_or_default();
    let mut columns = Vec::new();
    let mut next_name = 0;
    for (index, code) in types.iter().enumerate() {
        let kind = Kind::of(*code);
        // Backlinks are in the types but have no name.
        let name = if kind == Kind::BackLink {
            continue;
        } else {
            let n = names.get(next_name).cloned().flatten().unwrap_or_else(|| format!("column{index}"));
            next_name += 1;
            n
        };
        let attr = attrs.get(index).copied().unwrap_or(0);
        let collection = if attr & ATTR_LIST != 0 {
            Some("list")
        } else if attr & ATTR_SET != 0 {
            Some("set")
        } else if attr & ATTR_DICTIONARY != 0 {
            Some("dictionary")
        } else {
            None
        };
        columns.push(Column {
            name,
            kind: kind.name(),
            nullable: attr & ATTR_NULLABLE != 0,
            collection: collection.map(str::to_string),
            supported: kind.supported() && matches!(collection, None | Some("list")),
            index,
            code: *code,
            attrs: attr,
        });
    }
    let mut rows = 0u64;
    if let Some(root) = child(data, t, 2)? {
        let mut budget = MAX_NODES;
        for leaf in leaves(data, root.at as u64, 0, &mut budget)? {
            rows += cluster_rows(data, leaf)? as u64;
        }
    }
    Ok(Table { name: name.to_string(), rows, columns, top })
}

/// Lists the classes of a Realm file, with their columns and how many objects each has.
pub fn read(data: &[u8]) -> Result<Realm> {
    if !is_realm(data) {
        return Err(bad("this is not a Realm file"));
    }
    let slot = usize::from(data[23] & 1);
    let format = data[20 + slot];
    if !FORMATS.contains(&format) {
        return Err(bad(format!(
            "this file is in format {format}, and this reader has been checked against formats {} to {} (Realm Core 12 to 14)",
            FORMATS.start(),
            FORMATS.end()
        )));
    }
    let top_ref = u64::from_le_bytes(data[slot * 8..slot * 8 + 8].try_into().expect("8"));
    let top = node(data, top_ref)?;
    let names = string_list(data, top.reference(0), false)?;
    let tables = child(data, top, 1)?.ok_or_else(|| bad("the file has no tables"))?;
    let mut out = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let Some(name) = name.as_deref().and_then(|n| n.strip_prefix("class_")) else { continue };
        let r = tables.reference(i);
        if r == 0 {
            continue;
        }
        out.push(read_table(data, name, r)?);
    }
    Ok(Realm { format, tables: out })
}

/// A page of a class's objects.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rows {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    pub truncated: bool,
}

/// Reads `limit` objects of `class`, starting at the `offset`th. Columns the reader cannot show are named in angle brackets.
pub fn rows(data: &[u8], class: &str, offset: u64, limit: u64) -> Result<Rows> {
    let realm = read(data)?;
    let table = realm.tables.iter().find(|t| t.name == class).ok_or_else(|| Error::invalid("class", class))?;
    let limit = limit.min(10_000);
    let mut budget = MAX_NODES;
    let root = child(data, node(data, table.top)?, 2)?;
    let leaves = match root {
        Some(r) => leaves(data, r.at as u64, 0, &mut budget)?,
        None => Vec::new(),
    };
    let mut rows: Vec<Vec<Value>> = Vec::new();
    let mut skipped = 0u64;
    let mut truncated = false;
    'leaves: for leaf in leaves {
        let n = node(data, leaf)?;
        let count = cluster_rows(data, leaf)?;
        if skipped + (count as u64) <= offset {
            skipped += count as u64;
            continue;
        }
        let mut columns: Vec<Vec<Value>> = Vec::new();
        for col in &table.columns {
            let r = n.reference(1 + col.index);
            let values = if r == 0 {
                vec![Value::Null; count]
            } else if col.collection.as_deref() == Some("list") {
                list_values(data, r, col, count)?
            } else if !col.supported {
                let label = col.collection.clone().unwrap_or_else(|| col.kind.clone());
                vec![json!(format!("<{label}>")); count]
            } else {
                column_values(data, r, col, count)?
            };
            columns.push(values);
        }
        for i in 0..count {
            if skipped < offset {
                skipped += 1;
                continue;
            }
            if rows.len() as u64 >= limit {
                truncated = true;
                break 'leaves;
            }
            rows.push(columns.iter().map(|c| c.get(i).cloned().unwrap_or(Value::Null)).collect());
        }
    }
    Ok(Rows { columns: table.columns.iter().map(|c| c.name.clone()).collect(), rows, truncated })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Written by the official `realm` package: `tools/fixtures/realm/make.mjs`.
    const FILE: &[u8] = include_bytes!("../tests/fixtures/default.realm");

    fn table<'a>(r: &'a Realm, name: &str) -> &'a Table {
        r.tables
            .iter()
            .find(|t| t.name == name)
            .unwrap_or_else(|| panic!("{name} is not in {:?}", r.tables.iter().map(|t| &t.name).collect::<Vec<_>>()))
    }

    #[test]
    fn lists_the_classes_their_columns_and_their_sizes() {
        let realm = read(FILE).unwrap();
        assert_eq!(realm.format, 24);
        let names: Vec<_> = realm.tables.iter().map(|t| (t.name.as_str(), t.rows)).collect();
        assert_eq!(names, [("Big", 700), ("Person", 3), ("Pet", 2)]);
        let person = table(&realm, "Person");
        let cols: Vec<_> = person
            .columns
            .iter()
            .map(|c| (c.name.as_str(), c.kind.as_str(), c.nullable, c.collection.as_deref()))
            .collect();
        assert_eq!(
            cols,
            [
                ("id", "int", false, None),
                ("name", "string", false, None),
                ("age", "int", true, None),
                ("score", "double", false, None),
                ("ratio", "float", false, None),
                ("active", "bool", false, None),
                ("born", "date", false, None),
                ("avatar", "binary", true, None),
                ("token", "objectId", false, None),
                ("note", "string", true, None),
                ("memo", "string", true, None),
                ("tags", "string", false, Some("list")),
                ("friend", "link", true, None),
            ]
        );
        assert!(person.columns.iter().all(|c| c.supported));
    }

    #[test]
    fn reads_every_kind_of_value_of_a_real_class() {
        let page = rows(FILE, "Person", 0, 10).unwrap();
        assert!(!page.truncated);
        assert_eq!(page.columns.len(), 13);
        let ana = &page.rows[0];
        assert_eq!(ana[..4], [json!(1), json!("Ana"), json!(31), json!(9.5)]);
        assert_eq!(ana[4], json!(0.25));
        assert_eq!(ana[5], json!(true));
        assert_eq!(ana[6], json!("1990-01-01T00:00:00Z"));
        assert_eq!(ana[7], json!("<4 bytes: 01020304>"));
        assert_eq!(ana[8], json!("65f0a1b2c3d4e5f607182930"));
        assert_eq!(
            ana[9],
            json!("first user with a longer note that does not fit in a short string slot of the array format")
        );
        assert_eq!(ana[10], json!("a memo of forty characters, more or less"));
        assert_eq!(ana[11], json!(["a", "bb"]));
        assert_eq!(ana[12], Value::Null);

        let bo = &page.rows[1];
        assert_eq!(bo[..4], [json!(2), json!("Bo"), Value::Null, json!(-1.5)]);
        assert_eq!(bo[4], json!(1.5));
        assert_eq!(bo[5], json!(false));
        assert_eq!(bo[6], json!("2001-09-09T01:46:40Z"));
        assert_eq!(bo[7], Value::Null);
        assert_eq!(bo[9], Value::Null);
        assert_eq!(bo[10], json!("short"));
        assert_eq!(bo[11], json!([]));
        assert_eq!(bo[12], json!("#0"), "Bo's friend is the first object");

        let cy = &page.rows[2];
        assert_eq!(cy[..4], [json!(3), json!("Cy"), json!(0), json!(0.0)]);
        assert_eq!(cy[6], json!("1969-12-31T23:59:59Z"), "before 1970");
        assert_eq!(cy[9], json!(""), "an empty string is not a null");
        assert_eq!(cy[10], Value::Null);
        assert_eq!(cy[11], json!(["x"]));
    }

    #[test]
    fn reads_a_second_class_with_links() {
        let page = rows(FILE, "Pet", 0, 10).unwrap();
        assert_eq!(page.columns, ["nickname", "legs", "owner"]);
        assert_eq!(
            page.rows,
            [vec![json!("Tom"), json!(4), json!("#0")], vec![json!("Tweety"), json!(2), Value::Null]]
        );
    }

    #[test]
    fn rows_spread_over_a_tree_of_clusters_read_in_order_and_page() {
        let page = rows(FILE, "Big", 0, 5).unwrap();
        assert!(page.truncated);
        assert_eq!(page.columns, ["n", "label", "wide"]);
        assert_eq!(
            page.rows[0],
            [json!(-5), json!("row 0"), Value::Null],
            "row 0 is a multiple of 7: its double is null"
        );
        assert_eq!(page.rows[1], [json!(-2), json!("row 1"), json!(0.25)]);
        // A page that starts inside the second cluster and runs into the third.
        let page = rows(FILE, "Big", 250, 10).unwrap();
        let ns: Vec<_> = page.rows.iter().map(|r| r[0].as_i64().unwrap()).collect();
        assert_eq!(ns, (250..260).map(|i| i * 3 - 5).collect::<Vec<_>>());
        assert_eq!(page.rows[0][1], json!("row 250"));
        let last = rows(FILE, "Big", 695, 100).unwrap();
        assert_eq!(last.rows.len(), 5);
        assert!(!last.truncated);
        assert_eq!(last.rows[4][1], json!("row 699"));
        assert_eq!(last.rows[4][2], json!(699.0 / 4.0));
        assert!(rows(FILE, "Big", 700, 10).unwrap().rows.is_empty());
        assert!(rows(FILE, "Nope", 0, 10).is_err());
    }

    #[test]
    fn what_is_not_a_realm_file_or_is_damaged_is_an_error_and_never_a_panic() {
        assert!(!is_realm(b"SQLite format 3\0"));
        assert!(read(b"nothing like it").is_err());
        // Every prefix of a real file, which cuts arrays and references short.
        for cut in (0..FILE.len()).step_by(97) {
            let _ = read(&FILE[..cut]);
            let _ = rows(&FILE[..cut], "Person", 0, 10);
        }
        // Single bytes changed all over a real file.
        for at in (0..FILE.len()).step_by(7) {
            let mut copy = FILE.to_vec();
            copy[at] ^= 0xff;
            let _ = read(&copy);
            let _ = rows(&copy, "Person", 0, 10);
            let _ = rows(&copy, "Big", 0, 10);
        }
        // A format this reader has not been checked against is said to be one.
        let mut other = FILE.to_vec();
        other[20] = 9;
        other[21] = 9;
        assert!(read(&other).unwrap_err().to_string().contains("format 9"));
    }

    #[test]
    fn dates_are_formatted_in_utc() {
        assert_eq!(iso(0, 0), "1970-01-01T00:00:00Z");
        assert_eq!(iso(631_152_000, 0), "1990-01-01T00:00:00Z");
        assert_eq!(iso(-1, 0), "1969-12-31T23:59:59Z");
        assert_eq!(iso(1_000_000_000, 500_000_000), "2001-09-09T01:46:40.5Z");
        assert_eq!(iso(951_782_400, 0), "2000-02-29T00:00:00Z");
    }
}
