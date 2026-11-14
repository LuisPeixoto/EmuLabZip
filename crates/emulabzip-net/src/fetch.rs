//! A small HTTPS GET for reading a file from a website: the files that tie an app to a domain
//! (`assetlinks.json`, `apple-app-site-association`).
//!
//! Redirects are not followed and are reported instead, because both Android and iOS refuse a domain file that
//! redirects. The answer is cut at a limit, the request times out, and nothing but HTTPS is spoken.

use std::time::Duration;

use bytes::Bytes;
use emulabzip_core::{Error, Result};
use http::{Request, Uri, header};
use http_body_util::{BodyExt, Empty};
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;

/// What a server answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    pub status: u16,
    pub content_type: Option<String>,
    /// Where a redirect points.
    pub location: Option<String>,
    pub body: Vec<u8>,
    /// More than the limit was sent, and the rest was dropped.
    pub truncated: bool,
}

/// An address that may be fetched: HTTPS, with a host and no credentials.
pub fn check_url(url: &str) -> Result<Uri> {
    let uri: Uri = url.parse().map_err(|_| Error::invalid("address", url))?;
    let ok = uri.scheme_str() == Some("https")
        && uri.host().is_some_and(|h| !h.is_empty())
        && !uri.authority().is_some_and(|a| a.as_str().contains('@'))
        && url.len() <= 2000
        && !url.chars().any(|c| c.is_control() || c.is_whitespace());
    if ok { Ok(uri) } else { Err(Error::invalid("address (it must be https://…)", url)) }
}

/// Reads at most `limit` bytes of `url`, giving up after `timeout`.
pub async fn fetch(url: &str, limit: usize, timeout: Duration) -> Result<Fetched> {
    let uri = check_url(url)?;
    let tls = crate::proxy::client_tls_config(&[])?;
    let https = HttpsConnectorBuilder::new().with_tls_config(tls).https_only().enable_http1().enable_http2().build();
    let client = Client::builder(TokioExecutor::new()).build::<_, Empty<Bytes>>(https);
    let request = Request::get(uri)
        .header(header::USER_AGENT, "EmuLabZip link check")
        .header(header::ACCEPT, "application/json, */*")
        .body(Empty::<Bytes>::new())
        .map_err(|e| Error::failed(e.to_string()))?;
    let work = async {
        let response =
            client.request(request).await.map_err(|e| Error::failed(format!("Could not reach {url}: {e}")))?;
        let (parts, mut body) = response.into_parts();
        let header_text =
            |name: header::HeaderName| parts.headers.get(name).and_then(|v| v.to_str().ok()).map(str::to_string);
        let mut bytes: Vec<u8> = Vec::new();
        let mut truncated = false;
        while let Some(frame) = body.frame().await {
            let frame = frame.map_err(|e| Error::failed(format!("The answer broke off: {e}")))?;
            if let Some(data) = frame.data_ref() {
                let room = limit.saturating_sub(bytes.len());
                if data.len() > room {
                    bytes.extend_from_slice(&data[..room]);
                    truncated = true;
                    break;
                }
                bytes.extend_from_slice(data);
            }
        }
        Ok(Fetched {
            status: parts.status.as_u16(),
            content_type: header_text(header::CONTENT_TYPE),
            location: header_text(header::LOCATION),
            body: bytes,
            truncated,
        })
    };
    tokio::time::timeout(timeout, work)
        .await
        .map_err(|_| Error::failed(format!("{url} did not answer in {} seconds.", timeout.as_secs())))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_https_addresses_are_fetched() {
        assert!(check_url("https://example.com/.well-known/assetlinks.json").is_ok());
        for bad in [
            "http://example.com/x",
            "file:///etc/passwd",
            "https://user:pw@example.com/",
            "https:///nohost",
            "ftp://example.com",
            "https://exa mple.com/",
            "example.com/x",
            "",
        ] {
            assert!(check_url(bad).is_err(), "{bad:?}");
        }
        assert!(check_url(&format!("https://example.com/{}", "a".repeat(2000))).is_err());
    }
}
