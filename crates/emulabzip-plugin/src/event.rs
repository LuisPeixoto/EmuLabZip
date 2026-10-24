//! What a plugin prints: one JSON object per line.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    /// A number over time. In `stream` mode the app draws it as a chart.
    Metric { name: String, value: f64, unit: Option<String> },
    /// Rows to show in a table. Cells are text, numbers, booleans or null.
    Table { title: Option<String>, columns: Vec<String>, rows: Vec<Vec<Value>> },
    /// Some text, shown as it is.
    Text { title: Option<String>, body: String },
    /// A line for the plugin's own log.
    Log { level: Option<String>, message: String },
    /// The plugin could not do what it was asked.
    Error { message: String },
}

/// Reads a line. Lines that are not JSON, or that name an event this version does not know, are
/// skipped (`None`), so a plugin can print progress for people or use newer events safely.
pub fn parse_line(line: &str) -> Option<Event> {
    let line = line.trim();
    if !line.starts_with('{') {
        return None;
    }
    serde_json::from_str(line).ok().filter(valid)
}

fn valid(event: &Event) -> bool {
    match event {
        Event::Metric { name, value, .. } => !name.is_empty() && value.is_finite(),
        Event::Table { columns, rows, .. } => {
            !columns.is_empty()
                && rows.iter().all(|r| r.len() <= columns.len() && r.iter().all(|c| !c.is_array() && !c.is_object()))
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_every_kind_of_event() {
        assert_eq!(
            parse_line(r#"{"type":"metric","name":"temp","value":31.5,"unit":"°C"}"#),
            Some(Event::Metric { name: "temp".into(), value: 31.5, unit: Some("°C".into()) })
        );
        assert!(matches!(
            parse_line(r#"{"type":"table","title":"t","columns":["a","b"],"rows":[[1,"x"],[null,true]]}"#),
            Some(Event::Table { .. })
        ));
        assert!(matches!(parse_line(r#"{"type":"text","body":"hello"}"#), Some(Event::Text { title: None, .. })));
        assert!(matches!(parse_line(r#"{"type":"log","level":"warn","message":"m"}"#), Some(Event::Log { .. })));
        assert_eq!(
            parse_line(r#"  {"type":"error","message":"nope"}  "#),
            Some(Event::Error { message: "nope".into() })
        );
    }

    #[test]
    fn skips_what_it_does_not_understand() {
        for line in [
            "",
            "starting up…",
            "[1,2]",
            "{",
            r#"{"type":"video","url":"x"}"#,
            r#"{"type":"metric","name":"x"}"#,
            r#"{"type":"metric","name":"","value":1}"#,
        ] {
            assert_eq!(parse_line(line), None, "{line:?}");
        }
    }

    #[test]
    fn tables_must_be_rectangular_and_flat() {
        assert_eq!(parse_line(r#"{"type":"table","columns":[],"rows":[]}"#), None);
        assert_eq!(parse_line(r#"{"type":"table","columns":["a"],"rows":[[1,2]]}"#), None, "more cells than columns");
        assert_eq!(parse_line(r#"{"type":"table","columns":["a"],"rows":[[{"x":1}]]}"#), None, "no nested values");
        assert!(
            parse_line(r#"{"type":"table","columns":["a","b"],"rows":[[1]]}"#).is_some(),
            "short rows are padded by the view"
        );
    }
}
