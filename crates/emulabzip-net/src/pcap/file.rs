//! Reading the two capture file formats: classic `.pcap` and `.pcapng`.

use emulabzip_core::{Error, Result};

/// One captured packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    /// Microseconds since the Unix epoch.
    pub ts_us: u64,
    /// The link type of the interface it came from (1 is Ethernet).
    pub link: u32,
    pub data: Vec<u8>,
}

#[derive(Clone, Copy)]
struct Order(bool);

impl Order {
    fn u16(self, b: &[u8]) -> u16 {
        let a = [b[0], b[1]];
        if self.0 { u16::from_be_bytes(a) } else { u16::from_le_bytes(a) }
    }

    fn u32(self, b: &[u8]) -> u32 {
        let a = [b[0], b[1], b[2], b[3]];
        if self.0 { u32::from_be_bytes(a) } else { u32::from_le_bytes(a) }
    }
}

fn bad(why: &str) -> Error {
    Error::parse("capture file", why)
}

/// Reads every packet of a `.pcap` or `.pcapng` file.
pub fn read(bytes: &[u8]) -> Result<Vec<Packet>> {
    match bytes.get(..4) {
        Some([0x0a, 0x0d, 0x0d, 0x0a]) => read_pcapng(bytes),
        Some(
            [0xd4, 0xc3, 0xb2, 0xa1] | [0xa1, 0xb2, 0xc3, 0xd4] | [0x4d, 0x3c, 0xb2, 0xa1] | [0xa1, 0xb2, 0x3c, 0x4d],
        ) => read_pcap(bytes),
        Some(_) => Err(bad("this is not a pcap or pcapng file")),
        None => Err(bad("the file is empty")),
    }
}

fn read_pcap(bytes: &[u8]) -> Result<Vec<Packet>> {
    if bytes.len() < 24 {
        return Err(bad("the header is cut short"));
    }
    let (order, nanos) = match [bytes[0], bytes[1], bytes[2], bytes[3]] {
        [0xd4, 0xc3, 0xb2, 0xa1] => (Order(false), false),
        [0xa1, 0xb2, 0xc3, 0xd4] => (Order(true), false),
        [0x4d, 0x3c, 0xb2, 0xa1] => (Order(false), true),
        _ => (Order(true), true),
    };
    let link = order.u32(&bytes[20..24]);
    let mut packets = Vec::new();
    let mut at = 24;
    while at + 16 <= bytes.len() {
        let (secs, frac, caplen) =
            (order.u32(&bytes[at..]), order.u32(&bytes[at + 4..]), order.u32(&bytes[at + 8..]) as usize);
        at += 16;
        // A cut-off last record is dropped; everything before it is good.
        let Some(data) = bytes.get(at..at + caplen) else { break };
        at += caplen;
        let micros = if nanos { u64::from(frac) / 1000 } else { u64::from(frac) };
        packets.push(Packet { ts_us: u64::from(secs) * 1_000_000 + micros, link, data: data.to_vec() });
    }
    Ok(packets)
}

struct Interface {
    link: u32,
    /// Ticks per second of this interface's timestamps.
    ticks: f64,
}

fn read_pcapng(bytes: &[u8]) -> Result<Vec<Packet>> {
    let mut order = Order(false);
    let mut interfaces: Vec<Interface> = Vec::new();
    let mut packets = Vec::new();
    let mut at = 0;
    while at + 12 <= bytes.len() {
        // The byte order is only known once the section header has been read, and that block's type
        // reads the same either way.
        let kind = &bytes[at..at + 4];
        if kind == [0x0a, 0x0d, 0x0d, 0x0a] {
            let magic = bytes.get(at + 8..at + 12).ok_or_else(|| bad("the section header is cut short"))?;
            order = match magic {
                [0x4d, 0x3c, 0x2b, 0x1a] => Order(false),
                [0x1a, 0x2b, 0x3c, 0x4d] => Order(true),
                _ => return Err(bad("the byte order of the section is unknown")),
            };
            interfaces.clear();
        }
        let (kind, len) = (order.u32(kind), order.u32(&bytes[at + 4..]) as usize);
        if len < 12 || len % 4 != 0 {
            return Err(bad("a block has an impossible length"));
        }
        let Some(block) = bytes.get(at..at + len) else { break };
        let body = &block[8..len - 4];
        match kind {
            // Interface description.
            1 if body.len() >= 8 => {
                let mut ticks = 1e6;
                let mut o = 8;
                while o + 4 <= body.len() {
                    let (code, olen) = (order.u16(&body[o..]), order.u16(&body[o + 2..]) as usize);
                    if code == 0 {
                        break;
                    }
                    if code == 9 && olen >= 1 && o + 4 < body.len() {
                        let r = body[o + 4];
                        ticks = if r & 0x80 == 0 { 10f64.powi(i32::from(r)) } else { 2f64.powi(i32::from(r & 0x7f)) };
                    }
                    o += 4 + olen.div_ceil(4) * 4;
                }
                interfaces.push(Interface { link: u32::from(order.u16(body)), ticks });
            }
            // Enhanced packet.
            6 if body.len() >= 20 => {
                let (id, hi, lo, caplen) = (
                    order.u32(body) as usize,
                    order.u32(&body[4..]),
                    order.u32(&body[8..]),
                    order.u32(&body[12..]) as usize,
                );
                if let (Some(i), Some(data)) = (interfaces.get(id), body.get(20..20 + caplen)) {
                    let ticks = (u64::from(hi) << 32) | u64::from(lo);
                    packets.push(Packet {
                        ts_us: (ticks as f64 / i.ticks * 1e6) as u64,
                        link: i.link,
                        data: data.to_vec(),
                    });
                }
            }
            _ => {}
        }
        at += len;
    }
    Ok(packets)
}

#[cfg(test)]
pub mod build {
    //! Writes capture files for the tests.

    pub fn pcap(link: u32, packets: &[(u64, Vec<u8>)], nanos: bool, big_endian: bool) -> Vec<u8> {
        let w32 = |v: u32| if big_endian { v.to_be_bytes() } else { v.to_le_bytes() };
        let w16 = |v: u16| if big_endian { v.to_be_bytes() } else { v.to_le_bytes() };
        let mut out = Vec::new();
        out.extend_from_slice(&w32(if nanos { 0xa1b23c4d } else { 0xa1b2c3d4 }));
        out.extend_from_slice(&w16(2));
        out.extend_from_slice(&w16(4));
        out.extend_from_slice(&w32(0));
        out.extend_from_slice(&w32(0));
        out.extend_from_slice(&w32(65535));
        out.extend_from_slice(&w32(link));
        for (ts_us, data) in packets {
            out.extend_from_slice(&w32((ts_us / 1_000_000) as u32));
            let frac = (ts_us % 1_000_000) as u32;
            out.extend_from_slice(&w32(if nanos { frac * 1000 } else { frac }));
            out.extend_from_slice(&w32(data.len() as u32));
            out.extend_from_slice(&w32(data.len() as u32));
            out.extend_from_slice(data);
        }
        out
    }

    pub fn pcapng(link: u16, packets: &[(u64, Vec<u8>)], tsresol: Option<u8>) -> Vec<u8> {
        fn block(kind: u32, body: &[u8]) -> Vec<u8> {
            let padded = body.len().div_ceil(4) * 4;
            let total = (12 + padded) as u32;
            let mut out = Vec::new();
            out.extend_from_slice(&kind.to_le_bytes());
            out.extend_from_slice(&total.to_le_bytes());
            out.extend_from_slice(body);
            out.resize(8 + padded, 0);
            out.extend_from_slice(&total.to_le_bytes());
            out
        }
        let mut shb = Vec::new();
        shb.extend_from_slice(&0x1a2b3c4du32.to_le_bytes());
        shb.extend_from_slice(&[1, 0, 0, 0]);
        shb.extend_from_slice(&u64::MAX.to_le_bytes());
        let mut idb = Vec::new();
        idb.extend_from_slice(&link.to_le_bytes());
        idb.extend_from_slice(&[0, 0]);
        idb.extend_from_slice(&65535u32.to_le_bytes());
        if let Some(r) = tsresol {
            idb.extend_from_slice(&9u16.to_le_bytes());
            idb.extend_from_slice(&1u16.to_le_bytes());
            idb.extend_from_slice(&[r, 0, 0, 0]);
            idb.extend_from_slice(&[0, 0, 0, 0]);
        }
        let mut out = block(0x0a0d0d0a, &shb);
        out.extend(block(1, &idb));
        for (ts_us, data) in packets {
            // Timestamps in the interface's own unit.
            let ticks = match tsresol {
                Some(9) => ts_us * 1000,
                _ => *ts_us,
            };
            let mut epb = Vec::new();
            epb.extend_from_slice(&0u32.to_le_bytes());
            epb.extend_from_slice(&((ticks >> 32) as u32).to_le_bytes());
            epb.extend_from_slice(&(ticks as u32).to_le_bytes());
            epb.extend_from_slice(&(data.len() as u32).to_le_bytes());
            epb.extend_from_slice(&(data.len() as u32).to_le_bytes());
            epb.extend_from_slice(data);
            out.extend(block(6, &epb));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::build::{pcap, pcapng};
    use super::*;

    fn sample() -> Vec<(u64, Vec<u8>)> {
        vec![(1_700_000_000_123_456, vec![1, 2, 3]), (1_700_000_001_000_000, vec![4, 5, 6, 7, 8])]
    }

    #[test]
    fn reads_classic_pcap_in_every_flavour() {
        for (nanos, big) in [(false, false), (true, false), (false, true), (true, true)] {
            let got = read(&pcap(1, &sample(), nanos, big)).unwrap();
            assert_eq!(got.len(), 2, "nanos={nanos} big={big}");
            assert_eq!(
                (got[0].ts_us, got[0].link, got[0].data.as_slice()),
                (1_700_000_000_123_456, 1, &[1u8, 2, 3][..])
            );
            assert_eq!(got[1].data, [4, 5, 6, 7, 8]);
        }
    }

    #[test]
    fn reads_pcapng_with_either_timestamp_unit() {
        for resol in [None, Some(9)] {
            let got = read(&pcapng(113, &sample(), resol)).unwrap();
            assert_eq!(got.len(), 2, "{resol:?}");
            assert_eq!((got[0].ts_us, got[0].link), (1_700_000_000_123_456, 113));
            assert_eq!(got[1].data, [4, 5, 6, 7, 8]);
        }
    }

    #[test]
    fn a_file_cut_in_the_middle_keeps_what_came_before() {
        let whole = pcap(1, &sample(), false, false);
        let got = read(&whole[..whole.len() - 3]).unwrap();
        assert_eq!(got.len(), 1);
        assert!(read(&pcapng(1, &sample(), None)[..100]).unwrap().len() <= 1);
    }

    #[test]
    fn refuses_what_is_not_a_capture() {
        assert!(read(b"").is_err());
        assert!(read(b"GET / HTTP/1.1").is_err());
        assert!(read(&[0xd4, 0xc3, 0xb2, 0xa1, 0, 0]).is_err(), "a header cut short");
        let mut broken = pcapng(1, &sample(), None);
        broken[28 + 4..28 + 8].copy_from_slice(&[1, 0, 0, 0]);
        assert!(read(&broken).is_err(), "a block with an impossible length");
    }
}
