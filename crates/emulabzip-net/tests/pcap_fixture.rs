//! Reads a capture written by a real emulator (`emulator -tcpdump`, Android 14, x86_64): the
//! connectivity check over HTTP, a TLS connection to Google, and the DNS lookups around them.

use emulabzip_net::flow::{FlowKind, FlowState};
use emulabzip_net::pcap::import;

const CAPTURE: &[u8] = include_bytes!("fixtures/emulator-tcpdump.pcap");

#[test]
fn reads_what_a_real_emulator_wrote() {
    let capture = import(CAPTURE).unwrap();
    assert_eq!(capture.summary.packets, 143);
    assert!(!capture.summary.gaps);

    let http: Vec<_> = capture.flows.iter().filter(|f| f.kind == FlowKind::Http).collect();
    assert_eq!(http.len(), 1, "{:#?}", capture.flows.iter().map(|f| &f.url).collect::<Vec<_>>());
    let check = http[0];
    assert_eq!((check.method.as_str(), check.status), ("GET", Some(204)));
    assert_eq!(check.url, "http://connectivitycheck.gstatic.com/generate_204");
    assert_eq!((check.state, check.source.as_deref()), (FlowState::Completed, Some("pcap")));
    assert!(check.timing.total_ms.is_some());

    // The encrypted connection is named by the server name in its first message.
    let tls = capture.flows.iter().find(|f| f.kind == FlowKind::Tunnel).expect("a TLS connection");
    assert_eq!((tls.host.as_str(), tls.port), ("www.google.com", 443));
    assert!(tls.response_size > 1000, "the server's answer was counted");

    let answer = |name: &str, kind: &str| {
        capture
            .dns
            .iter()
            .find(|l| l.name == name && l.kind == kind)
            .unwrap_or_else(|| panic!("no {kind} lookup of {name}"))
    };
    let a = answer("www.google.com", "A");
    assert_eq!(a.status.as_deref(), Some("NOERROR"));
    assert!(a.answers.iter().all(|ip| ip.parse::<std::net::Ipv4Addr>().is_ok()) && !a.answers.is_empty(), "{a:?}");
    assert!(answer("connectivitycheck.gstatic.com", "AAAA").answers[0].contains(':'));
    assert_eq!(capture.dns.len(), 6);
}
