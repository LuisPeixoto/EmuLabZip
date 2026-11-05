//! Writes small HPROF files, for tests of this crate and of the programs that use it.

/// A value of an instance field, in the order the class lists its fields.
#[derive(Debug, Clone, Copy)]
pub enum Value {
    Ref(u64),
    Bool(bool),
    Int(i32),
}

/// Builds a heap dump with 4-byte ids. Strings and classes are written first, then one heap segment.
pub struct Builder {
    strings: Vec<u8>,
    loads: Vec<u8>,
    sub: Vec<u8>,
    next_string: u64,
}

fn record(out: &mut Vec<u8>, tag: u8, body: &[u8]) {
    out.push(tag);
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(body);
}

impl Default for Builder {
    fn default() -> Self {
        Builder::new()
    }
}

impl Builder {
    pub fn new() -> Self {
        Builder { strings: Vec::new(), loads: Vec::new(), sub: Vec::new(), next_string: 0x1000 }
    }

    fn string(&mut self, text: &str) -> u64 {
        let id = self.next_string;
        self.next_string += 1;
        let mut body = (id as u32).to_be_bytes().to_vec();
        body.extend_from_slice(text.as_bytes());
        record(&mut self.strings, 0x01, &body);
        id
    }

    fn id(&mut self, v: u64) {
        self.sub.extend_from_slice(&(v as u32).to_be_bytes());
    }

    fn u4(&mut self, v: u32) {
        self.sub.extend_from_slice(&v.to_be_bytes());
    }

    /// Names the heap that the objects written next belong to (Android's `app`, `zygote`, `image`).
    pub fn heap(&mut self, heap_id: u32, name: &str) -> &mut Self {
        let s = self.string(name);
        self.sub.push(0xfe);
        self.u4(heap_id);
        self.id(s);
        self
    }

    /// A class with its own instance fields (`ty` 2 is an object, 4 a boolean, 10 an int) and static
    /// object references. The class is also made a garbage collector root when `sticky`.
    pub fn class(
        &mut self,
        id: u64,
        name: &str,
        super_id: u64,
        fields: &[(&str, u8)],
        statics: &[(&str, u64)],
        sticky: bool,
    ) -> &mut Self {
        let name_id = self.string(&name.replace('.', "/"));
        let mut load = Vec::new();
        load.extend_from_slice(&1u32.to_be_bytes());
        load.extend_from_slice(&(id as u32).to_be_bytes());
        load.extend_from_slice(&0u32.to_be_bytes());
        load.extend_from_slice(&(name_id as u32).to_be_bytes());
        record(&mut self.loads, 0x02, &load);

        let field_names: Vec<u64> = fields.iter().map(|f| self.string(f.0)).collect();
        let static_names: Vec<u64> = statics.iter().map(|s| self.string(s.0)).collect();
        let size: u32 = 8 + fields
            .iter()
            .map(|f| match f.1 {
                2 | 6 | 10 => 4,
                4 | 8 => 1,
                5 | 9 => 2,
                _ => 8,
            })
            .sum::<u32>();
        self.sub.push(0x20);
        self.id(id);
        self.u4(0);
        self.id(super_id);
        for _ in 0..5 {
            self.id(0);
        }
        self.u4(size);
        self.sub.extend_from_slice(&0u16.to_be_bytes());
        self.sub.extend_from_slice(&(statics.len() as u16).to_be_bytes());
        for (i, s) in statics.iter().enumerate() {
            self.id(static_names[i]);
            self.sub.push(2);
            self.id(s.1);
        }
        self.sub.extend_from_slice(&(fields.len() as u16).to_be_bytes());
        for (i, f) in fields.iter().enumerate() {
            self.id(field_names[i]);
            self.sub.push(f.1);
        }
        if sticky {
            self.root(0x05, id);
        }
        self
    }

    /// An instance. `values` are the fields of its class, then of its parent, and so on.
    pub fn instance(&mut self, id: u64, class: u64, values: &[Value]) -> &mut Self {
        let mut data = Vec::new();
        for v in values {
            match *v {
                Value::Ref(r) => data.extend_from_slice(&(r as u32).to_be_bytes()),
                Value::Bool(b) => data.push(u8::from(b)),
                Value::Int(i) => data.extend_from_slice(&i.to_be_bytes()),
            }
        }
        self.sub.push(0x21);
        self.id(id);
        self.u4(0);
        self.id(class);
        self.u4(data.len() as u32);
        self.sub.extend_from_slice(&data);
        self
    }

    pub fn object_array(&mut self, id: u64, class: u64, elements: &[u64]) -> &mut Self {
        self.sub.push(0x22);
        self.id(id);
        self.u4(0);
        self.u4(elements.len() as u32);
        self.id(class);
        for &e in elements {
            self.id(e);
        }
        self
    }

    pub fn primitive_array(&mut self, id: u64, ty: u8, length: u32) -> &mut Self {
        let size = match ty {
            4 | 8 => 1,
            5 | 9 => 2,
            6 | 10 => 4,
            _ => 8,
        };
        self.sub.push(0x23);
        self.id(id);
        self.u4(0);
        self.u4(length);
        self.sub.push(ty);
        self.sub.extend(std::iter::repeat_n(0u8, length as usize * size));
        self
    }

    /// A garbage collector root. `tag` is the HPROF sub-record: 0x05 sticky class, 0x03 Java frame,
    /// 0x01 JNI global, 0x08 thread object.
    pub fn root(&mut self, tag: u8, id: u64) -> &mut Self {
        self.sub.push(tag);
        self.id(id);
        match tag {
            0x01 => self.id(0),
            0x02 | 0x03 => {
                self.u4(0);
                self.u4(0);
            }
            0x04 | 0x06 => self.u4(0),
            0x08 => {
                self.u4(0);
                self.u4(0);
            }
            _ => {}
        }
        self
    }

    pub fn finish(&self) -> Vec<u8> {
        let mut out = b"JAVA PROFILE 1.0.3\0".to_vec();
        out.extend_from_slice(&4u32.to_be_bytes());
        out.extend_from_slice(&0u64.to_be_bytes());
        out.extend_from_slice(&self.strings);
        out.extend_from_slice(&self.loads);
        record(&mut out, 0x1c, &self.sub);
        record(&mut out, 0x2c, &[]);
        out
    }
}
