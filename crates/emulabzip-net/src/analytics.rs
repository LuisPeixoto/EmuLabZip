//! Reading what analytics SDKs send: Segment, Amplitude, Mixpanel and Firebase Analytics.
//!
//! An analytics call is an ordinary HTTP request, so it stays in the request list. This module reads its host, address,
//! headers and body and says which events it carries. It works on any recorded request, including a session or a HAR file
//! that is opened later, because nothing is added to the request when it is captured.

use base64::Engine;
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::classify::{percent_decode, query_param};
use crate::flow::Flow;

/// One service that is recognised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Service {
    Segment,
    Amplitude,
    Mixpanel,
    Firebase,
}

impl Service {
    pub fn label(self) -> &'static str {
        match self {
            Service::Segment => "Segment",
            Service::Amplitude => "Amplitude",
            Service::Mixpanel => "Mixpanel",
            Service::Firebase => "Firebase",
        }
    }

    pub fn parse(text: &str) -> Option<Service> {
        match text.trim().to_ascii_lowercase().as_str() {
            "segment" => Some(Service::Segment),
            "amplitude" => Some(Service::Amplitude),
            "mixpanel" => Some(Service::Mixpanel),
            "firebase" => Some(Service::Firebase),
            _ => None,
        }
    }
}

/// One event, screen view, identification or profile change.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsEvent {
    pub service: Service,
    /// `track`, `screen`, `page`, `identify`, `group`, `alias` or `profile`.
    pub kind: String,
    /// The event's name: `Order Completed`, `screen_view`.
    pub name: String,
    pub properties: Value,
    /// The user id, or the anonymous or device id when there is no user.
    pub user: Option<String>,
    /// Milliseconds since the Unix epoch, when the event carries a time.
    pub time_ms: Option<u64>,
}

fn ends(host: &str, domain: &str) -> bool {
    let h = host.to_ascii_lowercase();
    h == domain || h.ends_with(&format!(".{domain}"))
}

/// The service a host belongs to.
pub fn detect(host: &str) -> Option<Service> {
    if ends(host, "segment.io") || ends(host, "segment.com") {
        Some(Service::Segment)
    } else if ends(host, "amplitude.com") {
        Some(Service::Amplitude)
    } else if ends(host, "mixpanel.com") {
        Some(Service::Mixpanel)
    } else if ends(host, "app-measurement.com") {
        Some(Service::Firebase)
    } else {
        None
    }
}

/// The events a request carries, when it goes to one of the services. A request that is not analytics, or that is, but
/// cannot be read, gives none.
pub fn events(flow: &Flow) -> Vec<AnalyticsEvent> {
    let Some(service) = detect(&flow.host) else { return Vec::new() };
    let encoding = Flow::header(&flow.request_headers, "content-encoding");
    let body = crate::decode::decode(encoding, &flow.request_body).unwrap_or_default();
    let content_type = Flow::header(&flow.request_headers, "content-type").unwrap_or("");
    decode(service, &flow.path, content_type, &body)
}

/// [`events`] for the parts of a request.
pub fn decode(service: Service, path_and_query: &str, content_type: &str, body: &[u8]) -> Vec<AnalyticsEvent> {
    let (path, query) = path_and_query.split_once('?').unwrap_or((path_and_query, ""));
    match service {
        Service::Segment => segment(path, body),
        Service::Amplitude => amplitude(path, query, content_type, body),
        Service::Mixpanel => mixpanel(path, query, body),
        Service::Firebase => firebase(body),
    }
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

fn millis(v: &Value) -> Option<u64> {
    match v {
        Value::Number(n) => n.as_u64().or_else(|| n.as_f64().map(|f| f as u64)),
        Value::String(s) => chrono_like(s),
        _ => None,
    }
}

/// An ISO 8601 time such as `2026-11-07T20:54:21.123Z`, as milliseconds. Only the plain UTC forms are read.
fn chrono_like(s: &str) -> Option<u64> {
    let t = time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()?;
    u64::try_from(t.unix_timestamp_nanos() / 1_000_000).ok()
}

// ---- Segment --------------------------------------------------------------------------------

fn segment(path: &str, body: &[u8]) -> Vec<AnalyticsEvent> {
    let Ok(value) = serde_json::from_slice::<Value>(body) else { return Vec::new() };
    let from_path = match path.trim_end_matches('/').rsplit('/').next().unwrap_or("") {
        "t" | "track" => Some("track"),
        "i" | "identify" => Some("identify"),
        "p" | "page" => Some("page"),
        "s" | "screen" => Some("screen"),
        "g" | "group" => Some("group"),
        "a" | "alias" => Some("alias"),
        _ => None,
    };
    let calls: Vec<&Value> = match value.get("batch").and_then(Value::as_array) {
        Some(batch) => batch.iter().collect(),
        None => vec![&value],
    };
    calls
        .into_iter()
        .filter_map(|c| {
            let kind = text(c, "type").or_else(|| from_path.map(str::to_string))?;
            let name = match kind.as_str() {
                "track" => text(c, "event")?,
                "page" | "screen" => text(c, "name").unwrap_or_else(|| kind.clone()),
                "identify" => "identify".to_string(),
                other => other.to_string(),
            };
            let properties = c.get("properties").or_else(|| c.get("traits")).cloned().unwrap_or(Value::Null);
            Some(AnalyticsEvent {
                service: Service::Segment,
                kind,
                name,
                properties,
                user: text(c, "userId").or_else(|| text(c, "anonymousId")),
                time_ms: c.get("timestamp").and_then(millis),
            })
        })
        .collect()
}

// ---- Amplitude ------------------------------------------------------------------------------

fn amplitude(_path: &str, query: &str, content_type: &str, body: &[u8]) -> Vec<AnalyticsEvent> {
    // The older endpoint sends a form with the events in `e`; the HTTP API sends JSON.
    let events: Vec<Value> =
        if content_type.contains("x-www-form-urlencoded") || body.starts_with(b"e=") || body.contains(&b'&') {
            let form = String::from_utf8_lossy(body);
            let e = form
                .split('&')
                .find_map(|p| p.strip_prefix("e="))
                .or_else(|| query.split('&').find_map(|p| p.strip_prefix("e=")));
            match e.and_then(|e| serde_json::from_str::<Value>(&percent_decode(e)).ok()) {
                Some(Value::Array(a)) => a,
                _ => return Vec::new(),
            }
        } else {
            match serde_json::from_slice::<Value>(body) {
                Ok(v) => match v.get("events").and_then(Value::as_array) {
                    Some(a) => a.clone(),
                    None => return Vec::new(),
                },
                Err(_) => return Vec::new(),
            }
        };
    events
        .iter()
        .filter_map(|e| {
            let name = text(e, "event_type")?;
            let mut props = e.get("event_properties").and_then(Value::as_object).cloned().unwrap_or_default();
            if let Some(user_props) = e.get("user_properties").filter(|v| v.as_object().is_some_and(|o| !o.is_empty()))
            {
                props.insert("$user_properties".into(), user_props.clone());
            }
            let kind = if name.starts_with("[Amplitude] ") && name.contains("Screen") { "screen" } else { "track" };
            Some(AnalyticsEvent {
                service: Service::Amplitude,
                kind: kind.into(),
                name,
                properties: Value::Object(props),
                user: text(e, "user_id").or_else(|| text(e, "device_id")),
                time_ms: e.get("time").and_then(millis),
            })
        })
        .collect()
}

// ---- Mixpanel -------------------------------------------------------------------------------

fn mixpanel(path: &str, query: &str, body: &[u8]) -> Vec<AnalyticsEvent> {
    let profile = path.trim_end_matches('/').ends_with("/engage");
    // The data is base64 JSON in `data`, in the form body or the query; the newer clients send plain JSON.
    let form = String::from_utf8_lossy(body);
    let data = form
        .split('&')
        .find_map(|p| p.strip_prefix("data="))
        .map(percent_decode)
        .or_else(|| query_param(query, "data"));
    let value: Option<Value> = match data {
        Some(d) => {
            let engine = base64::engine::general_purpose::STANDARD;
            engine
                .decode(d.trim())
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .or_else(|| serde_json::from_str(&d).ok())
        }
        None => serde_json::from_slice(body).ok(),
    };
    let Some(value) = value else { return Vec::new() };
    let items: Vec<Value> = match value {
        Value::Array(a) => a,
        other => vec![other],
    };
    items
        .iter()
        .filter_map(|item| {
            if profile {
                let user = text(item, "$distinct_id");
                let mut props = Map::new();
                for key in ["$set", "$set_once", "$add", "$append", "$unset", "$delete"] {
                    if let Some(v) = item.get(key) {
                        props.insert(key.into(), v.clone());
                    }
                }
                let name = props.keys().next().map_or("profile", |k| k.as_str()).trim_start_matches('$').to_string();
                return Some(AnalyticsEvent {
                    service: Service::Mixpanel,
                    kind: "profile".into(),
                    name,
                    properties: Value::Object(props),
                    user,
                    time_ms: None,
                });
            }
            let name = text(item, "event")?;
            let props = item.get("properties").cloned().unwrap_or(Value::Null);
            let user = props.get("distinct_id").and_then(Value::as_str).map(str::to_string);
            // Mixpanel's time is in seconds.
            let time_ms = props
                .get("time")
                .and_then(Value::as_f64)
                .map(|t| if t < 1e12 { (t * 1000.0) as u64 } else { t as u64 });
            Some(AnalyticsEvent {
                service: Service::Mixpanel,
                kind: "track".into(),
                name,
                properties: props,
                user,
                time_ms,
            })
        })
        .collect()
}

// ---- Firebase Analytics ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum Wire<'a> {
    Varint(u64),
    Fixed64(u64),
    Bytes(&'a [u8]),
    Fixed32(u32),
}

/// The fields of one protobuf message, in order. A message that does not parse gives what was read before the mistake.
fn fields(mut data: &[u8]) -> Vec<(u32, Wire<'_>)> {
    fn varint(data: &mut &[u8]) -> Option<u64> {
        let mut value = 0u64;
        for shift in (0..70).step_by(7) {
            let (&byte, rest) = data.split_first()?;
            *data = rest;
            value |= u64::from(byte & 0x7f).checked_shl(shift)?;
            if byte & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    }
    let mut out = Vec::new();
    while !data.is_empty() {
        let Some(key) = varint(&mut data) else { break };
        let (number, wire) = ((key >> 3) as u32, key & 7);
        let value = match wire {
            0 => varint(&mut data).map(Wire::Varint),
            1 if data.len() >= 8 => {
                let v = u64::from_le_bytes(data[..8].try_into().expect("8 bytes"));
                data = &data[8..];
                Some(Wire::Fixed64(v))
            }
            2 => varint(&mut data).and_then(|n| {
                let n = usize::try_from(n).ok()?;
                (data.len() >= n).then(|| {
                    let (head, rest) = data.split_at(n);
                    data = rest;
                    Wire::Bytes(head)
                })
            }),
            5 if data.len() >= 4 => {
                let v = u32::from_le_bytes(data[..4].try_into().expect("4 bytes"));
                data = &data[4..];
                Some(Wire::Fixed32(v))
            }
            _ => None,
        };
        match value {
            Some(v) => out.push((number, v)),
            None => break,
        }
    }
    out
}

fn utf8(w: Wire<'_>) -> Option<String> {
    match w {
        Wire::Bytes(b) => std::str::from_utf8(b).ok().map(str::to_string),
        _ => None,
    }
}

/// Firebase's measurement protocol: a batch holds bundles (field 1), a bundle holds events (field 2) and the app id (14),
/// an event holds its parameters (1), name (2) and time (3), and a parameter holds a name (1) and a string (2), integer
/// (3), float (4) or double (5) value.
fn firebase(body: &[u8]) -> Vec<AnalyticsEvent> {
    let mut out = Vec::new();
    for (n, bundle) in fields(body) {
        let Wire::Bytes(bundle) = bundle else { continue };
        if n != 1 {
            continue;
        }
        let parts = fields(bundle);
        let app = parts.iter().find(|(f, _)| *f == 14).and_then(|(_, w)| utf8(*w));
        let user = parts.iter().find(|(f, _)| *f == 21).and_then(|(_, w)| utf8(*w)).or(app);
        for (f, w) in &parts {
            let (2, Wire::Bytes(event)) = (*f, *w) else { continue };
            let mut name = None;
            let mut time = None;
            let mut props = Map::new();
            for (ef, ew) in fields(event) {
                match (ef, ew) {
                    (2, w) => name = utf8(w),
                    (3, Wire::Varint(t)) => time = Some(t),
                    (1, Wire::Bytes(param)) => {
                        let mut pname = None;
                        let mut pvalue = Value::Null;
                        for (pf, pw) in fields(param) {
                            match (pf, pw) {
                                (1, w) => pname = utf8(w),
                                (2, w) => pvalue = utf8(w).map_or(Value::Null, Value::String),
                                (3, Wire::Varint(v)) => pvalue = json!(v as i64),
                                (4, Wire::Fixed32(v)) => pvalue = json!(f64::from(f32::from_bits(v))),
                                (5, Wire::Fixed64(v)) => pvalue = json!(f64::from_bits(v)),
                                _ => {}
                            }
                        }
                        if let Some(k) = pname {
                            props.insert(k, pvalue);
                        }
                    }
                    _ => {}
                }
            }
            if let Some(name) = name {
                let kind = if name == "screen_view" { "screen" } else { "track" };
                out.push(AnalyticsEvent {
                    service: Service::Firebase,
                    kind: kind.into(),
                    name,
                    properties: Value::Object(props),
                    user: user.clone(),
                    time_ms: time,
                });
            }
        }
    }
    out
}

/// A line of the device log that Firebase Analytics wrote in debug mode
/// (`adb shell setprop debug.firebase.analytics.app <package>`): `Logging event: origin=app,name=screen_view,params=Bundle[{a=b}]`.
pub fn from_log_line(tag: &str, message: &str) -> Option<AnalyticsEvent> {
    if !tag.starts_with("FA") {
        return None;
    }
    let rest = message.split("Logging event").nth(1)?;
    let name = rest.split("name=").nth(1)?.split(',').next()?.trim().to_string();
    let params = rest.split("params=Bundle[{").nth(1).map(|p| p.trim_end_matches(['}', ']', ' ']));
    let mut props = Map::new();
    for pair in params.unwrap_or("").split(", ") {
        if let Some((k, v)) = pair.split_once('=') {
            props.insert(k.trim().to_string(), Value::String(v.trim().to_string()));
        }
    }
    let kind = if name == "screen_view" { "screen" } else { "track" };
    Some(AnalyticsEvent {
        service: Service::Firebase,
        kind: kind.into(),
        name,
        properties: Value::Object(props),
        user: None,
        time_ms: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::FlowKind;

    fn flow(host: &str, path: &str, content_type: &str, body: &[u8]) -> Flow {
        let mut f = Flow::new(FlowKind::Http, "POST", "https", host, 443, path);
        f.request_headers.push(("Content-Type".into(), content_type.into()));
        f.request_body = body.to_vec();
        f
    }

    #[test]
    fn hosts_are_told_to_their_service() {
        assert_eq!(detect("api.segment.io"), Some(Service::Segment));
        assert_eq!(detect("api2.amplitude.com"), Some(Service::Amplitude));
        assert_eq!(detect("api-eu.mixpanel.com"), Some(Service::Mixpanel));
        assert_eq!(detect("app-measurement.com"), Some(Service::Firebase));
        assert_eq!(detect("notsegment.io"), None);
        assert_eq!(detect("example.com"), None);
        assert_eq!(Service::parse("Mixpanel"), Some(Service::Mixpanel));
    }

    #[test]
    fn segment_calls_and_batches_are_read() {
        let one = br#"{"event":"Order Completed","userId":"u1","properties":{"total":42.5},"timestamp":"2026-11-07T20:54:21.500Z"}"#;
        let got = events(&flow("api.segment.io", "/v1/track", "application/json", one));
        assert_eq!(got.len(), 1);
        assert_eq!(
            (got[0].kind.as_str(), got[0].name.as_str(), got[0].user.as_deref()),
            ("track", "Order Completed", Some("u1"))
        );
        assert_eq!(got[0].properties["total"], 42.5);
        assert_eq!(got[0].time_ms, Some(1_794_084_861_500));

        let batch = br#"{"batch":[{"type":"screen","name":"Home","anonymousId":"a1"},{"type":"identify","userId":"u1","traits":{"plan":"pro"}},{"type":"track","event":"Tap"}]}"#;
        let got = events(&flow("api.segment.io", "/v1/batch", "application/json", batch));
        let names: Vec<_> = got.iter().map(|e| (e.kind.as_str(), e.name.as_str())).collect();
        assert_eq!(names, [("screen", "Home"), ("identify", "identify"), ("track", "Tap")]);
        assert_eq!(got[0].user.as_deref(), Some("a1"));
        assert_eq!(got[1].properties["plan"], "pro");
        assert!(events(&flow("api.segment.io", "/v1/track", "application/json", b"not json")).is_empty());
    }

    #[test]
    fn amplitude_json_and_form_bodies_are_read() {
        let json = br#"{"api_key":"k","events":[{"event_type":"Added to cart","user_id":"u9","time":1794084861000,"event_properties":{"sku":"APPLE"},"user_properties":{"vip":true}}]}"#;
        let got = events(&flow("api2.amplitude.com", "/2/httpapi", "application/json", json));
        assert_eq!(got.len(), 1);
        assert_eq!(
            (got[0].name.as_str(), got[0].user.as_deref(), got[0].time_ms),
            ("Added to cart", Some("u9"), Some(1_794_084_861_000))
        );
        assert_eq!(got[0].properties["sku"], "APPLE");
        assert_eq!(got[0].properties["$user_properties"]["vip"], true);

        let form = b"client=k&v=2&e=%5B%7B%22event_type%22%3A%22Open%22%2C%22device_id%22%3A%22d1%22%7D%5D";
        let got = events(&flow("api.amplitude.com", "/", "application/x-www-form-urlencoded", form));
        assert_eq!((got[0].name.as_str(), got[0].user.as_deref()), ("Open", Some("d1")));
    }

    #[test]
    fn mixpanel_base64_data_and_profiles_are_read() {
        let track = br#"[{"event":"Signed Up","properties":{"distinct_id":"u3","time":1794084861,"plan":"free"}}]"#;
        let data = base64::engine::general_purpose::STANDARD.encode(track);
        let body = format!("data={}", data.replace('+', "%2B").replace('=', "%3D"));
        let got = events(&flow("api.mixpanel.com", "/track", "application/x-www-form-urlencoded", body.as_bytes()));
        assert_eq!(got.len(), 1);
        assert_eq!(
            (got[0].name.as_str(), got[0].user.as_deref(), got[0].time_ms),
            ("Signed Up", Some("u3"), Some(1_794_084_861_000))
        );
        assert_eq!(got[0].properties["plan"], "free");

        let profile =
            base64::engine::general_purpose::STANDARD.encode(br#"{"$distinct_id":"u3","$set":{"name":"Ana"}}"#);
        let got = events(&flow(
            "api.mixpanel.com",
            "/engage",
            "application/x-www-form-urlencoded",
            format!("data={profile}").as_bytes(),
        ));
        assert_eq!(
            (got[0].kind.as_str(), got[0].name.as_str(), got[0].user.as_deref()),
            ("profile", "set", Some("u3"))
        );
        assert_eq!(got[0].properties["$set"]["name"], "Ana");

        // Plain JSON and a query string work too.
        let got = events(&flow("api.mixpanel.com", "/track?data=eyJldmVudCI6IlgifQ==", "", b""));
        assert_eq!(got[0].name, "X");
    }

    fn varint(mut v: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                out.push(b);
                return out;
            }
            out.push(b | 0x80);
        }
    }

    fn field_bytes(n: u32, data: &[u8]) -> Vec<u8> {
        let mut out = varint(u64::from(n) << 3 | 2);
        out.extend(varint(data.len() as u64));
        out.extend(data);
        out
    }

    fn field_varint(n: u32, v: u64) -> Vec<u8> {
        let mut out = varint(u64::from(n) << 3);
        out.extend(varint(v));
        out
    }

    #[test]
    fn firebase_batches_are_read() {
        let param = |name: &str, value: Vec<u8>| {
            let mut p = field_bytes(1, name.as_bytes());
            p.extend(value);
            field_bytes(1, &p)
        };
        let mut event = param("firebase_screen", field_bytes(2, b"Home"));
        event.extend(param("count", field_varint(3, 7)));
        let mut ratio = varint(5 << 3 | 1);
        ratio.extend(0.5f64.to_le_bytes());
        event.extend(param("ratio", ratio));
        event.extend(field_bytes(2, b"screen_view"));
        event.extend(field_varint(3, 1_794_084_861_000));
        let mut bundle = field_bytes(2, &event);
        bundle.extend(field_bytes(2, &field_bytes(2, b"app_open")));
        bundle.extend(field_bytes(14, b"com.example.shop"));
        let body = field_bytes(1, &bundle);

        let got = events(&flow("app-measurement.com", "/a", "application/x-protobuf", &body));
        assert_eq!(got.len(), 2);
        assert_eq!((got[0].kind.as_str(), got[0].name.as_str()), ("screen", "screen_view"));
        assert_eq!(got[0].properties["firebase_screen"], "Home");
        assert_eq!(got[0].properties["count"], 7);
        assert_eq!(got[0].properties["ratio"], 0.5);
        assert_eq!(got[0].time_ms, Some(1_794_084_861_000));
        assert_eq!(got[0].user.as_deref(), Some("com.example.shop"));
        assert_eq!(got[1].name, "app_open");
        // Damaged bytes give what could be read, and never a panic.
        assert!(events(&flow("app-measurement.com", "/a", "x", &body[..body.len() / 2])).len() <= 2);
        assert!(events(&flow("app-measurement.com", "/a", "x", b"\xff\xff\xff")).is_empty());
    }

    #[test]
    fn firebase_debug_log_lines_are_read() {
        let line = "Logging event: origin=app,name=add_to_cart,params=Bundle[{item_id=SKU1, quantity=2}]";
        let e = from_log_line("FA", line).unwrap();
        assert_eq!(e.name, "add_to_cart");
        assert_eq!(e.properties["item_id"], "SKU1");
        assert_eq!(e.properties["quantity"], "2");
        assert!(from_log_line("OkHttp", line).is_none());
        assert!(from_log_line("FA", "Some other FA line").is_none());
    }
}
