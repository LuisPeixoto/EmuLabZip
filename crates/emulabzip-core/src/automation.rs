//! Flows: a run of touches, typed text and key presses that can be recorded, played again, and exported.
//!
//! A flow is recorded from the touches on the screen (`getevent` on Android), with the pauses between them. Where
//! the layout of the screen is known, a tap also remembers the text or id of the view it hit, so an export can say
//! `tapOn: "Login"` and survive a different screen size. A flow plays through [`Control`]s, so it needs nothing on
//! the device, and is exported as a [Maestro](https://maestro.mobile.dev) flow or as a shell script of
//! `emulabzip control` commands.

use serde::{Deserialize, Serialize};

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::control::{Control, MAX_COORDINATE, MAX_SWIPE_MS, is_app_id};
use crate::events::{InputEvent, InputKind};
use crate::layout::{LayoutSnapshot, UiNode};
use crate::{Error, Lab, Result, Target};

/// The most steps in a flow, and the longest pause in one.
pub const MAX_STEPS: usize = 5_000;
pub const MAX_WAIT_MS: u32 = 60_000;
/// A pause shorter than this between two touches is not recorded: the device needs that long anyway.
const MIN_RECORDED_GAP_MS: u64 = 1_000;
/// How long to wait after a step while playing, so the screen can react before the next one.
pub const SETTLE_MS: u32 = 300;

/// What a tap hit, when the layout was known: the id or the text of the view.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub id: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Step {
    /// Start the app of the flow.
    Launch,
    Tap {
        x: u32,
        y: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<Label>,
    },
    LongPress {
        x: u32,
        y: u32,
        ms: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<Label>,
    },
    Swipe {
        x1: u32,
        y1: u32,
        x2: u32,
        y2: u32,
        ms: u32,
    },
    Text {
        text: String,
    },
    Key {
        key: String,
    },
    Wait {
        ms: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Flow {
    pub name: String,
    /// The package name or bundle id the flow is for.
    pub app: Option<String>,
    /// The size of the screen it was recorded on, in pixels.
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub steps: Vec<Step>,
}

/// One thing to do when playing a flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Do(Control),
    Wait(u32),
}

impl Flow {
    pub fn new(name: &str, app: Option<&str>, screen: Option<(u32, u32)>) -> Flow {
        Flow {
            name: name.to_string(),
            app: app.map(str::to_string),
            width: screen.map(|s| s.0),
            height: screen.map(|s| s.1),
            steps: Vec::new(),
        }
    }

    /// Reads a flow saved as JSON, and checks it.
    pub fn parse(text: &str) -> Result<Flow> {
        let flow: Flow = serde_json::from_str(text).map_err(|e| Error::failed(format!("That is not a flow: {e}")))?;
        flow.validate()?;
        Ok(flow)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default() + "\n"
    }

    /// Rejects what a device should never be sent. Every step is checked as the control it becomes.
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() || self.name.len() > 100 {
            return Err(Error::invalid("flow name", self.name.clone()));
        }
        if let Some(app) = &self.app
            && !is_app_id(app)
        {
            return Err(Error::invalid("app", app.clone()));
        }
        if self.steps.len() > MAX_STEPS {
            return Err(Error::invalid("flow", format!("{} steps (at most {MAX_STEPS})", self.steps.len())));
        }
        for step in &self.steps {
            match self.action(step, (1.0, 1.0))? {
                Action::Do(control) => control.validate()?,
                Action::Wait(_) => {}
            }
        }
        Ok(())
    }

    /// What a step does, with the coordinates multiplied by `scale` (for a screen of another size).
    pub fn action(&self, step: &Step, scale: (f64, f64)) -> Result<Action> {
        let sx = |v: u32| ((f64::from(v) * scale.0).round() as u32).min(MAX_COORDINATE);
        let sy = |v: u32| ((f64::from(v) * scale.1).round() as u32).min(MAX_COORDINATE);
        Ok(match step {
            Step::Launch => {
                let app = self.app.clone().ok_or_else(|| Error::failed("This flow has no app to launch."))?;
                Action::Do(Control::Launch { app })
            }
            Step::Tap { x, y, .. } => Action::Do(Control::Tap { x: sx(*x), y: sy(*y) }),
            Step::LongPress { x, y, ms, .. } => Action::Do(Control::Swipe {
                x1: sx(*x),
                y1: sy(*y),
                x2: sx(*x),
                y2: sy(*y),
                duration_ms: Some((*ms).clamp(1, MAX_SWIPE_MS)),
            }),
            Step::Swipe { x1, y1, x2, y2, ms } => Action::Do(Control::Swipe {
                x1: sx(*x1),
                y1: sy(*y1),
                x2: sx(*x2),
                y2: sy(*y2),
                duration_ms: Some((*ms).clamp(1, MAX_SWIPE_MS)),
            }),
            Step::Text { text } => Action::Do(Control::Text { text: text.clone() }),
            Step::Key { key } => Action::Do(Control::Key { key: key.clone() }),
            Step::Wait { ms } => {
                if *ms > MAX_WAIT_MS {
                    return Err(Error::invalid("a pause in milliseconds", ms.to_string()));
                }
                Action::Wait(*ms)
            }
        })
    }

    /// About how long playing takes, in milliseconds: the pauses, and a moment after each step.
    pub fn duration_ms(&self) -> u64 {
        self.steps
            .iter()
            .map(|s| match s {
                Step::Wait { ms } => u64::from(*ms),
                Step::Swipe { ms, .. } | Step::LongPress { ms, .. } => u64::from(*ms + SETTLE_MS),
                _ => u64::from(SETTLE_MS),
            })
            .sum()
    }

    /// The factor that moves coordinates recorded on this flow's screen to a screen of `width` by `height`.
    pub fn scale_to(&self, width: u32, height: u32) -> (f64, f64) {
        match (self.width, self.height) {
            (Some(w), Some(h)) if w > 0 && h > 0 => (f64::from(width) / f64::from(w), f64::from(height) / f64::from(h)),
            _ => (1.0, 1.0),
        }
    }

    // -----------------------------------------------------------------------------------------
    // Exports
    // -----------------------------------------------------------------------------------------

    /// A [Maestro](https://maestro.mobile.dev) flow. A tap that knows the view it hit says `id` or the text; the
    /// others say where, as a percentage of the screen when its size is known.
    pub fn to_maestro(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# {}\n", comment(&self.name)));
        match &self.app {
            Some(app) => out.push_str(&format!("appId: {}\n", yaml_str(app))),
            None => out.push_str("# appId: your.app.id\n"),
        }
        out.push_str("---\n");
        let point = |x: u32, y: u32| match (self.width, self.height) {
            (Some(w), Some(h)) if w > 0 && h > 0 => format!(
                "{}%,{}%",
                (f64::from(x) / f64::from(w) * 100.0).round().clamp(0.0, 100.0),
                (f64::from(y) / f64::from(h) * 100.0).round().clamp(0.0, 100.0)
            ),
            _ => format!("{x},{y}"),
        };
        for step in &self.steps {
            match step {
                Step::Launch => out.push_str("- launchApp\n"),
                Step::Tap { x, y, label } => out.push_str(&maestro_target("tapOn", *x, *y, label.as_ref(), &point)),
                Step::LongPress { x, y, label, .. } => {
                    out.push_str(&maestro_target("longPressOn", *x, *y, label.as_ref(), &point))
                }
                Step::Swipe { x1, y1, x2, y2, ms } => out.push_str(&format!(
                    "- swipe:\n    start: {}\n    end: {}\n    duration: {ms}\n",
                    yaml_str(&point(*x1, *y1)),
                    yaml_str(&point(*x2, *y2))
                )),
                Step::Text { text } => out.push_str(&format!("- inputText: {}\n", yaml_str(text))),
                Step::Key { key } => match maestro_key(key) {
                    Some(name) => out.push_str(&format!("- pressKey: {}\n", yaml_str(name))),
                    None => out.push_str(&format!("# the key {} has no Maestro name\n", comment(key))),
                },
                Step::Wait { ms } => {
                    out.push_str(&format!("- waitForAnimationToEnd:\n    timeout: {}\n", (*ms).clamp(500, MAX_WAIT_MS)))
                }
            }
        }
        out
    }

    /// A shell script that plays the flow with `emulabzip control`. The device is the one in `EMULABZIP_DEVICE`.
    pub fn to_script(&self) -> String {
        let mut out = String::from("#!/usr/bin/env bash\n");
        out.push_str(&format!("# {}\n", comment(&self.name)));
        out.push_str("# Plays on the device in EMULABZIP_DEVICE, or the only one running.\nset -euo pipefail\n");
        for step in &self.steps {
            match step {
                Step::Launch => match &self.app {
                    Some(app) => out.push_str(&format!("emulabzip control launch {}\n", sh(app))),
                    None => out.push_str("# launch: this flow has no app\n"),
                },
                Step::Tap { x, y, .. } => out.push_str(&format!("emulabzip control tap {x} {y}\n")),
                Step::LongPress { x, y, ms, .. } => {
                    out.push_str(&format!("emulabzip control swipe {x} {y} {x} {y} --ms {ms}\n"))
                }
                Step::Swipe { x1, y1, x2, y2, ms } => {
                    out.push_str(&format!("emulabzip control swipe {x1} {y1} {x2} {y2} --ms {ms}\n"))
                }
                Step::Text { text } => out.push_str(&format!("emulabzip control text {}\n", sh(text))),
                Step::Key { key } => out.push_str(&format!("emulabzip control key {}\n", sh(key))),
                Step::Wait { ms } => out.push_str(&format!("sleep {}.{:03}\n", ms / 1000, ms % 1000)),
            }
            if !matches!(step, Step::Wait { .. }) {
                out.push_str(&format!("sleep 0.{SETTLE_MS:03}\n"));
            }
        }
        out
    }
}

/// `tapOn: "text"`, with an `id` or text when the view is known, else a point.
fn maestro_target(command: &str, x: u32, y: u32, label: Option<&Label>, point: &impl Fn(u32, u32) -> String) -> String {
    if let Some(id) = label.and_then(|l| l.id.as_deref()) {
        return format!("- {command}:\n    id: {}\n", yaml_str(&regex_escape(id)));
    }
    if let Some(text) = label.and_then(|l| l.text.as_deref()) {
        // Maestro reads the text as a regular expression, so what it would act on is escaped.
        return format!("- {command}: {}\n", yaml_str(&regex_escape(text)));
    }
    format!("- {command}:\n    point: {}\n", yaml_str(&point(x, y)))
}

/// Maestro's name for one of our keys.
fn maestro_key(key: &str) -> Option<&'static str> {
    Some(match key {
        "back" => "Back",
        "home" => "Home",
        "enter" => "Enter",
        "delete" => "Backspace",
        "tab" => "Tab",
        "volume_up" => "Volume Up",
        "volume_down" => "Volume Down",
        "power" => "Power",
        "dpad_up" => "Remote Dpad Up",
        "dpad_down" => "Remote Dpad Down",
        "dpad_left" => "Remote Dpad Left",
        "dpad_right" => "Remote Dpad Right",
        "dpad_center" => "Remote Dpad Center",
        _ => return None,
    })
}

fn regex_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if "\\.^$|?*+()[]{}".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A double-quoted YAML string.
fn yaml_str(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Text that is safe on one comment line.
fn comment(text: &str) -> String {
    text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect()
}

/// A word for a shell: quoted when it needs it.
fn sh(text: &str) -> String {
    if !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/')) {
        text.to_string()
    } else {
        format!("'{}'", text.replace('\'', "'\\''"))
    }
}

// ---------------------------------------------------------------------------------------------
// Recording
// ---------------------------------------------------------------------------------------------

/// The id or the text of the view at a point of the screen, for a recorded tap.
pub fn label_at(root: &UiNode, x: u32, y: u32) -> Option<Label> {
    let (x, y) = (i32::try_from(x).ok()?, i32::try_from(y).ok()?);
    // The path from the root to the deepest view there, so the nearest named view can be found going back up.
    fn path<'a>(node: &'a UiNode, x: i32, y: i32, trail: &mut Vec<&'a UiNode>) {
        if !node.bounds.contains(x, y) {
            return;
        }
        trail.push(node);
        if let Some(child) = node.children.iter().rev().find(|c| c.bounds.contains(x, y)) {
            path(child, x, y, trail);
        }
    }
    let mut trail = Vec::new();
    path(root, x, y, &mut trail);
    for node in trail.into_iter().rev() {
        let id = node
            .resource_id
            .as_deref()
            .map(|r| r.rsplit_once('/').map_or(r, |(_, short)| short))
            .filter(|s| !s.is_empty() && s.len() <= 80);
        let text = node.own_name().filter(|t| t.chars().count() <= 60 && !t.contains('\n'));
        if id.is_some() || text.is_some() {
            return Some(Label { id: id.map(str::to_string), text: text.map(str::to_string) });
        }
        // A view that nothing is written on and that is not clickable is only the background: keep looking up.
    }
    None
}

/// Builds a flow from the gestures it is fed.
pub struct Recorder {
    flow: Flow,
    last_end: Option<u64>,
}

impl Recorder {
    /// `launch` puts a first step that starts the app, so playing begins from the same place.
    pub fn new(name: &str, app: Option<&str>, screen: Option<(u32, u32)>, launch: bool) -> Recorder {
        let mut flow = Flow::new(name, app, screen);
        if launch && app.is_some() {
            flow.steps.push(Step::Launch);
        }
        Recorder { flow, last_end: None }
    }

    /// Adds a gesture, with the pause since the one before, and what it hit when that is known.
    pub fn push(&mut self, event: &InputEvent, label: Option<Label>) {
        if !event.positioned || self.flow.steps.len() >= MAX_STEPS {
            return;
        }
        if let Some(end) = self.last_end {
            let gap = event.t_ms.saturating_sub(end);
            if gap >= MIN_RECORDED_GAP_MS {
                let ms = (((gap as f64) / 100.0).round() * 100.0) as u64;
                self.flow.steps.push(Step::Wait { ms: ms.min(u64::from(MAX_WAIT_MS)) as u32 });
            }
        } else if matches!(self.flow.steps.last(), Some(Step::Launch)) {
            // The app takes a moment to come up before the first touch.
            self.flow.steps.push(Step::Wait { ms: 2_000 });
        }
        self.last_end = Some(event.t_ms + u64::from(event.duration_ms));
        let ms = event.duration_ms.clamp(1, MAX_SWIPE_MS);
        self.flow.steps.push(match event.kind {
            InputKind::Tap => Step::Tap { x: event.x, y: event.y, label },
            InputKind::LongPress => Step::LongPress { x: event.x, y: event.y, ms, label },
            InputKind::Swipe => Step::Swipe { x1: event.x, y1: event.y, x2: event.x2, y2: event.y2, ms },
        });
    }

    pub fn len(&self) -> usize {
        self.flow.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.flow.steps.is_empty()
    }

    pub fn finish(self) -> Flow {
        self.flow
    }
}

// ---------------------------------------------------------------------------------------------
// Saved flows
// ---------------------------------------------------------------------------------------------

/// A saved flow, as the list shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowEntry {
    /// The file name without `.json`, which is also what to pass to [`load`].
    pub id: String,
    pub name: String,
    pub app: Option<String>,
    pub steps: usize,
}

/// Where flows are kept, shared by the app and the command line.
pub fn flows_dir() -> std::path::PathBuf {
    crate::data_dir().join("flows")
}

/// The most a flow file can hold. A flow of the most steps is far below this.
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// A name as a file name: lower-case letters, digits and hyphens.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_matches('-');
    if out.is_empty() {
        "flow".to_string()
    } else {
        out.chars().take(48).collect::<String>().trim_matches('-').to_string()
    }
}

fn is_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 48 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Keeps a flow in the flows folder, replacing one with the same name.
pub fn save(flow: &Flow) -> Result<std::path::PathBuf> {
    flow.validate()?;
    let dir = flows_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", slug(&flow.name)));
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, flow.to_json())?;
    std::fs::rename(&temp, &path)?;
    Ok(path)
}

/// Reads a flow from a file, with the size limit.
pub fn read(path: &std::path::Path) -> Result<Flow> {
    let meta = std::fs::metadata(path).map_err(|e| Error::failed(format!("Could not read {}: {e}", path.display())))?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return Err(Error::failed("That file is too big to be a flow."));
    }
    Flow::parse(&std::fs::read_to_string(path)?)
}

/// Reads a saved flow by its id.
pub fn load(id: &str) -> Result<Flow> {
    if !is_id(id) {
        return Err(Error::invalid("flow", id));
    }
    read(&flows_dir().join(format!("{id}.json")))
}

pub fn remove(id: &str) -> Result<()> {
    if !is_id(id) {
        return Err(Error::invalid("flow", id));
    }
    std::fs::remove_file(flows_dir().join(format!("{id}.json")))?;
    Ok(())
}

/// The saved flows, by name. A file that is not a flow is left out.
pub fn list() -> Vec<FlowEntry> {
    let Ok(dir) = std::fs::read_dir(flows_dir()) else { return Vec::new() };
    let mut out: Vec<FlowEntry> = dir
        .filter_map(|e| {
            let path = e.ok()?.path();
            let id = path.file_name()?.to_str()?.strip_suffix(".json")?.to_string();
            let flow = read(&path).ok()?;
            is_id(&id).then_some(FlowEntry { id, name: flow.name, app: flow.app, steps: flow.steps.len() })
        })
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

// ---------------------------------------------------------------------------------------------
// Recording and playing on a device
// ---------------------------------------------------------------------------------------------

/// Records the touches on a device until `stop` finishes, and returns the flow. `progress` hears every step as it is
/// added. After each gesture the screen is read again in the background, so the next tap can be given the text or id
/// of the view it hits; a tap that comes before that is ready is kept as a point.
pub async fn record(
    lab: &Lab,
    target: &Target,
    name: &str,
    app: Option<&str>,
    launch: bool,
    stop: impl Future<Output = ()>,
    mut progress: impl FnMut(&Step),
) -> Result<Flow> {
    let first = lab.layout(target).await.ok();
    let screen = first.as_ref().map(|l| (l.screen.width.max(0) as u32, l.screen.height.max(0) as u32));
    let mut tree: Option<UiNode> = first.map(|l| l.root);
    let mut input = lab.input(target).await?;
    let mut recorder = Recorder::new(name, app, screen, launch);
    type Reading<'a> = Pin<Box<dyn Future<Output = Result<LayoutSnapshot>> + 'a>>;
    let mut reading: Option<Reading<'_>> = None;
    tokio::pin!(stop);
    loop {
        tokio::select! {
            biased;
            () = &mut stop => break,
            event = input.next() => {
                let Some(event) = event else { break };
                // A screen read that has not finished is older than the touch: do not trust it for a label.
                let label = match (&tree, &reading, event.kind) {
                    (Some(root), None, InputKind::Tap | InputKind::LongPress) => label_at(root, event.x, event.y),
                    _ => None,
                };
                let before = recorder.len();
                recorder.push(&event, label);
                // `push` may add a pause before the gesture; tell about every new step.
                let flow = &recorder.flow;
                flow.steps[before..].iter().for_each(&mut progress);
                tree = None;
                reading = Some(Box::pin(lab.layout(target)));
            }
            snapshot = async { reading.as_mut().expect("guarded").await }, if reading.is_some() => {
                reading = None;
                tree = snapshot.ok().map(|l| l.root);
            }
        }
    }
    Ok(recorder.finish())
}

/// Plays a flow on a device through its controls, scaled when the screen is not the size it was recorded on.
/// `progress` hears the number of the step about to run. Playing ends early when `stop` finishes.
pub async fn play(
    lab: &Lab,
    target: &Target,
    flow: &Flow,
    stop: impl Future<Output = ()>,
    mut progress: impl FnMut(usize, &Step),
) -> Result<()> {
    flow.validate()?;
    let scale = match (flow.width, flow.height) {
        (Some(_), Some(_)) => match lab.layout(target).await {
            Ok(l) => flow.scale_to(l.screen.width.max(1) as u32, l.screen.height.max(1) as u32),
            Err(_) => (1.0, 1.0),
        },
        _ => (1.0, 1.0),
    };
    tokio::pin!(stop);
    for (i, step) in flow.steps.iter().enumerate() {
        progress(i, step);
        let pause = match flow.action(step, scale)? {
            Action::Do(control) => {
                tokio::select! {
                    biased;
                    () = &mut stop => return Ok(()),
                    done = lab.control(target, &control) => done?,
                }
                SETTLE_MS
            }
            Action::Wait(ms) => ms,
        };
        tokio::select! {
            biased;
            () = &mut stop => return Ok(()),
            () = tokio::time::sleep(Duration::from_millis(u64::from(pause))) => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::key_code;
    use crate::layout::Bounds;

    fn event(t: u64, kind: InputKind, x: u32, y: u32) -> InputEvent {
        InputEvent { t_ms: t, kind, x, y, x2: x, y2: y, duration_ms: 80, positioned: true }
    }

    fn node(id: Option<&str>, text: Option<&str>, b: Bounds, children: Vec<UiNode>) -> UiNode {
        UiNode {
            resource_id: id.map(str::to_string),
            text: text.map(str::to_string),
            bounds: b,
            enabled: true,
            children,
            ..UiNode::default()
        }
    }

    fn sample() -> Flow {
        let mut r = Recorder::new("Login", Some("dev.x.app"), Some((1080, 2400)), true);
        r.push(&event(10_000, InputKind::Tap, 540, 1200), Some(Label { id: None, text: Some("Log in (new)".into()) }));
        r.push(&event(10_400, InputKind::Tap, 100, 200), None);
        r.push(&event(14_000, InputKind::Swipe, 540, 1800), None);
        r.push(&event(15_000, InputKind::LongPress, 300, 300), Some(Label { id: Some("avatar".into()), text: None }));
        r.finish()
    }

    #[test]
    fn a_recording_keeps_the_gestures_and_the_long_pauses() {
        let flow = sample();
        assert_eq!(flow.steps[0], Step::Launch);
        assert_eq!(flow.steps[1], Step::Wait { ms: 2_000 });
        assert!(matches!(flow.steps[2], Step::Tap { x: 540, y: 1200, .. }));
        // 320 ms after the first tap ended: too short to record. The swipe is 3.5 s later.
        assert!(matches!(flow.steps[3], Step::Tap { x: 100, y: 200, label: None }));
        assert_eq!(flow.steps[4], Step::Wait { ms: 3_500 });
        assert!(matches!(flow.steps[5], Step::Swipe { .. }));
        assert!(flow.validate().is_ok());
    }

    #[test]
    fn a_touch_with_no_known_position_is_not_recorded() {
        let mut r = Recorder::new("x", None, None, false);
        r.push(&InputEvent { positioned: false, ..event(1, InputKind::Tap, 0, 0) }, None);
        assert!(r.is_empty());
    }

    #[test]
    fn a_flow_is_saved_and_read_back() {
        let flow = sample();
        let back = Flow::parse(&flow.to_json()).unwrap();
        assert_eq!(back, flow);
        assert!(Flow::parse("[]").is_err());
        let mut bad = flow.clone();
        bad.steps.push(Step::Key { key: "KEYCODE_X; reboot".into() });
        assert!(bad.validate().is_err());
        let mut long = flow.clone();
        long.steps.push(Step::Wait { ms: MAX_WAIT_MS + 1 });
        assert!(long.validate().is_err());
        let mut named = flow;
        named.app = Some("not an app".into());
        assert!(named.validate().is_err());
    }

    #[test]
    fn maestro_gets_ids_and_text_where_known_and_points_elsewhere() {
        let m = sample().to_maestro();
        assert!(m.starts_with("# Login\nappId: \"dev.x.app\"\n---\n- launchApp\n"), "{m}");
        // The text is escaped for Maestro's regular expressions, then quoted for YAML.
        assert!(m.contains("- tapOn: \"Log in \\\\(new\\\\)\"\n"), "{m}");
        assert!(m.contains("- tapOn:\n    point: \"9%,8%\"\n"), "{m}");
        assert!(m.contains("- waitForAnimationToEnd:\n    timeout: 3500\n"), "{m}");
        assert!(m.contains("- swipe:\n    start: \"50%,75%\"\n    end: \"50%,75%\"\n    duration: 80\n"), "{m}");
        assert!(m.contains("- longPressOn:\n    id: \"avatar\"\n"), "{m}");
    }

    #[test]
    fn maestro_text_and_keys_are_quoted_and_unknown_keys_are_said() {
        let mut f = Flow::new("Keys\nnewline", None, None);
        f.steps = vec![
            Step::Text { text: "say \"hi\"\nthere".into() },
            Step::Key { key: "back".into() },
            Step::Key { key: "menu".into() },
            Step::Tap { x: 5, y: 6, label: None },
        ];
        let m = f.to_maestro();
        assert!(m.starts_with("# Keys newline\n# appId: your.app.id\n---\n"), "{m}");
        assert!(m.contains("- inputText: \"say \\\"hi\\\"\\nthere\"\n"), "{m}");
        assert!(m.contains("- pressKey: \"Back\"\n"));
        assert!(m.contains("# the key menu has no Maestro name\n"));
        assert!(m.contains("point: \"5,6\""), "without a screen size the point is in pixels: {m}");
    }

    #[test]
    fn the_script_calls_the_cli_and_quotes_what_the_shell_would_act_on() {
        let mut f = sample();
        f.steps.push(Step::Text { text: "it's $HOME; reboot".into() });
        let s = f.to_script();
        assert!(s.starts_with("#!/usr/bin/env bash\n# Login\n"));
        assert!(s.contains("emulabzip control launch dev.x.app\n"));
        assert!(s.contains("emulabzip control tap 540 1200\n"));
        assert!(s.contains("emulabzip control swipe 300 300 300 300 --ms 80\n"));
        assert!(s.contains("sleep 3.500\n"));
        assert!(s.contains("emulabzip control text 'it'\\''s $HOME; reboot'\n"), "{s}");
    }

    #[test]
    fn steps_become_controls_scaled_to_another_screen() {
        let flow = sample();
        let (sx, sy) = flow.scale_to(540, 1200);
        assert_eq!((sx, sy), (0.5, 0.5));
        let Action::Do(Control::Tap { x, y }) = flow.action(&flow.steps[2], (sx, sy)).unwrap() else { panic!() };
        assert_eq!((x, y), (270, 600));
        let Action::Do(Control::Swipe { x1, x2, duration_ms, .. }) = flow.action(&flow.steps[6], (1.0, 1.0)).unwrap()
        else {
            panic!("{:?}", flow.steps[6])
        };
        assert_eq!((x1, x2, duration_ms), (300, 300, Some(80)), "a long press is a swipe from a point to itself");
        assert_eq!(flow.action(&Step::Wait { ms: 700 }, (1.0, 1.0)).unwrap(), Action::Wait(700));
        let none = Flow::new("x", None, None);
        assert!(none.action(&Step::Launch, (1.0, 1.0)).is_err());
        assert_eq!(none.scale_to(100, 100), (1.0, 1.0));
        let mut timed = Flow::new("t", None, None);
        timed.steps = vec![
            Step::Wait { ms: 1000 },
            Step::Tap { x: 1, y: 1, label: None },
            Step::Swipe { x1: 0, y1: 0, x2: 1, y2: 1, ms: 200 },
        ];
        assert_eq!(timed.duration_ms(), 1000 + 300 + 500);
        assert!(key_code("back").is_some());
    }

    /// A device that remembers the controls it was sent, and shows a button on its screen.
    struct Recording {
        sent: std::sync::Arc<std::sync::Mutex<Vec<Control>>>,
        touches: Vec<InputEvent>,
        screen: (i32, i32),
    }

    #[async_trait::async_trait]
    impl crate::Backend for Recording {
        fn platform(&self) -> crate::Platform {
            crate::Platform::Android
        }
        async fn is_available(&self) -> bool {
            true
        }
        async fn devices(&self) -> Result<Vec<crate::Device>> {
            Ok(vec![])
        }
        async fn info(&self, _: &str) -> Result<Vec<crate::InfoSection>> {
            Ok(vec![])
        }
        async fn apps(&self, _: &str) -> Result<Vec<crate::App>> {
            Ok(vec![])
        }
        async fn logs(&self, _: &str, _: crate::LogOptions) -> Result<crate::Stream<crate::LogEntry>> {
            Err(Error::Unsupported("logs"))
        }
        async fn screenshot(&self, _: &str) -> Result<Vec<u8>> {
            Ok(vec![])
        }
        async fn control(&self, _: &str, control: &Control) -> Result<()> {
            self.sent.lock().unwrap().push(control.clone());
            Ok(())
        }
        async fn layout(&self, _: &str) -> Result<LayoutSnapshot> {
            let button = node(Some("app:id/go"), Some("Go"), Bounds::new(100, 100, 300, 100), vec![]);
            let root = node(None, None, Bounds::new(0, 0, self.screen.0, self.screen.1), vec![button]);
            let screen = crate::layout::Screen { width: self.screen.0, height: self.screen.1, density: 1.0 };
            Ok(LayoutSnapshot::new(crate::Platform::Android, root, screen, vec![]))
        }
        async fn input(&self, _: &str) -> Result<crate::Stream<InputEvent>> {
            let touches = self.touches.clone();
            Ok(crate::Stream::spawn(8, |tx| async move {
                for t in touches {
                    let _ = tx.send(t).await;
                    // The screen is read again after a gesture; give it time before the next one.
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                // Keep the stream open, as a real device does, until the recording is stopped.
                std::future::pending::<()>().await;
            }))
        }
    }

    fn target() -> Target {
        Target { platform: crate::Platform::Android, id: "x".into() }
    }

    #[tokio::test]
    async fn a_recording_labels_a_tap_with_the_view_it_hit() {
        let device = Recording {
            sent: Default::default(),
            touches: vec![event(10_000, InputKind::Tap, 150, 150), event(20_000, InputKind::Tap, 900, 1500)],
            screen: (1080, 2400),
        };
        let lab = Lab::new().with(device);
        let mut seen = 0;
        let stop = tokio::time::sleep(Duration::from_millis(400));
        let flow = record(&lab, &target(), "Go", Some("dev.x.app"), true, stop, |_| seen += 1).await.unwrap();
        assert_eq!((flow.width, flow.height), (Some(1080), Some(2400)));
        let Step::Tap { label, .. } = &flow.steps[2] else { panic!("{:?}", flow.steps) };
        assert_eq!(label.as_ref().unwrap(), &Label { id: Some("go".into()), text: Some("Go".into()) });
        assert!(seen >= 4, "every step, including the pause, is reported: {seen}");
        assert!(flow.steps.iter().any(|s| matches!(s, Step::Wait { .. })));
    }

    #[tokio::test]
    async fn playing_sends_the_controls_scaled_to_the_screen() {
        let sent: std::sync::Arc<std::sync::Mutex<Vec<Control>>> = Default::default();
        let device = Recording { sent: sent.clone(), touches: vec![], screen: (540, 1200) };
        let lab = Lab::new().with(device);
        let mut flow = Flow::new("x", Some("dev.x.app"), Some((1080, 2400)));
        flow.steps = vec![
            Step::Launch,
            Step::Tap { x: 540, y: 1200, label: None },
            Step::Wait { ms: 10 },
            Step::Key { key: "back".into() },
        ];
        let mut order = Vec::new();
        play(&lab, &target(), &flow, std::future::pending(), |i, _| order.push(i)).await.unwrap();
        assert_eq!(order, [0, 1, 2, 3]);
        assert_eq!(
            *sent.lock().unwrap(),
            [
                Control::Launch { app: "dev.x.app".into() },
                // Recorded on a screen twice the size, so the point is halved.
                Control::Tap { x: 270, y: 600 },
                Control::Key { key: "back".into() },
            ]
        );
        // A flow that is not valid is refused before anything is sent.
        let mut bad = flow.clone();
        bad.steps.push(Step::Key { key: "nope".into() });
        assert!(play(&lab, &target(), &bad, std::future::pending(), |_, _| {}).await.is_err());
        // Stopping ends the play early.
        let mut slow = flow;
        slow.steps = vec![Step::Wait { ms: 30_000 }, Step::Key { key: "home".into() }];
        let started = std::time::Instant::now();
        play(&lab, &target(), &slow, tokio::time::sleep(Duration::from_millis(100)), |_, _| {}).await.unwrap();
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn names_become_safe_file_names() {
        assert_eq!(slug("Log in, then buy!"), "log-in-then-buy");
        assert_eq!(slug("../../etc/passwd"), "etc-passwd");
        assert_eq!(slug("   "), "flow");
        assert_eq!(slug(&"a".repeat(80)).len(), 48);
        assert!(!is_id("a/b") && !is_id("..") && !is_id("") && is_id("log-in"));
    }

    #[test]
    fn a_file_that_is_big_or_not_a_flow_is_refused() {
        let dir = std::env::temp_dir().join(format!("emulabzip-flows-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ok = dir.join("ok.json");
        std::fs::write(&ok, sample().to_json()).unwrap();
        assert_eq!(read(&ok).unwrap().name, "Login");
        let big = dir.join("big.json");
        std::fs::write(&big, "x".repeat(MAX_FILE_BYTES as usize + 1)).unwrap();
        assert!(read(&big).is_err());
        let junk = dir.join("junk.json");
        std::fs::write(&junk, "{\"name\": 3}").unwrap();
        assert!(read(&junk).is_err());
        assert!(read(&dir).is_err(), "a folder is not a flow");
        assert!(load("../x").is_err());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_tap_is_labelled_with_the_nearest_named_view() {
        let button = node(
            Some("dev.x.app:id/login"),
            None,
            Bounds::new(100, 100, 400, 120),
            vec![node(None, Some("Log in"), Bounds::new(200, 130, 200, 60), vec![])],
        );
        let root = node(None, None, Bounds::new(0, 0, 1080, 2400), vec![button]);
        // On the text inside the button: the text's own view has a name and no id.
        let on_text = label_at(&root, 250, 150).unwrap();
        assert_eq!(on_text, Label { id: None, text: Some("Log in".into()) });
        // On the button's padding: the button, by its id.
        let on_button = label_at(&root, 120, 110).unwrap();
        assert_eq!(on_button, Label { id: Some("login".into()), text: None });
        // On nothing named.
        assert_eq!(label_at(&root, 900, 2000), None);
        assert_eq!(label_at(&root, 5000, 5000), None);
    }
}
