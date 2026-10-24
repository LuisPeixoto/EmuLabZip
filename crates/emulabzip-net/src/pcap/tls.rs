//! What can be seen of an encrypted connection: the server name in the first message.

/// The name in the `server_name` extension of a TLS ClientHello at the start of `data`.
pub fn server_name(data: &[u8]) -> Option<String> {
    // Record: type 22 (handshake), version, length. Handshake: type 1 (ClientHello), length.
    if data.len() < 9 || data[0] != 0x16 || data[1] != 0x03 || data[5] != 0x01 {
        return None;
    }
    let mut r = Reader { data, at: 9 };
    r.skip(2 + 32)?; // version, random
    let session = r.u8()? as usize;
    r.skip(session)?;
    let suites = r.u16()? as usize;
    r.skip(suites)?;
    let compression = r.u8()? as usize;
    r.skip(compression)?;
    let extensions = r.u16()? as usize;
    let end = (r.at + extensions).min(data.len());
    while r.at + 4 <= end {
        let (kind, len) = (r.u16()?, r.u16()? as usize);
        if kind == 0 {
            // A list of names: length, then (type, length, name) entries.
            let mut list = Reader { data, at: r.at };
            list.skip(2)?;
            if list.u8()? == 0 {
                let n = list.u16()? as usize;
                return std::str::from_utf8(data.get(list.at..list.at + n)?).ok().map(str::to_string);
            }
            return None;
        }
        r.skip(len)?;
    }
    None
}

/// Whether `data` begins like a TLS record.
pub fn looks_like_tls(data: &[u8]) -> bool {
    data.len() >= 3 && matches!(data[0], 0x14..=0x17) && data[1] == 0x03 && data[2] <= 0x04
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn skip(&mut self, n: usize) -> Option<()> {
        (self.at + n <= self.data.len()).then(|| self.at += n)
    }

    fn u8(&mut self) -> Option<u8> {
        let v = *self.data.get(self.at)?;
        self.at += 1;
        Some(v)
    }

    fn u16(&mut self) -> Option<u16> {
        let v = u16::from_be_bytes([*self.data.get(self.at)?, *self.data.get(self.at + 1)?]);
        self.at += 2;
        Some(v)
    }
}

#[cfg(test)]
pub fn client_hello(name: &str) -> Vec<u8> {
    let mut sni = Vec::new();
    sni.extend_from_slice(&((name.len() + 3) as u16).to_be_bytes());
    sni.push(0);
    sni.extend_from_slice(&(name.len() as u16).to_be_bytes());
    sni.extend_from_slice(name.as_bytes());
    let mut extensions = vec![0x00, 0x0b, 0x00, 0x02, 0x01, 0x00]; // ec_point_formats, before the name
    extensions.extend_from_slice(&[0x00, 0x00]);
    extensions.extend_from_slice(&(sni.len() as u16).to_be_bytes());
    extensions.extend_from_slice(&sni);
    let mut hello = vec![0x03, 0x03];
    hello.extend_from_slice(&[7; 32]);
    hello.push(0);
    hello.extend_from_slice(&[0x00, 0x04, 0x13, 0x01, 0x13, 0x02]);
    hello.extend_from_slice(&[0x01, 0x00]);
    hello.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
    hello.extend_from_slice(&extensions);
    let mut handshake = vec![0x01, 0, 0, 0];
    let n = hello.len() as u32;
    handshake[1..4].copy_from_slice(&n.to_be_bytes()[1..]);
    handshake.extend_from_slice(&hello);
    let mut record = vec![0x16, 0x03, 0x01];
    record.extend_from_slice(&(handshake.len() as u16).to_be_bytes());
    record.extend_from_slice(&handshake);
    record
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_server_name() {
        assert_eq!(server_name(&client_hello("api.example.com")).as_deref(), Some("api.example.com"));
        assert_eq!(server_name(&client_hello("a.b")).as_deref(), Some("a.b"));
    }

    #[test]
    fn gives_up_quietly_on_anything_else() {
        assert_eq!(server_name(b"GET / HTTP/1.1\r\n\r\n"), None);
        assert_eq!(server_name(&[]), None);
        let hello = client_hello("example.com");
        for cut in [5, 9, 20, 60, hello.len() - 3] {
            assert_eq!(server_name(&hello[..cut]), None, "cut at {cut}");
        }
    }

    #[test]
    fn recognizes_tls_records() {
        assert!(looks_like_tls(&client_hello("x.y")));
        assert!(looks_like_tls(&[0x17, 0x03, 0x03, 0, 5]));
        assert!(!looks_like_tls(b"GET /") && !looks_like_tls(&[0x16, 0x03]));
    }
}
