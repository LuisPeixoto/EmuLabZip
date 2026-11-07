//! `emulabzip analytics`: the events an app sends to Segment, Amplitude, Mixpanel and Firebase Analytics.

use anyhow::{Context, Result, bail};
use emulabzip_core::{Lab, LogOptions};
use emulabzip_net::analytics::{self, AnalyticsEvent, Service};
use emulabzip_net::flow::Flow;

use super::net::{ConditionArgs, NetArgs};
use crate::output;

/// One line for an event: service, kind, name, user and its properties.
pub fn event_line(e: &AnalyticsEvent) -> String {
    let props = match &e.properties {
        serde_json::Value::Null => String::new(),
        v => v.to_string(),
    };
    let props =
        if props.chars().count() > 110 { format!("{}…", props.chars().take(110).collect::<String>()) } else { props };
    format!(
        "{:<9} {:<8} {}  {}  {}",
        e.service.label(),
        e.kind,
        e.name,
        output::dim(e.user.as_deref().unwrap_or("")),
        output::dim(&props)
    )
}

pub fn print_flow(flow: &Flow, services: &[Service], json: bool) -> Result<()> {
    for e in analytics::events(flow).iter().filter(|e| services.is_empty() || services.contains(&e.service)) {
        if json {
            println!("{}", serde_json::to_string(e)?);
        } else {
            println!("{}", event_line(e));
        }
    }
    Ok(())
}

pub struct Args {
    pub device: Option<String>,
    pub port: u16,
    pub no_connect: bool,
    pub services: Vec<String>,
    pub json: bool,
    /// Follow the device log for Firebase's debug-mode lines instead of running the proxy.
    pub firebase_log: bool,
}

pub fn parse_services(names: &[String]) -> Result<Vec<Service>> {
    names
        .iter()
        .map(|n| {
            Service::parse(n)
                .with_context(|| format!("`{n}` is not a service: use segment, amplitude, mixpanel or firebase"))
        })
        .collect()
}

pub async fn run(lab: &Lab, args: Args) -> Result<()> {
    let services = parse_services(&args.services)?;
    if args.firebase_log {
        return firebase_log(lab, args.device.as_deref(), &services, args.json).await;
    }
    let net = NetArgs {
        device: args.device,
        port: args.port,
        no_connect: args.no_connect,
        har: None,
        json: args.json,
        rules: None,
        http3: false,
        show_h3: false,
        conditions: ConditionArgs {
            preset: None,
            down_kbps: None,
            up_kbps: None,
            latency_ms: None,
            loss_percent: None,
        },
        replay: None,
        analytics: Some(services),
    };
    super::net::run(lab, net).await
}

/// Firebase Analytics writes each event to the log under the tag `FA` when debug mode is on for the app:
/// `adb shell setprop debug.firebase.analytics.app <package>`.
async fn firebase_log(lab: &Lab, device: Option<&str>, services: &[Service], json: bool) -> Result<()> {
    if !services.is_empty() && !services.contains(&Service::Firebase) {
        bail!("the log only has Firebase events");
    }
    let device = lab.resolve(device).await?;
    let mut stream = lab.logs(&device.target(), LogOptions { tail: 0, verbose: true }).await?;
    if !json {
        eprintln!(
            "{}",
            output::dim(
                "Listening for Firebase debug events. Turn debug mode on with: adb shell setprop debug.firebase.analytics.app <package>"
            )
        );
    }
    loop {
        tokio::select! {
            entry = stream.next() => {
                let Some(entry) = entry else { return Ok(()) };
                if let Some(e) = analytics::from_log_line(&entry.tag, &entry.message) {
                    if json { println!("{}", serde_json::to_string(&e)?) } else { println!("{}", event_line(&e)) }
                }
            }
            _ = tokio::signal::ctrl_c() => return Ok(()),
        }
    }
}
