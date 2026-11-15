//! Things an assistant can read without calling a tool: the devices, sessions, flows, the live network capture and a
//! guide. Every id in a URI is checked like the tools check theirs, so a URI can never name a path.

use rmcp::model::{
    ListResourceTemplatesResult, ListResourcesResult, ReadResourceResult, Resource, ResourceContents, ResourceTemplate,
};

use crate::server::EmuLabServer;

pub(crate) const GUIDE: &str = include_str!("guide.md");
const SCHEME: &str = "emulabzip://";
const JSON: &str = "application/json";

pub(crate) fn list() -> ListResourcesResult {
    let r = |uri: &str, name: &str, description: &str, mime: &str| {
        Resource::new(format!("{SCHEME}{uri}"), name).with_description(description).with_mime_type(mime)
    };
    ListResourcesResult::with_all_items(vec![
        r("guide", "Guide", "How to use EmuLabZip: the tools by task, and what is safe", "text/markdown"),
        r("devices", "Devices", "The running devices", JSON),
        r("sessions", "Sessions", "Recorded sessions, newest first", JSON),
        r("flows", "Flows", "Saved flows", JSON),
        r("states", "Saved states", "App states saved for later", JSON),
        r("links/history", "Deep link history", "Links opened and checked before", JSON),
        r("rules", "Network rules", "The rules the proxy applies", JSON),
        r("network/flows", "Captured requests", "What the proxy captured in this session", JSON),
    ])
}

pub(crate) fn templates() -> ListResourceTemplatesResult {
    let t = |uri: &str, name: &str, description: &str, mime: &str| {
        ResourceTemplate::new(format!("{SCHEME}{uri}"), name).with_description(description).with_mime_type(mime)
    };
    ListResourceTemplatesResult::with_all_items(vec![
        t("sessions/{id}", "Session summary", "A session's summary", JSON),
        t("flows/{id}", "Flow", "A saved flow's steps", JSON),
        t("flows/{id}/maestro", "Flow as Maestro", "A flow as a Maestro YAML flow", "text/yaml"),
        t("flows/{id}/script", "Flow as script", "A flow as a shell script", "text/x-shellscript"),
        t("network/flows/{id}", "Captured request", "One captured request in full", JSON),
    ])
}

impl EmuLabServer {
    /// The text and type of a resource, or why it cannot be read.
    pub(crate) async fn read_uri(&self, uri: &str) -> Result<ReadResourceResult, String> {
        let path = uri.strip_prefix(SCHEME).ok_or_else(|| format!("`{uri}` is not an {SCHEME} address"))?;
        let parts: Vec<&str> = path.split('/').collect();
        let (text, mime) = match parts.as_slice() {
            ["guide"] => (GUIDE.to_string(), "text/markdown"),
            ["devices"] => {
                let (devices, _) = self.lab.devices().await;
                (to_json(&devices)?, JSON)
            }
            ["sessions"] => (crate::server::sessions_list()?, JSON),
            ["sessions", id] => (crate::server::session_summary(id)?, JSON),
            ["flows"] => (to_json(&emulabzip_core::automation::list())?, JSON),
            ["flows", id] => (to_json(&flow(id)?)?, JSON),
            ["flows", id, "maestro"] => (flow(id)?.to_maestro(), "text/yaml"),
            ["flows", id, "script"] => (flow(id)?.to_script(), "text/x-shellscript"),
            ["states"] => (crate::server::states_list()?, JSON),
            ["links", "history"] => (to_json(&emulabzip_core::links::load_history())?, JSON),
            ["rules"] => (to_json(&self.hub.net.lock().await.rules.rules())?, JSON),
            ["network", "flows"] => (to_json(&self.hub.flows.summaries())?, JSON),
            ["network", "flows", id] => {
                let id: u64 = id.parse().map_err(|_| format!("`{id}` is not a request number"))?;
                let flow = self.hub.flows.get(id).ok_or_else(|| format!("no request is numbered {id}"))?;
                (to_json(&flow)?, JSON)
            }
            _ => return Err(format!("there is no resource at {uri}")),
        };
        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(crate::server::cap(text, 200_000), uri).with_mime_type(mime),
        ]))
    }
}

fn flow(id: &str) -> Result<emulabzip_core::automation::Flow, String> {
    if !crate::server::valid_session_id(id) {
        return Err(format!("`{id}` is not a flow name"));
    }
    emulabzip_core::automation::load(id).map_err(|e| e.to_string())
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string_pretty(value).map_err(|e| e.to_string())
}
