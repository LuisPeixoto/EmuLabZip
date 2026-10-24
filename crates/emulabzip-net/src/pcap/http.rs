//! HTTP/1.x messages in the reassembled bytes of a connection.

use crate::flow::Headers;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub path: String,
    /// 0 for HTTP/1.0, 1 for HTTP/1.1.
    pub version: u8,
    pub headers: Headers,
    pub body: Vec<u8>,
    /// Where the message starts and ends in the stream.
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub version: u8,
    pub headers: Headers,
    pub body: Vec<u8>,
    pub start: usize,
    pub end: usize,
}

const METHODS: &[&str] = &["GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "PATCH", "TRACE", "CONNECT"];

/// Whether `data` begins with an HTTP request line.
pub fn looks_like_request(data: &[u8]) -> bool {
    METHODS.iter().any(|m| data.starts_with(m.as_bytes()) && data.get(m.len()) == Some(&b' '))
}

fn header_list(headers: &[httparse::Header<'_>]) -> Headers {
    headers.iter().map(|h| (h.name.to_string(), String::from_utf8_lossy(h.value).into_owned())).collect()
}

fn find<'a>(headers: &'a Headers, name: &str) -> Option<&'a str> {
    headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

/// Reads a chunked body. Returns it and how many bytes the chunks took; a body that is cut
/// short gives what there is and takes everything.
fn dechunk(data: &[u8]) -> (Vec<u8>, usize) {
    let (mut body, mut at) = (Vec::new(), 0);
    loop {
        let Some(line_end) = data[at..].windows(2).position(|w| w == b"\r\n") else { return (body, data.len()) };
        let size_text = String::from_utf8_lossy(&data[at..at + line_end]);
        let Ok(size) = usize::from_str_radix(size_text.split(';').next().unwrap_or("").trim(), 16) else {
            return (body, data.len());
        };
        at += line_end + 2;
        if size == 0 {
            // Optional trailers, then an empty line.
            loop {
                let Some(end) = data[at..].windows(2).position(|w| w == b"\r\n") else { return (body, data.len()) };
                at += end + 2;
                if end == 0 {
                    return (body, at);
                }
            }
        }
        let Some(chunk) = data.get(at..at + size) else {
            body.extend_from_slice(&data[at..]);
            return (body, data.len());
        };
        body.extend_from_slice(chunk);
        at += size + 2;
        if at > data.len() {
            return (body, data.len());
        }
    }
}

/// The body that follows the headers, and how many bytes it takes.
fn body_of(headers: &Headers, rest: &[u8], until_close: bool) -> (Vec<u8>, usize) {
    if find(headers, "transfer-encoding").is_some_and(|v| v.to_ascii_lowercase().contains("chunked")) {
        return dechunk(rest);
    }
    if let Some(n) = find(headers, "content-length").and_then(|v| v.trim().parse::<usize>().ok()) {
        let n = n.min(rest.len());
        return (rest[..n].to_vec(), n);
    }
    if until_close { (rest.to_vec(), rest.len()) } else { (Vec::new(), 0) }
}

/// Every request in the client's bytes, in order.
pub fn requests(data: &[u8]) -> Vec<Request> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < data.len() {
        let mut storage = [httparse::EMPTY_HEADER; 96];
        let mut parsed = httparse::Request::new(&mut storage);
        let Ok(httparse::Status::Complete(head)) = parsed.parse(&data[at..]) else { break };
        let headers = header_list(parsed.headers);
        let (body, taken) = body_of(&headers, &data[at + head..], false);
        out.push(Request {
            method: parsed.method.unwrap_or("GET").to_string(),
            path: parsed.path.unwrap_or("/").to_string(),
            version: parsed.version.unwrap_or(1),
            headers,
            body,
            start: at,
            end: at + head + taken,
        });
        at += head + taken;
    }
    out
}

/// Every response in the server's bytes. `methods` are the methods of the requests, in order: a
/// response to `HEAD` has no body, whatever its headers say. Interim `1xx` responses are skipped.
pub fn responses(data: &[u8], methods: &[String]) -> Vec<Response> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < data.len() {
        let mut storage = [httparse::EMPTY_HEADER; 96];
        let mut parsed = httparse::Response::new(&mut storage);
        let Ok(httparse::Status::Complete(head)) = parsed.parse(&data[at..]) else { break };
        let status = parsed.code.unwrap_or(0);
        let headers = header_list(parsed.headers);
        let upgraded = status == 101;
        let head_only = methods.get(out.len()).is_some_and(|m| m.eq_ignore_ascii_case("HEAD"));
        let no_body = head_only || (100..200).contains(&status) || status == 204 || status == 304;
        let (body, taken) = if no_body { (Vec::new(), 0) } else { body_of(&headers, &data[at + head..], true) };
        let message =
            Response { status, version: parsed.version.unwrap_or(1), headers, body, start: at, end: at + head + taken };
        at += head + taken;
        if (100..200).contains(&status) && !upgraded {
            continue; // `100 Continue` and friends are not the answer
        }
        out.push(message);
        if upgraded {
            break; // what follows is not HTTP
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_get_and_a_post_with_a_body() {
        let data = b"GET /a?x=1 HTTP/1.1\r\nHost: api.test\r\nAccept: */*\r\n\r\nPOST /b HTTP/1.0\r\nContent-Length: 5\r\n\r\nhelloGET /c HTTP/1.1\r\n\r\n";
        let r = requests(data);
        assert_eq!(
            r.iter().map(|r| (r.method.as_str(), r.path.as_str(), r.version)).collect::<Vec<_>>(),
            [("GET", "/a?x=1", 1), ("POST", "/b", 0), ("GET", "/c", 1)]
        );
        assert_eq!(r[1].body, b"hello");
        assert_eq!(r[0].headers[0], ("Host".to_string(), "api.test".to_string()));
        assert_eq!((r[0].start, r[0].end), (0, 52));
        assert_eq!(r[2].end, data.len());
    }

    #[test]
    fn reads_responses_with_each_kind_of_body() {
        let data = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nokHTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6;ext=1\r\n world\r\n0\r\nX-Trailer: 1\r\n\r\nHTTP/1.1 404 Not Found\r\n\r\nrest of the stream";
        let r = responses(data, &[]);
        assert_eq!(r.len(), 3);
        assert_eq!((r[0].status, r[0].body.as_slice()), (200, &b"ok"[..]));
        assert_eq!(r[1].body, b"hello world");
        assert_eq!(
            (r[2].status, r[2].body.as_slice()),
            (404, &b"rest of the stream"[..]),
            "no length: the body runs to the end"
        );
    }

    #[test]
    fn responses_that_have_no_body() {
        let data = b"HTTP/1.1 204 No Content\r\n\r\nHTTP/1.1 304 Not Modified\r\nContent-Length: 100\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\n";
        let r = responses(data, &["GET".into(), "GET".into(), "HEAD".into()]);
        assert_eq!(
            r.iter().map(|r| (r.status, r.body.len())).collect::<Vec<_>>(),
            [(204, 0), (304, 0), (200, 0)],
            "the last answers a HEAD"
        );
    }

    #[test]
    fn skips_continue_and_stops_at_an_upgrade() {
        let data = b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\nxHTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n\r\n\x81\x05hello";
        let r = responses(data, &[]);
        assert_eq!(r.iter().map(|r| r.status).collect::<Vec<_>>(), [200, 101]);
    }

    #[test]
    fn a_message_cut_short_keeps_what_there_is() {
        let r = requests(b"POST /x HTTP/1.1\r\nContent-Length: 10\r\n\r\npartial");
        assert_eq!(r[0].body, b"partial");
        let c = responses(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhel", &[]);
        assert_eq!(c[0].body, b"hel");
        assert!(requests(b"GET /x HTT").is_empty() && responses(b"HTTP/1.1 2", &[]).is_empty());
    }

    #[test]
    fn recognizes_the_start_of_a_request() {
        assert!(looks_like_request(b"GET / HTTP/1.1") && looks_like_request(b"POST /x"));
        assert!(
            !looks_like_request(b"GETX / HTTP/1.1")
                && !looks_like_request(&[0x16, 0x03, 0x01])
                && !looks_like_request(b"")
        );
    }
}
