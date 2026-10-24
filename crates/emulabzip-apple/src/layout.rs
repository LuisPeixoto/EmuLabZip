//! The views on a simulator's screen, read from the accessibility tree.
//!
//! `simctl` cannot list views, but Simulator.app passes the iOS accessibility tree of the booted
//! device on to the macOS Accessibility API: labels, values and identifiers, with a frame for each.
//! That is the tree VoiceOver walks, so it suits the accessibility audit. Reading it needs the
//! program that asks (Terminal, or EmuLabZip) to be allowed under System Settings, Privacy &
//! Security, Accessibility, and the simulator window to be open.

use emulabzip_core::{Bounds, UiNode};

/// A frame in the coordinates of the Mac's screen.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// One element as the Accessibility API reports it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AxNode {
    pub role: String,
    pub title: String,
    /// What VoiceOver reads: the accessibility label.
    pub description: String,
    /// A switch's state, a field's text.
    pub value: String,
    pub identifier: String,
    pub enabled: bool,
    pub frame: Frame,
    pub children: Vec<AxNode>,
}

/// How the simulator's picture of the screen maps onto points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mapping {
    /// The top left of the screen on the Mac's display.
    pub origin: (f64, f64),
    /// Mac screen points per iOS point; the Simulator window can be zoomed.
    pub zoom: f64,
}

impl Mapping {
    /// `screen` is the on-Mac frame of the whole device screen, `points_wide` its width in iOS points.
    pub fn new(screen: Frame, points_wide: f64) -> Option<Mapping> {
        (screen.width > 0.0 && points_wide > 0.0)
            .then(|| Mapping { origin: (screen.x, screen.y), zoom: screen.width / points_wide })
    }

    fn bounds(&self, f: Frame) -> Bounds {
        let to = |v: f64| v.round() as i32;
        Bounds::new(
            to((f.x - self.origin.0) / self.zoom),
            to((f.y - self.origin.1) / self.zoom),
            to(f.width / self.zoom),
            to(f.height / self.zoom),
        )
    }
}

fn non_empty(text: &str) -> Option<String> {
    let t = text.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// Roles that are read out rather than operated.
fn is_text(role: &str) -> bool {
    matches!(role, "AXStaticText" | "AXHeading")
}

fn is_control(role: &str) -> bool {
    matches!(
        role,
        "AXButton"
            | "AXLink"
            | "AXCheckBox"
            | "AXRadioButton"
            | "AXSwitch"
            | "AXToggle"
            | "AXPopUpButton"
            | "AXMenuItem"
            | "AXSlider"
            | "AXTabButton"
            | "AXSegmentedControl"
            | "AXTextField"
            | "AXTextArea"
            | "AXSearchField"
            | "AXComboBox"
            | "AXCell"
    )
}

fn convert(ax: &AxNode, map: &Mapping) -> UiNode {
    let text_like = is_text(&ax.role);
    let editable = matches!(ax.role.as_str(), "AXTextField" | "AXTextArea" | "AXSearchField" | "AXComboBox");
    let switch = matches!(ax.role.as_str(), "AXSwitch" | "AXToggle" | "AXCheckBox" | "AXRadioButton");
    UiNode {
        id: 0,
        class: ax.role.clone(),
        // Static text is shown text; for anything else the description is the label.
        text: if text_like {
            non_empty(&ax.description).or_else(|| non_empty(&ax.value))
        } else if editable {
            non_empty(&ax.value)
        } else {
            None
        },
        label: if text_like { None } else { non_empty(&ax.description).or_else(|| non_empty(&ax.title)) },
        hint: None,
        resource_id: non_empty(&ax.identifier),
        package: None,
        bounds: map.bounds(ax.frame),
        clickable: is_control(&ax.role),
        focusable: is_control(&ax.role),
        enabled: ax.enabled,
        checked: switch.then(|| matches!(ax.value.trim(), "1" | "true" | "on")),
        scrollable: ax.role == "AXScrollArea",
        editable,
        children: ax.children.iter().map(|c| convert(c, map)).collect(),
    }
}

/// Turns the elements under the screen into a layout tree. The screen itself becomes the root.
pub fn to_layout(screen: &AxNode, map: &Mapping) -> UiNode {
    let mut root = convert(screen, map);
    root.class = "Screen".into();
    root.clickable = false;
    root.focusable = false;
    root
}

/// What the simulator's device type says about its screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenProfile {
    /// The screen size in pixels.
    pub width_px: f64,
    pub height_px: f64,
    pub scale: f64,
}

impl ScreenProfile {
    pub fn from_profile(profile: &serde_json::Value) -> Option<Self> {
        let get = |key: &str| profile.get(key).and_then(serde_json::Value::as_f64).filter(|v| *v > 0.0);
        Some(ScreenProfile {
            width_px: get("mainScreenWidth")?,
            height_px: get("mainScreenHeight")?,
            scale: get("mainScreenScale")?,
        })
    }

    pub fn points(&self) -> (f64, f64) {
        (self.width_px / self.scale, self.height_px / self.scale)
    }
}

/// Reads the screen: the view tree and a screenshot taken at the same moment.
#[cfg(target_os = "macos")]
pub async fn snapshot(
    backend: &crate::AppleBackend,
    id: &str,
) -> emulabzip_core::Result<emulabzip_core::LayoutSnapshot> {
    use emulabzip_core::{Backend, Error, LayoutSnapshot, Platform, Screen};

    let sim = backend.simulator(id).await?;
    if sim.state != "Booted" {
        return Err(Error::failed(format!("“{}” is not booted.", sim.name)));
    }
    let (shot, profile, pid) = tokio::join!(backend.screenshot(id), screen_profile(&sim), simulator_pid());
    let shot = shot?;
    let pid = pid.ok_or_else(|| Error::failed("Simulator.app is not open. Open it to read the screen."))?;
    let profile = profile
        .or_else(|| fallback_profile(&shot))
        .ok_or_else(|| Error::failed("Could not tell how large the screen is."))?;
    let (name, os) = (sim.name.clone(), sim.runtime.as_ref().map(|r| r.version.clone()));
    let ax = tokio::task::spawn_blocking(move || mac::read_screen(pid, &name, os.as_deref()))
        .await
        .map_err(|e| Error::failed(e.to_string()))??;
    let (width, height) = profile.points();
    let map = Mapping::new(ax.frame, width).ok_or_else(|| Error::failed("The simulator window has no size."))?;
    let root = to_layout(&ax, &map);
    Ok(LayoutSnapshot::new(
        Platform::Ios,
        root,
        Screen { width: width.round() as i32, height: height.round() as i32, density: 1.0 },
        shot,
    ))
}

#[cfg(not(target_os = "macos"))]
pub async fn snapshot(_: &crate::AppleBackend, _: &str) -> emulabzip_core::Result<emulabzip_core::LayoutSnapshot> {
    Err(emulabzip_core::Error::Unsupported("Layout inspection"))
}

#[cfg(target_os = "macos")]
async fn simulator_pid() -> Option<i32> {
    let out = tokio::process::Command::new("pgrep").args(["-x", "Simulator"]).kill_on_drop(true).output().await.ok()?;
    String::from_utf8_lossy(&out.stdout).lines().next()?.trim().parse().ok()
}

/// The screen size and scale from the device type's `profile.plist`.
#[cfg(target_os = "macos")]
async fn screen_profile(sim: &crate::devices::SimDevice) -> Option<ScreenProfile> {
    let id = sim.device_type_identifier.as_deref()?;
    let types: serde_json::Value =
        serde_json::from_slice(&crate::simctl::run(["list", "devicetypes", "-j"]).await.ok()?).ok()?;
    let bundle = types
        .get("devicetypes")?
        .as_array()?
        .iter()
        .find(|t| t.get("identifier").and_then(|v| v.as_str()) == Some(id))?
        .get("bundlePath")?
        .as_str()?
        .to_string();
    let plist = tokio::fs::read(std::path::Path::new(&bundle).join("Contents/Resources/profile.plist")).await.ok()?;
    ScreenProfile::from_profile(&crate::simctl::plist_to_json(&plist).await.ok()?)
}

/// When the profile cannot be read: the screenshot is in pixels, and iPhones and iPads of the last
/// years are 2x or 3x; a screen over 2000 pixels tall that is not tablet-shaped is taken to be 3x.
#[cfg(target_os = "macos")]
fn fallback_profile(png: &[u8]) -> Option<ScreenProfile> {
    let ihdr = png.get(16..24)?;
    let (w, h) = (
        f64::from(u32::from_be_bytes(ihdr[..4].try_into().ok()?)),
        f64::from(u32::from_be_bytes(ihdr[4..].try_into().ok()?)),
    );
    let scale = if h > 2000.0 && w / h < 0.6 { 3.0 } else { 2.0 };
    Some(ScreenProfile { width_px: w, height_px: h, scale })
}

#[cfg(target_os = "macos")]
pub mod mac {
    //! The calls into the Accessibility API.

    use std::ffi::c_void;
    use std::time::Duration;

    use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
    use core_foundation::base::{CFGetTypeID, CFRelease, CFRetain, CFType, CFTypeRef, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use emulabzip_core::{Error, Result};

    use super::{AxNode, Frame};

    type AxElement = *const c_void;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXUIElementCreateApplication(pid: i32) -> AxElement;
        fn AXUIElementCopyAttributeValue(element: AxElement, attribute: *const c_void, value: *mut CFTypeRef) -> i32;
        fn AXUIElementSetMessagingTimeout(element: AxElement, seconds: f32) -> i32;
        fn AXValueGetValue(value: CFTypeRef, kind: u32, out: *mut c_void) -> bool;
        fn AXValueGetTypeID() -> usize;
    }

    const AX_CG_POINT: u32 = 1;
    const AX_CG_SIZE: u32 = 2;
    /// Elements deeper than this, or beyond this many, are left out.
    const MAX_DEPTH: usize = 60;
    const MAX_NODES: usize = 6000;

    /// An element kept past the array it was found in.
    struct Held(AxElement);

    impl Held {
        fn new(element: AxElement) -> Held {
            // SAFETY: `element` is alive now; the extra reference keeps it so until `drop`.
            unsafe { CFRetain(element) };
            Held(element)
        }
    }

    impl Drop for Held {
        fn drop(&mut self) {
            // SAFETY: balances the retain in `new`.
            unsafe { CFRelease(self.0) };
        }
    }

    fn attribute(element: AxElement, name: &str) -> Option<CFType> {
        let key = CFString::new(name);
        let mut out: CFTypeRef = std::ptr::null();
        // SAFETY: `element` is a live AXUIElement and `key` outlives the call; on success `out` is
        // an owned reference that `wrap_under_create_rule` releases.
        let status = unsafe { AXUIElementCopyAttributeValue(element, key.as_concrete_TypeRef().cast(), &mut out) };
        (status == 0 && !out.is_null()).then(|| unsafe { CFType::wrap_under_create_rule(out) })
    }

    fn string(element: AxElement, name: &str) -> String {
        match attribute(element, name) {
            Some(v) => {
                if let Some(s) = v.downcast::<CFString>() {
                    s.to_string()
                } else if let Some(n) = v.downcast::<CFNumber>() {
                    n.to_i64().map(|n| n.to_string()).unwrap_or_default()
                } else {
                    String::new()
                }
            }
            None => String::new(),
        }
    }

    fn frame(element: AxElement) -> Frame {
        let (mut p, mut s) = ([0f64; 2], [0f64; 2]);
        // SAFETY: AXValue holds a CGPoint / CGSize, two f64 each, which is what the buffers are.
        unsafe {
            if let Some(v) = attribute(element, "AXPosition")
                && CFGetTypeID(v.as_CFTypeRef()) == AXValueGetTypeID()
            {
                AXValueGetValue(v.as_CFTypeRef(), AX_CG_POINT, p.as_mut_ptr().cast());
            }
            if let Some(v) = attribute(element, "AXSize")
                && CFGetTypeID(v.as_CFTypeRef()) == AXValueGetTypeID()
            {
                AXValueGetValue(v.as_CFTypeRef(), AX_CG_SIZE, s.as_mut_ptr().cast());
            }
        }
        Frame { x: p[0], y: p[1], width: s[0], height: s[1] }
    }

    /// Calls `visit` with each child, which stays valid only during the call.
    fn each_child(element: AxElement, mut visit: impl FnMut(AxElement)) {
        let Some(children) = attribute(element, "AXChildren") else { return };
        let array = children.as_CFTypeRef() as CFArrayRef;
        // SAFETY: the attribute is a CFArray of AXUIElements, owned by `children` until it drops.
        unsafe {
            for i in 0..CFArrayGetCount(array) {
                visit(CFArrayGetValueAtIndex(array, i));
            }
        }
    }

    fn read(element: AxElement, depth: usize, budget: &mut usize) -> AxNode {
        *budget = budget.saturating_sub(1);
        // Anything but an explicit `false` counts as enabled.
        let enabled =
            attribute(element, "AXEnabled").is_none_or(|v| v.as_CFTypeRef() != CFBoolean::false_value().as_CFTypeRef());
        let mut node = AxNode {
            role: string(element, "AXRole"),
            title: string(element, "AXTitle"),
            description: string(element, "AXDescription"),
            value: string(element, "AXValue"),
            identifier: string(element, "AXIdentifier"),
            enabled,
            frame: frame(element),
            children: Vec::new(),
        };
        if depth < MAX_DEPTH {
            each_child(element, |child| {
                if *budget > 0 {
                    node.children.push(read(child, depth + 1, budget));
                }
            });
        }
        node
    }

    fn count(node: &AxNode) -> usize {
        1 + node.children.iter().map(count).sum::<usize>()
    }

    /// The window of the simulator called `name` that runs `os` (such as `26.2`), and inside it the
    /// element that is the device's screen. Blocking: run it on a thread of its own.
    pub fn read_screen(simulator_pid: i32, name: &str, os: Option<&str>) -> Result<AxNode> {
        // SAFETY: a plain query of whether this process was granted access.
        if !unsafe { AXIsProcessTrusted() } {
            return Err(Error::failed(
                "Reading the simulator's screen needs permission. Open System Settings, Privacy & Security, Accessibility, \
                 and allow the program that runs EmuLabZip (Terminal, or EmuLabZip itself); then try again.",
            ));
        }
        // SAFETY: creates an owned reference to the application, released below.
        let app = unsafe { AXUIElementCreateApplication(simulator_pid) };
        if app.is_null() {
            return Err(Error::failed("Could not reach Simulator.app. Is it open?"));
        }
        // SAFETY: `app` is the element created above.
        unsafe { AXUIElementSetMessagingTimeout(app, 4.0) };
        let result = find_screen(app, name, os);
        // SAFETY: balances AXUIElementCreateApplication.
        unsafe { CFRelease(app) };
        result
    }

    fn find_screen(app: AxElement, name: &str, os: Option<&str>) -> Result<AxNode> {
        let mut windows: Vec<(String, AxElement)> = Vec::new();
        let list = attribute(app, "AXWindows");
        if let Some(list) = &list {
            let array = list.as_CFTypeRef() as CFArrayRef;
            // SAFETY: a CFArray of AXUIElements, kept alive by `list` until the end of this function.
            unsafe {
                for i in 0..CFArrayGetCount(array) {
                    let w = CFArrayGetValueAtIndex(array, i);
                    windows.push((string(w, "AXTitle"), w));
                }
            }
        }
        // Titles read `iPhone 17 – iOS 26.2`.
        let wanted = |title: &str| title.starts_with(name) && os.is_none_or(|v| title.contains(v));
        let window = windows
            .iter()
            .find(|(t, _)| wanted(t))
            .or_else(|| windows.iter().find(|(t, _)| t.starts_with(name)))
            .ok_or_else(|| {
                Error::failed(format!(
                    "The window of “{name}” is not open. Open it in Simulator (File, Open Simulator) and try again."
                ))
            })?
            .1;

        // The screen is the largest group in the window; the rest is the buttons around it.
        let mut best: Option<(f64, Held)> = None;
        each_child(window, |child| {
            if string(child, "AXRole") == "AXGroup" {
                let f = frame(child);
                if best.as_ref().is_none_or(|(area, _)| f.width * f.height > *area) {
                    best = Some((f.width * f.height, Held::new(child)));
                }
            }
        });
        let (_, screen) =
            best.ok_or_else(|| Error::failed("The simulator window has no screen. Is the device booted?"))?;

        // The tree fills in as it is asked for; give it a moment the first time.
        let mut node = AxNode::default();
        for attempt in 0..6 {
            let mut budget = MAX_NODES;
            node = read(screen.0, 0, &mut budget);
            if count(&node) > 1 {
                break;
            }
            if attempt < 5 {
                std::thread::sleep(Duration::from_millis(300));
            }
        }
        if count(&node) <= 1 {
            return Err(Error::failed(
                "The simulator shows nothing to read. Wake the device, and check that Accessibility is allowed for this program.",
            ));
        }
        Ok(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ax(role: &str, description: &str, frame: (f64, f64, f64, f64)) -> AxNode {
        AxNode {
            role: role.into(),
            description: description.into(),
            enabled: true,
            frame: Frame { x: frame.0, y: frame.1, width: frame.2, height: frame.3 },
            ..Default::default()
        }
    }

    /// The numbers of a real iPhone 17 (402 × 874 pt) shown at 545 × 1185 on the Mac, from a dump of Settings.
    fn settings() -> (AxNode, Mapping) {
        let mut screen = ax("AXGroup", "", (1327.0, 139.0, 545.0, 1185.0));
        screen.children.push(ax("AXHeading", "Ajustes", (1348.0, 301.0, 162.0, 55.0)));
        let mut general = ax("AXButton", "Geral", (1348.0, 561.0, 501.0, 70.0));
        general.identifier = "com.apple.settings.general".into();
        screen.children.push(general);
        let mut wifi = ax("AXSwitch", "Wi-Fi", (1700.0, 700.0, 70.0, 40.0));
        wifi.value = "1".into();
        screen.children.push(wifi);
        screen.children.push(ax("AXImage", "", (1360.0, 800.0, 40.0, 40.0)));
        let map = Mapping::new(screen.frame, 402.0).unwrap();
        (screen, map)
    }

    #[test]
    fn frames_become_points_relative_to_the_screen() {
        let (screen, map) = settings();
        let root = to_layout(&screen, &map);
        assert_eq!((root.class.as_str(), root.bounds), ("Screen", Bounds::new(0, 0, 402, 874)));
        let general = &root.children[1];
        // (1348 - 1327) / 1.356 = 15.5 points from the left; 501 / 1.356 = 369.5 wide.
        assert_eq!((general.bounds.x, general.bounds.width, general.bounds.height), (15, 370, 52));
        assert_eq!(
            (general.label.as_deref(), general.resource_id.as_deref()),
            (Some("Geral"), Some("com.apple.settings.general"))
        );
        assert!(general.clickable && general.focusable);
    }

    #[test]
    fn headings_and_text_show_text_and_controls_carry_labels() {
        let (screen, map) = settings();
        let root = to_layout(&screen, &map);
        let heading = &root.children[0];
        assert_eq!(
            (heading.text.as_deref(), heading.label.as_deref(), heading.clickable),
            (Some("Ajustes"), None, false)
        );
    }

    #[test]
    fn switches_report_their_state() {
        let (screen, map) = settings();
        let toggle = &to_layout(&screen, &map).children[2];
        assert_eq!((toggle.checked, toggle.label.as_deref()), (Some(true), Some("Wi-Fi")));
    }

    #[test]
    fn an_image_without_a_label_has_no_name() {
        let (screen, map) = settings();
        let image = &to_layout(&screen, &map).children[3];
        assert_eq!(image.name(), None);
        assert!(!image.clickable);
    }

    #[test]
    fn the_audit_runs_on_what_ios_gives() {
        use emulabzip_core::a11y::audit;
        use emulabzip_core::{LayoutSnapshot, Platform, Screen};
        let (mut screen, map) = settings();
        // A tappable icon with no label, and a 20-point target.
        let mut icon = ax("AXButton", "", (1340.0, 900.0, 27.0, 27.0));
        icon.title = String::new();
        screen.children.push(icon);
        let root = to_layout(&screen, &map);
        let snapshot =
            LayoutSnapshot::new(Platform::Ios, root, Screen { width: 402, height: 874, density: 1.0 }, Vec::new());
        let result = audit(&snapshot);
        let rules: Vec<_> = result.findings.iter().map(|f| f.rule.as_str()).collect();
        assert!(rules.contains(&"missing-label") && rules.contains(&"touch-target"), "{rules:?}");
        // “Geral” is 370 × 52 pt, which passes the 44 pt rule.
        assert!(result.findings.iter().all(|f| f.node != 2));
    }

    #[test]
    fn a_device_profile_gives_points() {
        let profile = serde_json::json!({"mainScreenWidth": 1206, "mainScreenHeight": 2622, "mainScreenScale": 3});
        let p = ScreenProfile::from_profile(&profile).unwrap();
        assert_eq!(p.points(), (402.0, 874.0));
        assert!(ScreenProfile::from_profile(&serde_json::json!({})).is_none());
        assert!(Mapping::new(Frame::default(), 402.0).is_none());
    }
}
