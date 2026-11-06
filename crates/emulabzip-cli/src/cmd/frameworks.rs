//! `emulabzip frameworks`, `emulabzip flutter` and `emulabzip rn`

use anyhow::{Result, bail};
use emulabzip_core::control::is_app_id;
use emulabzip_core::{Framework, Lab, Target};
use emulabzip_frameworks::flutter::{Flutter, FlutterMemory, FrameStats, VmUri, WidgetNode, find_uri};
use emulabzip_frameworks::react_native::{
    ConsoleLine, HermesHeap, METRO_PORT, ReactNative, ReactNode, list_targets, pick,
};

use crate::output;

/// The longest a measurement runs, so a typo cannot hold the terminal for an hour.
pub const MAX_SECONDS: u64 = 300;

fn check_app(app: &str) -> Result<()> {
    if !is_app_id(app) {
        bail!("`{app}` is not a package name or bundle id");
    }
    Ok(())
}

fn check_seconds(seconds: u64) -> Result<()> {
    if seconds == 0 || seconds > MAX_SECONDS {
        bail!("--seconds is between 1 and {MAX_SECONDS}");
    }
    Ok(())
}

fn check_uri(uri: Option<&str>) -> Result<Option<VmUri>> {
    uri.map(|u| {
        VmUri::parse(u).ok_or_else(|| anyhow::anyhow!("`{u}` is not an address like http://127.0.0.1:41233/token=/"))
    })
    .transpose()
}

pub fn print_frameworks(app: &str, found: &[Framework]) {
    if found.is_empty() {
        println!("{app}: no Flutter, React Native, Compose or SwiftUI found. It looks like a plain native app.");
        return;
    }
    println!("{}", output::heading(app));
    for f in found {
        let version = f.version.as_deref().map(|v| format!(" {v}")).unwrap_or_default();
        println!("  {}{version}  {}", f.name, output::dim(&f.evidence));
    }
}

pub async fn detect(lab: &Lab, device: Option<&str>, app: &str, json: bool) -> Result<()> {
    check_app(app)?;
    let device = lab.resolve(device).await?;
    let found = lab.frameworks(&device.target(), app).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&found)?);
    } else {
        print_frameworks(app, &found);
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Flutter
// ---------------------------------------------------------------------------------------------

async fn flutter_connect(lab: &Lab, device: Option<&str>, uri: Option<VmUri>) -> Result<(Flutter, Target)> {
    let device = lab.resolve(device).await?;
    let target = device.target();
    let uri = match uri {
        Some(uri) => uri,
        None => {
            let mut messages = Vec::new();
            for needle in ["Dart VM service", "Observatory listening"] {
                messages.extend(lab.recent_logs(&target, needle).await?.into_iter().map(|e| e.message));
            }
            find_uri(messages.iter().map(String::as_str)).ok_or_else(|| {
                anyhow::anyhow!(
                    "Could not find where the Dart VM service listens. Is a debug build running? \
                     Start the app again, or pass its address with --uri"
                )
            })?
        }
    };
    let stream = lab.connect_port(&target, uri.port).await?;
    let flutter = Flutter::connect(stream, &uri).await?;
    Ok((flutter, target))
}

fn print_widget(node: &WidgetNode, depth: usize, max_depth: usize, out: &mut Vec<String>) {
    let place = match (&node.file, node.line) {
        (Some(file), Some(line)) if node.local => format!("  {}", output::dim(&format!("{file}:{line}"))),
        _ => String::new(),
    };
    let marker = if node.local { "●" } else { "○" };
    out.push(format!("{}{marker} {}{place}", "  ".repeat(depth), node.description));
    if depth >= max_depth {
        if !node.children.is_empty() {
            let hidden: usize = node.children.iter().map(WidgetNode::count).sum();
            out.push(format!("{}  {}", "  ".repeat(depth), output::dim(&format!("… {hidden} more, see --depth"))));
        }
        return;
    }
    for child in &node.children {
        print_widget(child, depth + 1, max_depth, out);
    }
}

pub async fn flutter_tree(
    lab: &Lab,
    device: Option<&str>,
    app: &str,
    uri: Option<&str>,
    depth: usize,
    json: bool,
) -> Result<()> {
    check_app(app)?;
    let uri = check_uri(uri)?;
    let (flutter, _) = flutter_connect(lab, device, uri).await?;
    let tree = flutter.widget_tree().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&tree)?);
    } else {
        let mut lines = Vec::new();
        print_widget(&tree, 0, depth, &mut lines);
        println!("{}", lines.join("\n"));
        println!(
            "\n{}",
            output::dim(&format!("{} widgets. ● made by the app's own code, ○ by Flutter or a package.", tree.count()))
        );
        if !flutter.creation_tracked() {
            println!(
                "{}",
                output::dim(
                    "This build does not record where widgets were created, so there are no file names and the app's own widgets cannot be told apart. Apps started with `flutter run` do; for `flutter build` add --extra-front-end-options=--track-widget-creation."
                )
            );
        }
    }
    Ok(())
}

pub fn print_frames(s: &FrameStats) {
    if s.frames == 0 {
        println!(
            "No frames were drawn in {} s. Use the app while it measures: an idle screen draws nothing.",
            s.seconds
        );
        return;
    }
    println!(
        "{} frames in {} s: {} janky ({:.1}%), build {:.1} ms and raster {:.1} ms on average, worst {:.1} ms",
        s.frames,
        s.seconds,
        s.janky,
        s.jank_percent,
        s.avg_build_us as f64 / 1000.0,
        s.avg_raster_us as f64 / 1000.0,
        s.worst_us as f64 / 1000.0
    );
    println!("\n{}", output::heading("Slowest frames"));
    for f in &s.slowest {
        println!(
            "  #{:<6} build {:>6.1} ms  raster {:>6.1} ms",
            f.number,
            f.build_us as f64 / 1000.0,
            f.raster_us as f64 / 1000.0
        );
    }
}

pub async fn flutter_frames(
    lab: &Lab,
    device: Option<&str>,
    app: &str,
    uri: Option<&str>,
    seconds: u64,
    json: bool,
) -> Result<()> {
    check_app(app)?;
    check_seconds(seconds)?;
    let uri = check_uri(uri)?;
    let (flutter, _) = flutter_connect(lab, device, uri).await?;
    eprintln!("{}", output::dim(&format!("Recording frames for {seconds} s. Use the app now.")));
    let stats = flutter.frames(seconds).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&stats)?);
    } else {
        print_frames(&stats);
    }
    Ok(())
}

pub fn print_flutter_memory(m: &FlutterMemory) {
    println!(
        "Dart heap: {} used of {}, {} outside the heap",
        output::size(m.heap_used),
        output::size(m.heap_capacity),
        output::size(m.external)
    );
    println!("\n{}", output::heading("Classes using the most"));
    for c in &m.classes {
        println!("  {:>9}  {:>8}×  {}", output::size(c.bytes), c.instances, c.name);
    }
}

pub async fn flutter_memory(
    lab: &Lab,
    device: Option<&str>,
    app: &str,
    uri: Option<&str>,
    top: usize,
    json: bool,
) -> Result<()> {
    check_app(app)?;
    let uri = check_uri(uri)?;
    let (flutter, _) = flutter_connect(lab, device, uri).await?;
    let memory = flutter.memory(top.clamp(1, 200)).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&memory)?);
    } else {
        print_flutter_memory(&memory);
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// React Native
// ---------------------------------------------------------------------------------------------

async fn rn_connect(app: &str, metro: u16) -> Result<ReactNative> {
    check_app(app)?;
    let targets = list_targets(metro).await?;
    let Some(target) = pick(&targets, app) else {
        let known: Vec<String> = targets.iter().filter_map(|t| t.app_id.clone()).collect();
        bail!(
            "Metro does not list a runtime for {app}. It lists: {}. Open the app on a device that reaches Metro.",
            if known.is_empty() { "nothing".to_string() } else { known.join(", ") }
        );
    };
    eprintln!("{}", output::dim("Metro lets one debugger connect at a time: this may close React Native DevTools."));
    Ok(ReactNative::connect(metro, target).await?)
}

fn print_react(node: &ReactNode, depth: usize, max_depth: usize, out: &mut Vec<String>) {
    let props = if node.props.is_empty() { String::new() } else { format!("  {}", output::dim(&node.props.join(" "))) };
    let marker = if node.host { "○" } else { "●" };
    out.push(format!("{}{marker} {}{props}", "  ".repeat(depth), node.name));
    if depth >= max_depth {
        if !node.children.is_empty() {
            let hidden: usize = node.children.iter().map(ReactNode::count).sum();
            out.push(format!("{}  {}", "  ".repeat(depth), output::dim(&format!("… {hidden} more, see --depth"))));
        }
        return;
    }
    for child in &node.children {
        print_react(child, depth + 1, max_depth, out);
    }
}

pub async fn rn_tree(app: &str, metro: u16, depth: usize, json: bool) -> Result<()> {
    let rn = rn_connect(app, metro).await?;
    let tree = rn.tree().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&tree)?);
    } else {
        let mut lines = Vec::new();
        print_react(&tree, 0, depth, &mut lines);
        println!("{}", lines.join("\n"));
        println!("\n{}", output::dim(&format!("{} components. ● components, ○ native views.", tree.count())));
    }
    Ok(())
}

pub fn print_heap(h: &HermesHeap) {
    println!("Hermes heap: {} used of {}", output::size(h.used_bytes), output::size(h.total_bytes));
    if let Some(runtime) = &h.runtime {
        println!("{}", output::dim(&format!("Hermes {runtime}")));
    }
}

pub async fn rn_heap(app: &str, metro: u16, json: bool) -> Result<()> {
    let rn = rn_connect(app, metro).await?;
    let heap = rn.heap().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&heap)?);
    } else {
        print_heap(&heap);
    }
    Ok(())
}

pub fn print_console(lines: &[ConsoleLine]) {
    if lines.is_empty() {
        println!("Nothing was written to the console.");
    }
    for l in lines {
        println!("{:<6} {}", l.level, l.text);
    }
}

pub async fn rn_console(app: &str, metro: u16, seconds: u64, json: bool) -> Result<()> {
    check_seconds(seconds)?;
    let rn = rn_connect(app, metro).await?;
    eprintln!("{}", output::dim(&format!("Listening to the console for {seconds} s.")));
    let lines = rn.console(seconds).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&lines)?);
    } else {
        print_console(&lines);
    }
    Ok(())
}

pub const DEFAULT_METRO: u16 = METRO_PORT;
