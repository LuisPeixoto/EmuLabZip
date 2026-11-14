//! `emulabzip webview`: the pages inside an app's WebView, or in Chrome, on an Android device.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, bail};
use emulabzip_core::backend::is_socket_name;
use emulabzip_core::control::is_app_id;
use emulabzip_core::{DevtoolsSocket, Lab, Target};
use emulabzip_frameworks::webview::{self, MAX_EXPRESSION, Page, WebLine, WebView};
use emulabzip_net::cdp::CdpFlows;
use emulabzip_net::{FlowStore, to_har};

use crate::cmd::frameworks::MAX_SECONDS;
use crate::output;

/// Which socket and page to look at.
pub struct Pick<'a> {
    pub app: Option<&'a str>,
    pub socket: Option<&'a str>,
    pub page: Option<&'a str>,
}

/// What can be checked without a device, so a typo fails before anything is looked up.
pub fn check(pick: &Pick<'_>, seconds: Option<u64>, expression: Option<&str>) -> Result<()> {
    if let Some(app) = pick.app
        && !is_app_id(app)
    {
        bail!("`{app}` is not a package name or bundle id");
    }
    if let Some(socket) = pick.socket
        && !is_socket_name(socket)
    {
        bail!("`{socket}` is not a socket name like webview_devtools_remote_1234");
    }
    if let Some(s) = seconds
        && (s == 0 || s > MAX_SECONDS)
    {
        bail!("--seconds is between 1 and {MAX_SECONDS}");
    }
    if let Some(e) = expression
        && (e.trim().is_empty() || e.len() > MAX_EXPRESSION)
    {
        bail!("the expression is between 1 and {MAX_EXPRESSION} characters");
    }
    Ok(())
}

async fn open(lab: &Lab, target: &Target, pick: &Pick<'_>) -> Result<(DevtoolsSocket, Page, WebView)> {
    Ok(webview::connect(lab, target, pick.app, pick.socket, pick.page).await?)
}

pub async fn list(lab: &Lab, device: Option<&str>, json: bool) -> Result<()> {
    let device = lab.resolve(device).await?;
    let target = device.target();
    let sockets = lab.devtools_sockets(&target).await?;
    let mut out = Vec::new();
    for s in &sockets {
        let pages = webview::pages(lab, &target, &s.socket).await.unwrap_or_default();
        out.push((s.clone(), pages));
    }
    if json {
        let value: Vec<_> = out.iter().map(|(s, p)| serde_json::json!({ "socket": s, "pages": p })).collect();
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }
    if out.is_empty() {
        println!("No WebView or browser can be inspected on {}.", device.name);
        println!(
            "{}",
            output::dim("A WebView needs WebView.setWebContentsDebuggingEnabled(true) in a debuggable app.")
        );
    }
    for (s, pages) in &out {
        println!("{}  {}", output::heading(&s.socket), output::dim(s.app.as_deref().unwrap_or(&s.kind)));
        for p in pages {
            println!("  {}  {}  {}", &p.id[..p.id.len().min(8)], p.title, output::dim(&p.url));
        }
    }
    Ok(())
}

pub fn print_line(l: &WebLine) {
    let src = l.source.as_deref().map(|s| format!("  {}", output::dim(s))).unwrap_or_default();
    println!("[{:<5}] {}{src}", l.level, l.text);
}

pub async fn console(lab: &Lab, device: Option<&str>, pick: Pick<'_>, seconds: u64, json: bool) -> Result<()> {
    check(&pick, Some(seconds), None)?;
    let device = lab.resolve(device).await?;
    let (socket, page, view) = open(lab, &device.target(), &pick).await?;
    eprintln!("{}", output::dim(&format!("Listening to {} on {} for {seconds} s.", page.url, socket.socket)));
    let lines = view.console(seconds, 1000).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&lines)?);
    } else if lines.is_empty() {
        println!("{}", output::dim("Nothing was written to the console."));
    } else {
        lines.iter().for_each(print_line);
    }
    Ok(())
}

pub async fn eval(lab: &Lab, device: Option<&str>, pick: Pick<'_>, expression: &str, json: bool) -> Result<()> {
    check(&pick, None, Some(expression))?;
    let device = lab.resolve(device).await?;
    let (_, _, view) = open(lab, &device.target(), &pick).await?;
    let result = view.evaluate(expression).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("{}", result.value);
    }
    if result.error {
        bail!("the expression threw");
    }
    Ok(())
}

pub async fn network(
    lab: &Lab,
    device: Option<&str>,
    pick: Pick<'_>,
    seconds: u64,
    reload: bool,
    har: Option<&std::path::Path>,
    json: bool,
) -> Result<()> {
    check(&pick, Some(seconds), None)?;
    let device = lab.resolve(device).await?;
    let (_, page, view) = open(lab, &device.target(), &pick).await?;
    let store: Arc<FlowStore> = FlowStore::new(5_000);
    let mut flows = CdpFlows::new(store.clone());
    let mut events = view.events();
    view.start(true).await?;
    if reload {
        view.reload().await?;
    }
    eprintln!("{}", output::dim(&format!("Watching the requests of {} for {seconds} s.", page.url)));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);
    loop {
        let Ok(received) = tokio::time::timeout_at(deadline, events.recv()).await else { break };
        let Ok(event) = received else { break };
        if let Some(need) = flows.on_event(&event)
            && let Ok((body, base64)) = view.response_body(&need.request_id).await
        {
            flows.set_body(need.flow, &body, base64);
        }
    }
    let all = store.all();
    if let Some(path) = har {
        std::fs::write(path, serde_json::to_string_pretty(&to_har(&all))?)?;
        eprintln!("Saved {} requests to {}.", all.len(), path.display());
    }
    if json {
        let summaries: Vec<_> = all.iter().map(|f| f.summary()).collect();
        println!("{}", serde_json::to_string_pretty(&summaries)?);
    } else if all.is_empty() {
        println!("{}", output::dim("The page made no requests."));
    } else {
        all.iter().for_each(|f| println!("{}", output::flow_line(&f.summary())));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_checked_before_any_device_is_looked_up() {
        let ok = Pick { app: Some("com.example.shop"), socket: Some("webview_devtools_remote_12"), page: None };
        assert!(check(&ok, Some(10), Some("document.title")).is_ok());
        assert!(check(&Pick { app: Some("not an app"), socket: None, page: None }, None, None).is_err());
        assert!(check(&Pick { app: None, socket: Some("@x y"), page: None }, None, None).is_err());
        let none = Pick { app: None, socket: None, page: None };
        assert!(check(&none, Some(0), None).is_err());
        assert!(check(&none, Some(MAX_SECONDS + 1), None).is_err());
        assert!(check(&none, None, Some("  ")).is_err());
        assert!(check(&none, None, Some(&"x".repeat(MAX_EXPRESSION + 1))).is_err());
    }
}
