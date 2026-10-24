//! Taking packets apart: link layer, IP, then TCP or UDP.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

pub const FIN: u8 = 0x01;
pub const SYN: u8 = 0x02;
pub const RST: u8 = 0x04;
pub const ACK: u8 = 0x10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tcp {
    pub src: IpAddr,
    pub dst: IpAddr,
    pub sport: u16,
    pub dport: u16,
    pub seq: u32,
    pub flags: u8,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Udp {
    pub src: IpAddr,
    pub dst: IpAddr,
    pub sport: u16,
    pub dport: u16,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    Tcp(Tcp),
    Udp(Udp),
}

/// Decodes one captured packet. `None` for anything that is not TCP or UDP over IP, and for IP
/// fragments, which are not put back together.
pub fn decode(link: u32, data: &[u8]) -> Option<Transport> {
    let ip = match link {
        // Ethernet, possibly with one VLAN tag.
        1 => {
            let mut proto = u16::from_be_bytes([*data.get(12)?, *data.get(13)?]);
            let mut at = 14;
            if proto == 0x8100 || proto == 0x88a8 {
                proto = u16::from_be_bytes([*data.get(16)?, *data.get(17)?]);
                at = 18;
            }
            matches!(proto, 0x0800 | 0x86dd).then(|| &data[at..])?
        }
        // Linux "cooked" capture, versions 1 and 2.
        113 => {
            let proto = u16::from_be_bytes([*data.get(14)?, *data.get(15)?]);
            matches!(proto, 0x0800 | 0x86dd).then(|| &data[16..])?
        }
        276 => {
            let proto = u16::from_be_bytes([*data.first()?, *data.get(1)?]);
            matches!(proto, 0x0800 | 0x86dd).then(|| data.get(20..)).flatten()?
        }
        // Raw IP, as it is on the wire or with BSD's different number.
        101 | 12 | 14 => data,
        // The loopback of BSD systems: a four byte family first.
        0 | 108 => data.get(4..)?,
        _ => return None,
    };
    match ip.first()? >> 4 {
        4 => ipv4(ip),
        6 => ipv6(ip),
        _ => None,
    }
}

fn ipv4(p: &[u8]) -> Option<Transport> {
    let ihl = usize::from(p.first()? & 0x0f) * 4;
    let total = usize::from(u16::from_be_bytes([*p.get(2)?, *p.get(3)?]));
    let frag = u16::from_be_bytes([*p.get(6)?, *p.get(7)?]);
    if ihl < 20 || p.len() < ihl || frag & 0x3fff != 0 {
        // The more-fragments bit, or a piece after the first.
        return None;
    }
    let end = if total >= ihl { total.min(p.len()) } else { p.len() };
    let src = IpAddr::V4(Ipv4Addr::new(p[12], p[13], p[14], p[15]));
    let dst = IpAddr::V4(Ipv4Addr::new(p[16], p[17], p[18], p[19]));
    transport(p[9], src, dst, &p[ihl..end])
}

fn ipv6(p: &[u8]) -> Option<Transport> {
    if p.len() < 40 {
        return None;
    }
    let len = usize::from(u16::from_be_bytes([p[4], p[5]]));
    let src = IpAddr::V6(Ipv6Addr::from(<[u8; 16]>::try_from(&p[8..24]).ok()?));
    let dst = IpAddr::V6(Ipv6Addr::from(<[u8; 16]>::try_from(&p[24..40]).ok()?));
    let end = (40 + len).min(p.len());
    let (mut next, mut at) = (p[6], 40);
    // Skip the extension headers that sit between IP and TCP.
    while matches!(next, 0 | 43 | 60) {
        let (n, size) = (*p.get(at)?, usize::from(*p.get(at + 1)?) * 8 + 8);
        next = n;
        at += size;
    }
    transport(next, src, dst, p.get(at..end)?)
}

fn transport(protocol: u8, src: IpAddr, dst: IpAddr, p: &[u8]) -> Option<Transport> {
    match protocol {
        6 => {
            let offset = usize::from(p.get(12)? >> 4) * 4;
            Some(Transport::Tcp(Tcp {
                src,
                dst,
                sport: u16::from_be_bytes([p[0], p[1]]),
                dport: u16::from_be_bytes([p[2], p[3]]),
                seq: u32::from_be_bytes([p[4], p[5], p[6], p[7]]),
                flags: p[13],
                payload: p.get(offset..)?.to_vec(),
            }))
        }
        17 => Some(Transport::Udp(Udp {
            src,
            dst,
            sport: u16::from_be_bytes([*p.first()?, *p.get(1)?]),
            dport: u16::from_be_bytes([*p.get(2)?, *p.get(3)?]),
            payload: p.get(8..)?.to_vec(),
        })),
        _ => None,
    }
}

#[cfg(test)]
pub mod build {
    //! Builds packets for the tests.

    use super::*;

    fn ip4(addr: &str) -> [u8; 4] {
        addr.parse::<Ipv4Addr>().unwrap().octets()
    }

    pub fn ipv4_packet(protocol: u8, src: &str, dst: &str, transport: &[u8]) -> Vec<u8> {
        let mut p = vec![0x45, 0, 0, 0, 0, 0, 0x40, 0, 64, protocol, 0, 0];
        p.extend_from_slice(&ip4(src));
        p.extend_from_slice(&ip4(dst));
        p.extend_from_slice(transport);
        let total = p.len() as u16;
        p[2..4].copy_from_slice(&total.to_be_bytes());
        p
    }

    pub fn tcp_segment(sport: u16, dport: u16, seq: u32, flags: u8, payload: &[u8]) -> Vec<u8> {
        let mut t = Vec::new();
        t.extend_from_slice(&sport.to_be_bytes());
        t.extend_from_slice(&dport.to_be_bytes());
        t.extend_from_slice(&seq.to_be_bytes());
        t.extend_from_slice(&[0, 0, 0, 0, 0x50, flags, 0xff, 0xff, 0, 0, 0, 0]);
        t.extend_from_slice(payload);
        t
    }

    pub fn udp_datagram(sport: u16, dport: u16, payload: &[u8]) -> Vec<u8> {
        let mut u = Vec::new();
        u.extend_from_slice(&sport.to_be_bytes());
        u.extend_from_slice(&dport.to_be_bytes());
        u.extend_from_slice(&((8 + payload.len()) as u16).to_be_bytes());
        u.extend_from_slice(&[0, 0]);
        u.extend_from_slice(payload);
        u
    }

    pub fn ethernet(ip: &[u8]) -> Vec<u8> {
        let mut f = vec![0u8; 12];
        f.extend_from_slice(&[0x08, 0x00]);
        f.extend_from_slice(ip);
        f
    }
}

#[cfg(test)]
mod tests {
    use super::build::*;
    use super::*;

    #[test]
    fn decodes_tcp_over_ethernet() {
        let ip = ipv4_packet(6, "10.0.2.15", "93.184.216.34", &tcp_segment(40000, 80, 1000, SYN | ACK, b"hi"));
        let Some(Transport::Tcp(t)) = decode(1, &ethernet(&ip)) else { panic!("not TCP") };
        assert_eq!((t.src.to_string().as_str(), t.dst.to_string().as_str()), ("10.0.2.15", "93.184.216.34"));
        assert_eq!((t.sport, t.dport, t.seq, t.flags, t.payload.as_slice()), (40000, 80, 1000, SYN | ACK, &b"hi"[..]));
    }

    #[test]
    fn decodes_udp_and_ignores_ethernet_padding() {
        let mut ip = ipv4_packet(17, "10.0.2.15", "10.0.2.3", &udp_datagram(5353, 53, b"dns"));
        ip.extend_from_slice(&[0; 6]); // the frame was padded to its minimum size
        let Some(Transport::Udp(u)) = decode(1, &ethernet(&ip)) else { panic!("not UDP") };
        assert_eq!((u.sport, u.dport, u.payload.as_slice()), (5353, 53, &b"dns"[..]));
    }

    #[test]
    fn understands_the_other_link_types() {
        let ip = ipv4_packet(6, "1.2.3.4", "5.6.7.8", &tcp_segment(1, 2, 3, ACK, b"x"));
        let mut cooked = vec![0u8; 14];
        cooked.extend_from_slice(&[0x08, 0x00]);
        cooked.extend_from_slice(&ip);
        let mut cooked2 = vec![0x08, 0x00];
        cooked2.resize(20, 0);
        cooked2.extend_from_slice(&ip);
        let mut null = vec![2, 0, 0, 0];
        null.extend_from_slice(&ip);
        let mut vlan = vec![0u8; 12];
        vlan.extend_from_slice(&[0x81, 0x00, 0x00, 0x05, 0x08, 0x00]);
        vlan.extend_from_slice(&ip);
        for (link, data) in [(113, cooked), (276, cooked2), (101, ip.clone()), (0, null), (1, vlan)] {
            assert!(matches!(decode(link, &data), Some(Transport::Tcp(t)) if t.payload == b"x"), "link type {link}");
        }
        assert!(decode(9999, &ip).is_none());
    }

    #[test]
    fn decodes_ipv6_with_an_extension_header() {
        let tcp = tcp_segment(1234, 443, 7, ACK, b"v6");
        let mut p = vec![0x60, 0, 0, 0, 0, 0, 0, 64];
        p.extend_from_slice(&"2001:db8::1".parse::<Ipv6Addr>().unwrap().octets());
        p.extend_from_slice(&"2001:db8::2".parse::<Ipv6Addr>().unwrap().octets());
        // A hop-by-hop header (next is TCP, 8 bytes long) before the segment.
        p[6] = 0;
        let mut ext = vec![6, 0, 0, 0, 0, 0, 0, 0];
        ext.extend_from_slice(&tcp);
        p.extend_from_slice(&ext);
        let len = ext.len() as u16;
        p[4..6].copy_from_slice(&len.to_be_bytes());
        let Some(Transport::Tcp(t)) = decode(101, &p) else { panic!("not TCP") };
        assert_eq!((t.dport, t.payload.as_slice(), t.src.to_string().as_str()), (443, &b"v6"[..], "2001:db8::1"));
    }

    #[test]
    fn leaves_out_fragments_and_other_protocols() {
        let mut frag = ipv4_packet(6, "1.1.1.1", "2.2.2.2", &tcp_segment(1, 2, 3, ACK, b"x"));
        frag[6] = 0x20; // more fragments
        assert!(decode(101, &frag).is_none());
        assert!(decode(101, &ipv4_packet(1, "1.1.1.1", "2.2.2.2", &[8, 0, 0, 0])).is_none(), "ICMP");
        assert!(decode(1, &[0u8; 5]).is_none() && decode(101, &[]).is_none(), "cut short");
        let arp = [vec![0u8; 12], vec![0x08, 0x06], vec![0u8; 28]].concat();
        assert!(decode(1, &arp).is_none());
    }
}
