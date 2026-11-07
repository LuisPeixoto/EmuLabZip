//! Commands for the Audit, Size, Visual and Analytics views.

use base64::Engine;
use emulabzip_audit::collect::{Sources, collect};
use emulabzip_audit::package::{self, Diff, Package};
use emulabzip_audit::security::traffic::Request;
use emulabzip_audit::visual::{self, Comparison, Options, Region};
use emulabzip_core::{Control, StatusBar, Target, Vision, a11y, i18n};
use emulabzip_net::analytics::{self, AnalyticsEvent};
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::commands::{CmdResult, ask_open_path, err};
use crate::state::AppState;

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

// ---- Audit ----------------------------------------------------------------------------------

/// The security audit of an app. The requests the proxy has recorded so far are part of it.
#[tauri::command]
pub async fn audit_security(
    state: State<'_, AppState>,
    target: Target,
    app: String,
) -> CmdResult<emulabzip_audit::Audit> {
    if !emulabzip_core::control::is_app_id(&app) {
        return Err(format!("`{app}` is not a package name or bundle id"));
    }
    let flows = state.flows.all();
    let requests: Option<Vec<Request>> = (!flows.is_empty()).then(|| {
        flows
            .iter()
            .map(|f| Request {
                method: f.method.clone(),
                url: f.url.clone(),
                body: std::str::from_utf8(&f.request_body).ok().map(str::to_string),
            })
            .collect()
    });
    let input = collect(&state.lab, &target, &app, Sources::all(requests)).await;
    Ok(emulabzip_audit::security::audit(&input))
}

#[tauri::command]
pub async fn text_check(state: State<'_, AppState>, target: Target, rtl: bool) -> CmdResult<a11y::Audit> {
    let snapshot = state.lab.layout(&target).await.map_err(err)?;
    Ok(i18n::check(&snapshot, i18n::Options { rtl }))
}

#[derive(Serialize)]
pub struct Simulated {
    /// The changed screenshot, as a PNG in base64.
    pub png: String,
    pub audit: a11y::Audit,
}

#[tauri::command]
pub async fn colour_simulate(state: State<'_, AppState>, target: Target, vision: String) -> CmdResult<Simulated> {
    let vision = Vision::parse(&vision).ok_or("That is not a kind of colour blindness.")?;
    let snapshot = state.lab.layout(&target).await.map_err(err)?;
    let sim = emulabzip_core::vision::simulate(&snapshot, vision).map_err(err)?;
    Ok(Simulated { png: b64(&sim.png), audit: sim.audit })
}

// ---- Size -----------------------------------------------------------------------------------

#[tauri::command]
pub async fn package_pick(app: AppHandle) -> CmdResult<Option<String>> {
    Ok(ask_open_path(&app, ("Build", &["apk", "aab", "ipa"])).await.map(|p| p.display().to_string()))
}

#[derive(Serialize)]
pub struct PackageView {
    #[serde(flatten)]
    pub package: Package,
    pub largest: Vec<package::Entry>,
    pub methods: u32,
}

fn read_package(path: &str) -> CmdResult<Package> {
    let p = std::path::Path::new(path);
    let ext = p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    if !matches!(ext.as_deref(), Some("apk" | "aab" | "ipa")) {
        return Err("That is not an .apk, .aab or .ipa file.".into());
    }
    package::analyze_file(p).map_err(err)
}

#[tauri::command]
pub async fn package_analyze(path: String) -> CmdResult<PackageView> {
    tokio::task::spawn_blocking(move || {
        let mut package = read_package(&path)?;
        let largest = package.largest(30).into_iter().cloned().collect();
        let methods = package.methods();
        package.entries.clear();
        Ok(PackageView { package, largest, methods })
    })
    .await
    .map_err(err)?
}

#[tauri::command]
pub async fn package_diff(before: String, after: String) -> CmdResult<Diff> {
    tokio::task::spawn_blocking(move || {
        let mut d = package::diff(&read_package(&before)?, &read_package(&after)?);
        d.files.truncate(60);
        Ok(d)
    })
    .await
    .map_err(err)?
}

// ---- Visual ---------------------------------------------------------------------------------

fn baselines() -> std::path::PathBuf {
    emulabzip_core::data_dir().join("visual")
}

#[tauri::command]
pub async fn visual_list() -> CmdResult<Vec<(String, u64)>> {
    Ok(visual::list_baselines(&baselines()))
}

/// The screen, with a clean status bar when asked; the bar is put back whatever happens.
async fn shot(state: &AppState, target: &Target, clean: bool) -> CmdResult<Vec<u8>> {
    if clean {
        let bar = StatusBar { time: Some("9:41".into()), battery: Some(100), wifi: Some(true), cellular: Some(true) };
        state.lab.control(target, &Control::StatusBar { status_bar: bar }).await.map_err(err)?;
    }
    let result = state.lab.screenshot(target).await;
    if clean {
        let _ = state.lab.control(target, &Control::ClearStatusBar).await;
    }
    result.map_err(err)
}

/// Saves the screen as a baseline. An existing one is only replaced with `replace`, which the interface sets after asking.
#[tauri::command]
pub async fn visual_capture(
    state: State<'_, AppState>,
    target: Target,
    name: String,
    clean_status_bar: bool,
    replace: bool,
) -> CmdResult<()> {
    let png = shot(&state, &target, clean_status_bar).await?;
    visual::save_baseline(&baselines(), &name, &png, replace).map_err(err)?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualResult {
    pub comparison: Comparison,
    pub threshold: f64,
    pub exceeds: bool,
    pub baseline: String,
    pub current: String,
    pub diff: String,
}

#[tauri::command]
pub async fn visual_compare(
    state: State<'_, AppState>,
    target: Target,
    name: String,
    clean_status_bar: bool,
    threshold: f64,
    ignore: Vec<Region>,
) -> CmdResult<VisualResult> {
    if !(0.0..=100.0).contains(&threshold) {
        return Err("The limit is a percentage between 0 and 100.".into());
    }
    let baseline = visual::load_baseline(&baselines(), &name).map_err(err)?;
    let current = shot(&state, &target, clean_status_bar).await?;
    let done = visual::compare(&baseline, &current, &Options { tolerance: 8, ignore }).map_err(err)?;
    Ok(VisualResult {
        exceeds: done.comparison.exceeds(threshold),
        comparison: done.comparison,
        threshold,
        baseline: b64(&baseline),
        current: b64(&current),
        diff: b64(&done.diff),
    })
}

// ---- Analytics ------------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsItem {
    pub flow: u64,
    pub started_ms: u64,
    pub host: String,
    pub event: AnalyticsEvent,
}

/// The analytics events of every request the proxy has recorded after `after` (a flow id; 0 for all).
#[tauri::command]
pub async fn analytics_events(state: State<'_, AppState>, after: u64) -> CmdResult<Vec<AnalyticsItem>> {
    let mut out = Vec::new();
    for f in state.flows.all().iter().filter(|f| f.id > after) {
        for event in analytics::events(f) {
            out.push(AnalyticsItem { flow: f.id, started_ms: f.started_ms, host: f.host.clone(), event });
        }
    }
    Ok(out)
}
