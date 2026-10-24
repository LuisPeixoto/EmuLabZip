//! EmuLabZip's debugging proxy.
//!
//! The proxy listens on the loopback interface. Plain HTTP requests are forwarded and recorded.
//! For `CONNECT`, TLS is terminated with a certificate signed by the [`CertificateAuthority`] the
//! device was told to trust, the decrypted HTTP/1.1 or HTTP/2 requests are forwarded to the real
//! server over a new TLS connection, and everything is recorded as a [`Flow`] in a [`FlowStore`].
//! Connections that are not TLS, and apps that refuse our certificate, are passed through.

pub mod breakpoints;
pub mod ca;
pub mod decode;
pub mod flow;
pub mod har;
pub mod http3;
pub mod io;
pub mod pcap;
pub mod proxy;
pub mod rules;
pub mod store;
pub mod ws;

pub use breakpoints::{BreakpointEvent, Breakpoints, Edits, Paused, Phase, Resume};
pub use ca::{CertificateAuthority, default_dir};
pub use flow::{Flow, FlowId, FlowKind, FlowState, FlowSummary, Headers};
pub use har::to_har;
pub use http3::Http3;
pub use proxy::{Proxy, ProxyConfig, ProxyHandle};
pub use rules::{Action, Matcher, Rule, RuleSet};
pub use store::{FlowEvent, FlowStore};
