//! Importing a packet capture, such as the file `emulator -tcpdump` writes, as flows.
//!
//! The capture is read packet by packet, the TCP connections are put back together, and what can be
//! read from them is shown the way live traffic is: plain HTTP/1.x as requests and responses,
//! encrypted connections as tunnels named by the server name in their first message, anything else
//! as a tunnel to its address. DNS lookups come out in a list of their own.

pub mod dns;
pub mod file;
pub mod http;
pub mod net;
pub mod tcp;
pub mod tls;

use std::collections::HashMap;
use std::net::IpAddr;

use emulabzip_core::{Error, Result};
use serde::Serialize;

use crate::flow::{BODY_LIMIT, Flow, FlowKind, FlowState};
use net::{ACK, SYN, Transport};
use tcp::Stream;

/// The biggest capture file that is read.
pub const MAX_CAPTURE: usize = 512 * 1024 * 1024;

type Endpoint = (IpAddr, u16);

struct Connection {
    client: Endpoint,
    server: Endpoint,
    up: Stream,
    down: Stream,
    first_ts: u64,
    last_ts: u64,
}

/// What an import found.
#[derive(Debug)]
pub struct Capture {
    pub flows: Vec<Flow>,
    pub dns: Vec<dns::Lookup>,
    pub summary: Summary,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub packets: u64,
    /// Packets that were not TCP or UDP over IP, or were fragments.
    pub skipped_packets: u64,
    pub http_requests: u64,
    pub tls_connections: u64,
    pub other_connections: u64,
    pub dns_lookups: u64,
    pub duration_ms: u64,
    /// Some segments of some connection are missing from the capture.
    pub gaps: bool,
    /// Some connection had more data than is kept.
    pub truncated: bool,
}

fn ordered(a: Endpoint, b: Endpoint) -> (Endpoint, Endpoint) {
    if a <= b { (a, b) } else { (b, a) }
}

/// Reads a `.pcap` or `.pcapng` file.
pub fn import(bytes: &[u8]) -> Result<Capture> {
    if bytes.len() > MAX_CAPTURE {
        return Err(Error::invalid(
            "capture size",
            format!("{} MB (at most {})", bytes.len() / (1024 * 1024), MAX_CAPTURE / (1024 * 1024)),
        ));
    }
    let packets = file::read(bytes)?;
    let mut summary = Summary { packets: packets.len() as u64, ..Default::default() };
    let mut connections: HashMap<(Endpoint, Endpoint), Connection> = HashMap::new();
    let mut order: Vec<(Endpoint, Endpoint)> = Vec::new();
    let mut lookups = dns::Tracker::default();
    let (mut first, mut last) = (u64::MAX, 0u64);

    for packet in &packets {
        first = first.min(packet.ts_us);
        last = last.max(packet.ts_us);
        match net::decode(packet.link, &packet.data) {
            Some(Transport::Tcp(t)) => {
                let (src, dst) = ((t.src, t.sport), (t.dst, t.dport));
                let key = ordered(src, dst);
                let c = connections.entry(key).or_insert_with(|| {
                    order.push(key);
                    // Whoever sends the opening SYN is the client; with no SYN in the capture, whoever
                    // is heard first is taken to be.
                    let (client, server) = match (t.flags & SYN != 0, t.flags & ACK != 0) {
                        (true, true) => (dst, src),
                        _ => (src, dst),
                    };
                    Connection {
                        client,
                        server,
                        up: Stream::default(),
                        down: Stream::default(),
                        first_ts: packet.ts_us,
                        last_ts: packet.ts_us,
                    }
                });
                c.last_ts = c.last_ts.max(packet.ts_us);
                let stream = if src == c.client { &mut c.up } else { &mut c.down };
                stream.add(t.seq, t.flags & SYN != 0, &t.payload, packet.ts_us);
            }
            Some(Transport::Udp(u)) if u.sport == 53 || u.dport == 53 => lookups.feed(packet.ts_us, &u.payload),
            Some(Transport::Udp(_)) => {}
            None => summary.skipped_packets += 1,
        }
    }
    summary.duration_ms = if packets.is_empty() { 0 } else { (last - first) / 1000 };

    let mut flows = Vec::new();
    for key in order {
        let Some(mut c) = connections.remove(&key) else { continue };
        c.up.finish();
        c.down.finish();
        summary.gaps |= c.up.gaps || c.down.gaps;
        summary.truncated |= c.up.truncated || c.down.truncated;
        if c.up.data.is_empty() && c.down.data.is_empty() {
            continue; // a handshake and nothing else
        }
        if http::looks_like_request(&c.up.data) {
            let made = http_flows(&c);
            summary.http_requests += made.len() as u64;
            flows.extend(made);
        } else if tls::looks_like_tls(&c.up.data) {
            summary.tls_connections += 1;
            flows.push(tunnel(&c, tls::server_name(&c.up.data)));
        } else {
            summary.other_connections += 1;
            flows.push(tunnel(&c, None));
        }
    }
    flows.sort_by_key(|f| f.started_ms);
    let dns = lookups.into_lookups();
    summary.dns_lookups = dns.len() as u64;
    Ok(Capture { flows, dns, summary })
}

fn ms(us: Option<u64>) -> u64 {
    us.unwrap_or(0) / 1000
}

/// The host a request names: its `Host` header without the port, or the server's address.
fn host_of(request: &http::Request, server: IpAddr) -> String {
    let named = request.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("host")).map(|(_, v)| v.trim());
    match named {
        Some(h) if !h.is_empty() => {
            let name = if h.starts_with('[') {
                h.split(']').next().map_or(h, |x| &h[..x.len() + 1])
            } else {
                h.rsplit_once(':').filter(|(_, p)| p.parse::<u16>().is_ok()).map_or(h, |(n, _)| n)
            };
            name.to_string()
        }
        _ => server.to_string(),
    }
}

/// A request line may carry the whole address (`GET http://host/path`), as it does to a proxy.
fn origin_path(path: &str) -> String {
    for scheme in ["http://", "https://"] {
        if let Some(rest) = path.strip_prefix(scheme) {
            return rest.find('/').map_or("/".to_string(), |i| rest[i..].to_string());
        }
    }
    path.to_string()
}

fn http_flows(c: &Connection) -> Vec<Flow> {
    let requests = http::requests(&c.up.data);
    let methods: Vec<String> = requests.iter().map(|r| r.method.clone()).collect();
    let responses = http::responses(&c.down.data, &methods);
    let mut flows = Vec::new();
    for (i, request) in requests.iter().enumerate() {
        let host = host_of(request, c.server.0);
        let mut f = Flow::new(FlowKind::Http, &request.method, "http", &host, c.server.1, &origin_path(&request.path));
        let began = c.up.ts_range(request.start, request.end).map(|(first, _)| first);
        f.started_ms = ms(began);
        f.http_version = format!("HTTP/1.{}", request.version);
        f.request_headers = request.headers.clone();
        f.request_size = request.body.len() as u64;
        f.request_truncated = request.body.len() > BODY_LIMIT;
        f.request_body = request.body[..request.body.len().min(BODY_LIMIT)].to_vec();
        f.source = Some("pcap".into());
        match responses.get(i) {
            Some(r) => {
                f.status = Some(r.status);
                f.response_headers = r.headers.clone();
                f.response_size = r.body.len() as u64;
                f.response_truncated = r.body.len() > BODY_LIMIT;
                f.response_body = r.body[..r.body.len().min(BODY_LIMIT)].to_vec();
                let start = began.unwrap_or(0);
                let (first, last) = c.down.ts_range(r.start, r.end).unwrap_or((start, start));
                f.timing.ttfb_ms = Some(first.saturating_sub(start) / 1000);
                f.timing.total_ms = Some(last.saturating_sub(start) / 1000);
                f.state = FlowState::Completed;
            }
            None => {
                f.state = FlowState::Failed;
                f.error = Some("The capture has no response to this request.".into());
            }
        }
        flows.push(f);
    }
    flows
}

fn tunnel(c: &Connection, name: Option<String>) -> Flow {
    let host = name.unwrap_or_else(|| c.server.0.to_string());
    let mut f = Flow::new(FlowKind::Tunnel, "CONNECT", "tcp", &host, c.server.1, "");
    f.http_version = String::new();
    f.started_ms = c.first_ts / 1000;
    f.request_size = c.up.wire_bytes;
    f.response_size = c.down.wire_bytes;
    f.timing.total_ms = Some(c.last_ts.saturating_sub(c.first_ts) / 1000);
    f.state = FlowState::Completed;
    f.source = Some("pcap".into());
    f
}

#[cfg(test)]
mod tests {
    use super::dns::build as dnsb;
    use super::file::build as filebuild;
    use super::net::build::*;
    use super::*;

    const CLIENT: &str = "10.0.2.15";
    const SERVER: &str = "93.184.216.34";

    fn packet(
        ts: u64,
        from_client: bool,
        sport: u16,
        dport: u16,
        seq: u32,
        flags: u8,
        payload: &[u8],
    ) -> (u64, Vec<u8>) {
        let (src, dst, sp, dp) =
            if from_client { (CLIENT, SERVER, sport, dport) } else { (SERVER, CLIENT, dport, sport) };
        (ts, ethernet(&ipv4_packet(6, src, dst, &tcp_segment(sp, dp, seq, flags, payload))))
    }

    /// One HTTP exchange on port 80, with the response split in pieces and sent out of order and twice.
    fn http_exchange() -> Vec<(u64, Vec<u8>)> {
        let request = b"GET /hello?name=emu HTTP/1.1\r\nHost: example.test\r\nAccept: */*\r\n\r\n";
        let body = b"{\"hello\":\"world\"}";
        let head =
            format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len());
        let response = [head.as_bytes(), body].concat();
        let (a, b) = response.split_at(20);
        let (b1, b2) = b.split_at(30);
        let (cseq, sseq) = (1000u32, 9000u32);
        vec![
            packet(1_000_000, true, 40000, 80, cseq, SYN, &[]),
            packet(1_001_000, false, 40000, 80, sseq, SYN | ACK, &[]),
            packet(1_002_000, true, 40000, 80, cseq + 1, ACK, &[]),
            packet(1_010_000, true, 40000, 80, cseq + 1, ACK, request),
            // The response arrives scrambled: the middle first, the end, a repeat, then the start.
            packet(1_052_000, false, 40000, 80, sseq + 1 + a.len() as u32, ACK, b1),
            packet(1_053_000, false, 40000, 80, sseq + 1 + (a.len() + b1.len()) as u32, ACK, b2),
            packet(1_054_000, false, 40000, 80, sseq + 1 + a.len() as u32, ACK, b1),
            packet(1_055_000, false, 40000, 80, sseq + 1, ACK, a),
        ]
    }

    #[test]
    fn rebuilds_an_http_exchange_from_scrambled_segments() {
        let capture = import(&filebuild::pcap(1, &http_exchange(), false, false)).unwrap();
        assert_eq!(capture.flows.len(), 1, "{:?}", capture.summary);
        let f = &capture.flows[0];
        assert_eq!(
            (f.kind, f.method.as_str(), f.scheme.as_str(), f.host.as_str(), f.port),
            (FlowKind::Http, "GET", "http", "example.test", 80)
        );
        assert_eq!(f.url, "http://example.test/hello?name=emu");
        assert_eq!((f.status, f.state, f.http_version.as_str()), (Some(200), FlowState::Completed, "HTTP/1.1"));
        assert_eq!(f.response_body, b"{\"hello\":\"world\"}");
        assert_eq!(f.content_type().as_deref(), Some("application/json"));
        assert_eq!(f.source.as_deref(), Some("pcap"));
        assert_eq!(f.started_ms, 1010, "the request was captured at 1.010 s");
        assert_eq!(
            (f.timing.ttfb_ms, f.timing.total_ms),
            (Some(42), Some(45)),
            "from the request to the first and the last segment of the response"
        );
        assert_eq!((capture.summary.http_requests, capture.summary.packets, capture.summary.gaps), (1, 8, false));
    }

    #[test]
    fn pcapng_gives_the_same_answer() {
        let a = import(&filebuild::pcap(1, &http_exchange(), false, false)).unwrap();
        let b = import(&filebuild::pcapng(1, &http_exchange(), None)).unwrap();
        assert_eq!(b.flows.len(), 1);
        assert_eq!(
            (b.flows[0].url.as_str(), b.flows[0].response_body.as_slice()),
            (a.flows[0].url.as_str(), a.flows[0].response_body.as_slice())
        );
        assert_eq!(b.flows[0].timing, a.flows[0].timing);
    }

    #[test]
    fn two_requests_on_one_connection_become_two_flows() {
        let r1 = b"GET /one HTTP/1.1\r\nHost: h.test\r\n\r\n";
        let r2 = b"POST /two HTTP/1.1\r\nHost: h.test\r\nContent-Length: 3\r\n\r\nabc";
        let a1 = b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n1";
        let a2 = b"HTTP/1.1 500 Oops\r\nContent-Length: 1\r\n\r\n2";
        let p = vec![
            packet(1, true, 1111, 8080, 0, SYN, &[]),
            packet(2, false, 1111, 8080, 0, SYN | ACK, &[]),
            packet(10, true, 1111, 8080, 1, ACK, r1),
            packet(20, false, 1111, 8080, 1, ACK, a1),
            packet(30, true, 1111, 8080, 1 + r1.len() as u32, ACK, r2),
            packet(40, false, 1111, 8080, 1 + a1.len() as u32, ACK, a2),
        ];
        let c = import(&filebuild::pcap(1, &p, false, false)).unwrap();
        assert_eq!(
            c.flows.iter().map(|f| (f.method.as_str(), f.path.as_str(), f.status, f.port)).collect::<Vec<_>>(),
            [("GET", "/one", Some(200), 8080), ("POST", "/two", Some(500), 8080)]
        );
        assert_eq!(c.flows[1].request_body, b"abc");
    }

    #[test]
    fn a_request_without_a_response_is_a_failed_flow() {
        let p = vec![packet(1, true, 2222, 80, 5, ACK, b"GET /x HTTP/1.1\r\nHost: a.test\r\n\r\n")];
        let c = import(&filebuild::pcap(1, &p, false, false)).unwrap();
        assert_eq!((c.flows[0].state, c.flows[0].status), (FlowState::Failed, None));
        assert!(c.flows[0].error.as_deref().unwrap().contains("no response"));
    }

    #[test]
    fn encrypted_connections_are_named_by_their_server_name() {
        let hello = tls::client_hello("secure.example.com");
        let p = vec![
            packet(1, true, 3333, 443, 100, SYN, &[]),
            packet(2, false, 3333, 443, 500, SYN | ACK, &[]),
            packet(10, true, 3333, 443, 101, ACK, &hello),
            packet(20, false, 3333, 443, 501, ACK, &[0x16, 0x03, 0x03, 0, 4, 2, 0, 0, 0]),
        ];
        let c = import(&filebuild::pcap(1, &p, false, false)).unwrap();
        let f = &c.flows[0];
        assert_eq!((f.kind, f.host.as_str(), f.port), (FlowKind::Tunnel, "secure.example.com", 443));
        assert_eq!((f.request_size, f.response_size), (hello.len() as u64, 9));
        assert_eq!(c.summary.tls_connections, 1);
    }

    #[test]
    fn other_connections_are_tunnels_to_their_address() {
        let p =
            vec![packet(1, true, 4444, 5228, 1, ACK, b"\x00\x01binary"), packet(2, false, 4444, 5228, 1, ACK, b"ok")];
        let c = import(&filebuild::pcap(1, &p, false, false)).unwrap();
        assert_eq!((c.flows[0].kind, c.flows[0].host.as_str(), c.flows[0].port), (FlowKind::Tunnel, SERVER, 5228));
        assert_eq!(c.summary.other_connections, 1);
    }

    #[test]
    fn a_capture_that_missed_segments_says_so() {
        let request = b"GET /big HTTP/1.1\r\nHost: g.test\r\n\r\n";
        let p = vec![
            packet(1, true, 5555, 80, 1, ACK, request),
            packet(2, false, 5555, 80, 1, ACK, b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nab"),
            // bytes 38..44 were never captured
            packet(3, false, 5555, 80, 1 + 44, ACK, b"ij"),
        ];
        let c = import(&filebuild::pcap(1, &p, false, false)).unwrap();
        assert!(c.summary.gaps);
        assert_eq!(c.flows[0].status, Some(200));
    }

    #[test]
    fn dns_lookups_are_listed_with_their_answers() {
        let q = dnsb::query(1, "example.test", 1);
        let a = dnsb::answer(1, "example.test", 1, 0, &[(1, vec![93, 184, 216, 34])]);
        let udp = |from_client: bool, payload: &[u8]| {
            let (src, dst) = if from_client { (CLIENT, "10.0.2.3") } else { ("10.0.2.3", CLIENT) };
            let (sp, dp) = if from_client { (5000, 53) } else { (53, 5000) };
            ethernet(&ipv4_packet(17, src, dst, &udp_datagram(sp, dp, payload)))
        };
        let p = vec![(5_000_000, udp(true, &q)), (5_030_000, udp(false, &a))];
        let c = import(&filebuild::pcap(1, &p, false, false)).unwrap();
        assert!(c.flows.is_empty());
        assert_eq!((c.dns.len(), c.dns[0].name.as_str(), c.dns[0].duration_ms), (1, "example.test", Some(30)));
        assert_eq!((c.summary.dns_lookups, c.summary.duration_ms), (1, 30));
    }

    #[test]
    fn packets_that_are_not_tcp_or_udp_are_counted_and_left_out() {
        let arp = [vec![0u8; 12], vec![0x08, 0x06], vec![0u8; 28]].concat();
        let p = vec![(1, arp), (2, vec![1, 2, 3])];
        let c = import(&filebuild::pcap(1, &p, false, false)).unwrap();
        assert_eq!((c.summary.packets, c.summary.skipped_packets, c.flows.len()), (2, 2, 0));
    }

    #[test]
    fn a_proxy_style_request_line_and_an_ipv6_host_header() {
        let p = vec![packet(
            1,
            true,
            6666,
            3128,
            1,
            ACK,
            b"GET http://via.proxy.test/path?q=1 HTTP/1.1\r\nHost: via.proxy.test:80\r\n\r\n",
        )];
        let c = import(&filebuild::pcap(1, &p, false, false)).unwrap();
        assert_eq!((c.flows[0].host.as_str(), c.flows[0].path.as_str()), ("via.proxy.test", "/path?q=1"));
        assert_eq!(origin_path("/plain"), "/plain");
        let req = http::requests(b"GET / HTTP/1.1\r\nHost: [::1]:8080\r\n\r\n").remove(0);
        assert_eq!(host_of(&req, "1.2.3.4".parse().unwrap()), "[::1]");
    }

    #[test]
    fn refuses_files_that_are_not_captures() {
        assert!(import(b"not a capture").is_err());
        assert!(import(&[]).is_err());
    }
}
