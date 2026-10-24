//! An accessibility audit of a [`LayoutSnapshot`]: what a screen reader or a thumb would trip over.
//!
//! It checks four things that can be found from the layout alone and from the screenshot taken with
//! it: controls without a name, touch targets that are too small, text that is hard to read against
//! its background, and neighbours that sound the same. It is a first pass, not a replacement for
//! trying the app with TalkBack or VoiceOver.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::layout::{Bounds, LayoutSnapshot, UiNode};

/// Text needs this much contrast against its background (WCAG 1.4.3, level AA).
pub const MIN_CONTRAST: f64 = 4.5;
/// Large text needs less.
pub const MIN_CONTRAST_LARGE: f64 = 3.0;
/// Text this tall (in dp or pt) counts as large: 18 pt is about 24 dp.
const LARGE_TEXT_DP: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }

    pub fn parse(text: &str) -> Option<Severity> {
        match text.to_ascii_lowercase().as_str() {
            "info" => Some(Severity::Info),
            "warning" | "warn" => Some(Severity::Warning),
            "error" => Some(Severity::Error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// `missing-label`, `touch-target`, `contrast`, `duplicate-label` or `unlabeled-focusable`.
    pub rule: String,
    pub severity: Severity,
    pub node: u32,
    pub class: String,
    pub resource_id: Option<String>,
    pub bounds: Bounds,
    pub message: String,
    /// The success criterion it relates to.
    pub wcag: String,
    pub fix: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Audit {
    pub findings: Vec<Finding>,
    /// Views that were looked at.
    pub checked: u32,
    /// The contrast could not be measured, because the layout came without a screenshot.
    pub contrast_skipped: bool,
}

impl Audit {
    pub fn count(&self, severity: Severity) -> usize {
        self.findings.iter().filter(|f| f.severity == severity).count()
    }

    /// Whether any finding is at least this serious.
    pub fn fails(&self, at_least: Severity) -> bool {
        self.findings.iter().any(|f| f.severity >= at_least)
    }
}

/// Runs every check.
pub fn audit(snapshot: &LayoutSnapshot) -> Audit {
    let image = decode_png(&snapshot.screenshot);
    let mut audit = Audit { contrast_skipped: image.is_none(), ..Default::default() };
    let min_target = snapshot.min_target();
    let (unit, noun) = match snapshot.platform {
        crate::Platform::Android => (48, "dp"),
        crate::Platform::Ios => (44, "pt"),
    };

    snapshot.root.walk(&mut |node| {
        audit.checked += 1;
        if node.bounds.is_empty() {
            return;
        }
        let mut add = |rule: &str, severity, message: String, wcag: &str, fix: &str| {
            audit.findings.push(Finding {
                rule: rule.into(),
                severity,
                node: node.id,
                class: node.class.clone(),
                resource_id: node.resource_id.clone(),
                bounds: node.bounds,
                message,
                wcag: wcag.into(),
                fix: fix.into(),
            });
        };

        if node.interactive() && node.name().is_none() {
            add(
                "missing-label",
                Severity::Error,
                format!("This {} can be tapped, but a screen reader has nothing to say for it.", node.short_class()),
                "1.1.1 Non-text Content, 4.1.2 Name, Role, Value",
                "Give it a content description (Android: android:contentDescription) or an accessibility label (iOS).",
            );
        } else if node.focusable && !node.interactive() && node.children.is_empty() && node.own_name().is_none() {
            add(
                "unlabeled-focusable",
                Severity::Warning,
                format!("This {} takes focus, but has no name.", node.short_class()),
                "4.1.2 Name, Role, Value",
                "Label it, or take it out of the focus order (importantForAccessibility=\"no\").",
            );
        }

        if node.interactive() {
            let small = (node.bounds.width as f32) < min_target - 0.5 || (node.bounds.height as f32) < min_target - 0.5;
            if small {
                let dp = |px: i32| (px as f32 / snapshot.screen.density).round() as i32;
                add(
                    "touch-target",
                    Severity::Warning,
                    format!(
                        "The touch target is {} × {} {noun}; at least {unit} × {unit} {noun} is recommended.",
                        dp(node.bounds.width),
                        dp(node.bounds.height)
                    ),
                    "2.5.8 Target Size",
                    "Make the view larger, or add padding (Android: minWidth/minHeight, or a TouchDelegate).",
                );
            }
        }

        if let (Some(image), Some(text)) = (&image, node.text.as_deref().filter(|t| !t.trim().is_empty()))
            && let Some(ratio) = contrast_in(image, snapshot, node.bounds)
        {
            let large = node.bounds.height as f32 / snapshot.screen.density >= LARGE_TEXT_DP;
            let needed = if large { MIN_CONTRAST_LARGE } else { MIN_CONTRAST };
            if ratio < needed {
                add(
                    "contrast",
                    if ratio < 2.0 { Severity::Error } else { Severity::Warning },
                    format!("“{}” has a contrast of {:.1}:1; {:.1}:1 is needed.", shorten(text), ratio, needed),
                    "1.4.3 Contrast (Minimum)",
                    "Use a darker text colour or a lighter background.",
                );
            }
        }
    });

    duplicates(&snapshot.root, &mut audit.findings);
    audit.findings.sort_by(|a, b| b.severity.cmp(&a.severity).then(a.node.cmp(&b.node)));
    audit
}

fn shorten(text: &str) -> String {
    let t = text.trim();
    if t.chars().count() > 40 { format!("{}…", t.chars().take(40).collect::<String>()) } else { t.to_string() }
}

/// Neighbouring controls that a screen reader would announce identically.
fn duplicates(root: &UiNode, findings: &mut Vec<Finding>) {
    root.walk(&mut |parent| {
        let mut seen: HashMap<&str, Vec<&UiNode>> = HashMap::new();
        for child in parent.children.iter().filter(|c| c.interactive() && !c.bounds.is_empty()) {
            if let Some(name) = child.name() {
                seen.entry(name).or_default().push(child);
            }
        }
        for (name, nodes) in seen.into_iter().filter(|(_, n)| n.len() > 1) {
            for node in nodes.iter().skip(1) {
                findings.push(Finding {
                    rule: "duplicate-label".into(),
                    severity: Severity::Info,
                    node: node.id,
                    class: node.class.clone(),
                    resource_id: node.resource_id.clone(),
                    bounds: node.bounds,
                    message: format!("{} neighbouring controls are all announced as “{}”.", nodes.len(), shorten(name)),
                    wcag: "2.4.6 Headings and Labels".into(),
                    fix: "Say what each one does (“Delete photo 3”), so they can be told apart.".into(),
                });
            }
        }
    });
}

// ---------------------------------------------------------------------------------------------
// Contrast
// ---------------------------------------------------------------------------------------------

/// A screenshot decoded to RGBA.
pub struct Image {
    pub width: u32,
    pub height: u32,
    rgba: Vec<u8>,
}

impl Image {
    fn pixel(&self, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * self.width + x) * 4) as usize;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]]
    }
}

pub fn decode_png(bytes: &[u8]) -> Option<Image> {
    if bytes.is_empty() {
        return None;
    }
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8() | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.as_chunks::<3>().0.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => {
            buf.as_chunks::<2>().0.iter().flat_map(|p| [p[0], p[0], p[0], p[1]]).collect()
        }
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    Some(Image { width: info.width, height: info.height, rgba })
}

/// WCAG relative luminance of an sRGB colour.
pub fn luminance([r, g, b]: [u8; 3]) -> f64 {
    let channel = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

/// WCAG contrast ratio, from 1 to 21.
pub fn contrast_ratio(a: [u8; 3], b: [u8; 3]) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// The contrast between the text and the background inside a view, read from the screenshot.
/// `None` when it cannot be told: the view is off screen, or the background is too busy (a photo).
fn contrast_in(image: &Image, snapshot: &LayoutSnapshot, bounds: Bounds) -> Option<f64> {
    let (sx, sy) = (
        f64::from(image.width) / f64::from(snapshot.screen.width.max(1)),
        f64::from(image.height) / f64::from(snapshot.screen.height.max(1)),
    );
    let x0 = ((f64::from(bounds.x) * sx).floor().max(0.0)) as u32;
    let y0 = ((f64::from(bounds.y) * sy).floor().max(0.0)) as u32;
    let x1 = (((f64::from(bounds.x + bounds.width)) * sx).ceil() as u32).min(image.width);
    let y1 = (((f64::from(bounds.y + bounds.height)) * sy).ceil() as u32).min(image.height);
    if x1 <= x0 + 1 || y1 <= y0 + 1 {
        return None;
    }
    // Look at no more than about 6,000 pixels.
    let step = (((x1 - x0) as f64 * (y1 - y0) as f64 / 6000.0).sqrt().ceil() as u32).max(1);

    // Group colours into buckets of 16 levels per channel and keep the average of each.
    let mut buckets: HashMap<u16, (u32, [u32; 3])> = HashMap::new();
    let mut total = 0u32;
    let mut y = y0;
    while y < y1 {
        let mut x = x0;
        while x < x1 {
            let p = image.pixel(x, y);
            let key = (u16::from(p[0] >> 4) << 8) | (u16::from(p[1] >> 4) << 4) | u16::from(p[2] >> 4);
            let e = buckets.entry(key).or_insert((0, [0; 3]));
            e.0 += 1;
            for (sum, c) in e.1.iter_mut().zip(p) {
                *sum += u32::from(c);
            }
            total += 1;
            x += step;
        }
        y += step;
    }
    let colour = |(n, sum): &(u32, [u32; 3])| [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8];
    let (_, background) = buckets.iter().max_by_key(|(k, (n, _))| (*n, **k))?;
    // A background that covers less than a third of the box is a picture, not a surface.
    if background.0 * 3 < total {
        return None;
    }
    let bg = colour(background);
    // The text colour is the one that stands furthest from the background, among colours that
    // cover enough of the box not to be noise.
    let floor = (total / 50).max(2);
    let best = buckets.values().filter(|b| b.0 >= floor).map(|b| contrast_ratio(colour(b), bg)).fold(1.0_f64, f64::max);
    Some(best)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Platform;
    use crate::layout::fixtures::*;
    use crate::layout::{LayoutSnapshot, Screen};

    fn png_of(width: u32, height: u32, paint: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, width, height);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().unwrap();
            let mut data = Vec::new();
            for y in 0..height {
                for x in 0..width {
                    data.extend_from_slice(&paint(x, y));
                }
            }
            w.write_image_data(&data).unwrap();
        }
        out
    }

    fn with(root: UiNode, screenshot: Vec<u8>) -> LayoutSnapshot {
        LayoutSnapshot::new(Platform::Android, root, Screen { width: 400, height: 400, density: 1.0 }, screenshot)
    }

    fn rules(a: &Audit) -> Vec<&str> {
        a.findings.iter().map(|f| f.rule.as_str()).collect()
    }

    #[test]
    fn contrast_matches_the_wcag_examples() {
        assert!((contrast_ratio([0, 0, 0], [255, 255, 255]) - 21.0).abs() < 0.01);
        assert!((contrast_ratio([255, 255, 255], [255, 255, 255]) - 1.0).abs() < 0.001);
        // #777 on white just fails AA, #767676 just passes: the well-known pair.
        assert!(contrast_ratio([0x77, 0x77, 0x77], [255, 255, 255]) < 4.5);
        assert!(contrast_ratio([0x76, 0x76, 0x76], [255, 255, 255]) >= 4.5);
    }

    #[test]
    fn a_tappable_image_without_a_name_is_an_error() {
        let mut root = node("FrameLayout", (0, 0, 400, 400));
        let mut icon = node("ImageButton", (0, 0, 120, 120));
        icon.clickable = true;
        root.children.push(icon);
        let a = audit(&with(root, Vec::new()));
        assert_eq!(rules(&a), ["missing-label"]);
        assert_eq!(a.findings[0].severity, Severity::Error);
        assert!(a.contrast_skipped);
    }

    #[test]
    fn a_name_can_come_from_the_label_the_text_or_a_child() {
        let mut root = node("FrameLayout", (0, 0, 400, 400));
        for i in 0..3 {
            let mut n = node("View", (0, i * 130, 130, 130));
            n.clickable = true;
            match i {
                0 => n.label = Some("Close".into()),
                1 => n.text = Some("Save".into()),
                _ => {
                    let mut t = node("TextView", (0, i * 130, 100, 40));
                    t.text = Some("Share".into());
                    n.children.push(t);
                }
            }
            root.children.push(n);
        }
        assert!(audit(&with(root, Vec::new())).findings.is_empty());
    }

    #[test]
    fn small_targets_are_found_and_sized_in_dp() {
        let mut root = node("FrameLayout", (0, 0, 400, 400));
        let mut small = node("Button", (0, 0, 40, 40));
        small.clickable = true;
        small.text = Some("OK".into());
        root.children.push(small);
        let mut big = node("Button", (0, 100, 150, 150));
        big.clickable = true;
        big.text = Some("Cancel".into());
        root.children.push(big);
        let a = audit(&with(root, Vec::new()));
        assert_eq!(rules(&a), ["touch-target"]);
        assert!(a.findings[0].message.contains("40 × 40 dp"), "{}", a.findings[0].message);
    }

    #[test]
    fn a_disabled_control_is_not_asked_to_be_big_or_named() {
        let mut root = node("FrameLayout", (0, 0, 400, 400));
        let mut off = node("Button", (0, 0, 20, 20));
        off.clickable = true;
        off.enabled = false;
        root.children.push(off);
        assert!(audit(&with(root, Vec::new())).findings.is_empty());
    }

    #[test]
    fn neighbours_with_the_same_name_are_noted() {
        let mut root = node("FrameLayout", (0, 0, 400, 400));
        for i in 0..3 {
            let mut n = node("Button", (0, i * 130, 130, 130));
            n.clickable = true;
            n.label = Some("Delete".into());
            root.children.push(n);
        }
        let a = audit(&with(root, Vec::new()));
        assert_eq!(rules(&a), ["duplicate-label", "duplicate-label"]);
        assert_eq!(a.findings[0].severity, Severity::Info);
    }

    #[test]
    fn an_unnamed_focusable_leaf_is_a_warning() {
        let mut root = node("FrameLayout", (0, 0, 400, 400));
        let mut n = node("View", (0, 0, 200, 200));
        n.focusable = true;
        root.children.push(n);
        assert_eq!(rules(&audit(&with(root, Vec::new()))), ["unlabeled-focusable"]);
    }

    /// Text drawn as thin dark-on-light (or light-on-light) strokes on a plain background.
    fn text_png(fg: [u8; 3], bg: [u8; 3]) -> Vec<u8> {
        png_of(400, 400, |x, y| {
            if (100..300).contains(&x) && (180..220).contains(&y) && (x / 6 + y / 4) % 2 == 0 { fg } else { bg }
        })
    }

    fn text_tree() -> UiNode {
        let mut root = node("FrameLayout", (0, 0, 400, 400));
        let mut t = node("TextView", (100, 180, 200, 40));
        t.text = Some("Hello there".into());
        root.children.push(t);
        root
    }

    #[test]
    fn faint_text_fails_and_dark_text_passes() {
        let faint = audit(&with(text_tree(), text_png([0xaa, 0xaa, 0xaa], [255, 255, 255])));
        assert_eq!(rules(&faint), ["contrast"], "{faint:?}");
        assert!(faint.findings[0].message.contains("Hello there"));
        assert!(!faint.contrast_skipped);

        let dark = audit(&with(text_tree(), text_png([0x20, 0x20, 0x20], [255, 255, 255])));
        assert!(dark.findings.is_empty(), "{dark:?}");
    }

    #[test]
    fn white_on_a_dark_surface_passes() {
        let a = audit(&with(text_tree(), text_png([255, 255, 255], [0x12, 0x12, 0x12])));
        assert!(a.findings.is_empty(), "{a:?}");
    }

    #[test]
    fn the_screenshot_may_be_larger_than_the_layout() {
        // An iPhone screenshot has 3 pixels per point.
        let shot = png_of(1200, 1200, |x, y| {
            if (300..900).contains(&x) && (540..660).contains(&y) && (x / 18 + y / 12) % 2 == 0 {
                [0xaa, 0xaa, 0xaa]
            } else {
                [255, 255, 255]
            }
        });
        let a = audit(&with(text_tree(), shot));
        assert_eq!(rules(&a), ["contrast"], "{a:?}");
    }

    #[test]
    fn a_busy_background_is_not_judged() {
        let noisy = png_of(400, 400, |x, y| [(x * 7 + y * 13) as u8, (x * 3) as u8, (y * 5) as u8]);
        assert!(audit(&with(text_tree(), noisy)).findings.is_empty());
    }

    #[test]
    fn results_are_ordered_by_severity_and_the_threshold_decides_failure() {
        let mut root = node("FrameLayout", (0, 0, 400, 400));
        let mut a = node("Button", (0, 0, 40, 40));
        a.clickable = true;
        a.text = Some("OK".into());
        let mut b = node("ImageButton", (0, 200, 150, 150));
        b.clickable = true;
        root.children.extend([a, b]);
        let audit = audit(&with(root, Vec::new()));
        assert_eq!(rules(&audit), ["missing-label", "touch-target"]);
        assert!(audit.fails(Severity::Error) && audit.fails(Severity::Info));
        assert_eq!((audit.count(Severity::Error), audit.count(Severity::Warning)), (1, 1));
        assert_eq!(Severity::parse("WARN"), Some(Severity::Warning));
    }
}
