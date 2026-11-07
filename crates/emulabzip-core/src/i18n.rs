//! Checks that find text that does not fit: cut off, pushed off the screen, clipped by its parent, or still laid out
//! left to right in a right-to-left language.
//!
//! They work from a [`LayoutSnapshot`] and its screenshot, so they can be run for any language the device can show,
//! including the pseudo-locales `en-XA` (long, accented text) and `ar-XB` (right to left), and at any text size. The
//! layout tells where each view is; the screenshot tells whether its text reaches the edge of its box. That is a
//! heuristic, and the findings say "may be".

use std::collections::HashMap;

use crate::a11y::{Audit, Finding, Image, Severity, contrast_ratio, decode_png};
use crate::layout::{Bounds, LayoutSnapshot, UiNode};

/// A box whose end is this close to the end of the container it was held to counts as having used all the room, in dp.
/// It is more than the gap before a neighbour, because containers have padding.
const FILLS_ROOM_DP: f32 = 20.0;
/// The gap before the next view in the row that still counts as being held back by it, in dp. Wider gaps are spacing the
/// layout chose, which a label that hugs its text has too.
const FILLS_NEIGHBOUR_DP: f32 = 4.0;
/// An ancestor this much wider than the box is what the box was held to; narrower ones only wrap it.
const ROOM_DP: f32 = 24.0;

/// A single line of text is at most this tall, in dp.
const SINGLE_LINE_DP: f32 = 56.0;

#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    /// The screen is shown in a right-to-left language.
    pub rtl: bool,
}

/// Whether a language tag is written right to left: Arabic, Hebrew, Persian, Urdu and a few more, and `ar-XB`, the
/// pseudo-locale made for it.
pub fn is_rtl(tag: &str) -> bool {
    let lang = tag.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
    matches!(lang.as_str(), "ar" | "he" | "iw" | "fa" | "ur" | "ps" | "sd" | "ug" | "yi" | "dv" | "ckb")
}

fn shorten(text: &str) -> String {
    let t = text.trim();
    if t.chars().count() > 40 { format!("{}…", t.chars().take(40).collect::<String>()) } else { t.to_string() }
}

fn finding(node: &UiNode, rule: &str, severity: Severity, message: String, wcag: &str, fix: &str) -> Finding {
    Finding {
        rule: rule.into(),
        severity,
        node: node.id,
        class: node.class.clone(),
        resource_id: node.resource_id.clone(),
        bounds: node.bounds,
        message,
        wcag: wcag.into(),
        fix: fix.into(),
    }
}

pub fn check(snapshot: &LayoutSnapshot, options: Options) -> Audit {
    let image = decode_png(&snapshot.screenshot);
    let mut audit = Audit { contrast_skipped: image.is_none(), ..Default::default() };
    visit(&snapshot.root, &mut Vec::new(), false, snapshot, image.as_ref(), &mut audit);
    if options.rtl {
        mirrored(snapshot, &mut audit);
    }
    audit.findings.sort_by(|a, b| b.severity.cmp(&a.severity).then(a.node.cmp(&b.node)));
    audit
}

fn text_of(node: &UiNode) -> Option<&str> {
    node.text.as_deref().filter(|t| !t.trim().is_empty())
}

fn visit<'a>(
    node: &'a UiNode,
    path: &mut Vec<&'a UiNode>,
    in_scroll: bool,
    snapshot: &LayoutSnapshot,
    image: Option<&Image>,
    audit: &mut Audit,
) {
    audit.checked += 1;
    let parent = path.last().copied();
    let density = snapshot.screen.density.max(0.1);
    // A little slack for rounding, in the units of the bounds.
    let tol = density.ceil() as i32 + 1;
    if let Some(text) = text_of(node).filter(|_| !node.bounds.is_empty()) {
        let b = node.bounds;
        let screen = snapshot.screen;
        let off_screen = !in_scroll && (b.x + b.width > screen.width + tol || b.x < -tol);
        if off_screen {
            audit.findings.push(finding(
                node,
                "text-off-screen",
                Severity::Warning,
                format!("“{}” reaches past the edge of the screen.", shorten(text)),
                "1.4.10 Reflow",
                "Let the text wrap, give it less room to grow into, or shorten it. Longer languages need about 40% more space.",
            ));
        } else if let Some(p) = parent.filter(|p| !p.scrollable && !in_scroll && !p.bounds.is_empty()) {
            let pb = p.bounds;
            if b.x + b.width > pb.x + pb.width + tol
                || b.x < pb.x - tol
                || b.y + b.height > pb.y + pb.height + tol
                || b.y < pb.y - tol
            {
                audit.findings.push(finding(
                    node,
                    "text-clipped-by-parent",
                    Severity::Warning,
                    format!("“{}” does not fit inside its container, so the container cuts it.", shorten(text)),
                    "1.4.4 Resize Text",
                    "Let the container grow with its content (wrap_content), or let the text wrap.",
                ));
            }
        }
        let single_line = b.height as f32 <= SINGLE_LINE_DP * density;
        let fills_room = uses_all_its_room(node, path, snapshot);
        if let Some(image) = image
            && single_line
            && !node.editable
            && text.chars().count() >= 4
            && !off_screen
            && fills_room
            && runs_to_the_edge(image, snapshot, b)
        {
            audit.findings.push(finding(
                node,
                "text-cut-off",
                Severity::Warning,
                format!("“{}” runs right up to the edge of its box, so part of it may be cut off.", shorten(text)),
                "1.4.4 Resize Text, 1.4.10 Reflow",
                "Let the text wrap (no maxLines=1), give the box more room, or shorten the text. Check it at 130% and 200% text size.",
            ));
        }
    }
    let scroll = in_scroll || node.scrollable;
    path.push(node);
    for child in &node.children {
        visit(child, path, scroll, snapshot, image, audit);
    }
    path.pop();
}

/// A box that hugs its text (wrap_content) always has ink at its edge, so ink there says nothing. Text is cut only when its
/// box was held to the room it was given: its end is at, or close to, the start of the next view in its row, or the end of
/// the nearest container that is wider than the box.
fn uses_all_its_room(node: &UiNode, path: &[&UiNode], snapshot: &LayoutSnapshot) -> bool {
    let density = snapshot.screen.density.max(0.1);
    let b = node.bounds;
    let end = b.x + b.width;
    let wider = (ROOM_DP * density) as i32;
    let container_end = path
        .iter()
        .rev()
        .find(|a| a.bounds.width > b.width + wider)
        .map_or(snapshot.screen.width, |a| a.bounds.x + a.bounds.width);
    let next_start = path
        .last()
        .into_iter()
        .flat_map(|p| p.children.iter())
        .filter(|s| !std::ptr::eq(*s, node) && !s.bounds.is_empty())
        .filter(|s| {
            // In the same row: they share at least half of the shorter one's height.
            let shared = (b.y + b.height).min(s.bounds.y + s.bounds.height) - b.y.max(s.bounds.y);
            shared * 2 > b.height.min(s.bounds.height) && s.bounds.x >= end - 2
        })
        .map(|s| s.bounds.x)
        .min()
        .unwrap_or(i32::MAX);
    container_end - end <= (FILLS_ROOM_DP * density) as i32
        || next_start.saturating_sub(end) <= (FILLS_NEIGHBOUR_DP * density) as i32
}

/// Whether there is ink in the last few pixels of the box: a label that ends well inside its box leaves them empty, and
/// one that was cut off, or ends in an ellipsis, reaches them.
fn runs_to_the_edge(image: &Image, snapshot: &LayoutSnapshot, bounds: Bounds) -> bool {
    let (sx, sy) = (
        f64::from(image.width) / f64::from(snapshot.screen.width.max(1)),
        f64::from(image.height) / f64::from(snapshot.screen.height.max(1)),
    );
    let x0 = (f64::from(bounds.x) * sx).floor().max(0.0) as u32;
    let y0 = (f64::from(bounds.y) * sy).floor().max(0.0) as u32;
    let x1 = ((f64::from(bounds.x + bounds.width) * sx).ceil() as u32).min(image.width);
    let y1 = ((f64::from(bounds.y + bounds.height) * sy).ceil() as u32).min(image.height);
    if x1 <= x0 + 12 || y1 <= y0 + 4 {
        return false;
    }
    let (top, bottom) = (y0 + (y1 - y0) / 5, y1 - (y1 - y0) / 5);
    // The background is the colour most of the box has.
    let mut buckets: HashMap<u16, (u32, [u8; 3])> = HashMap::new();
    let mut y = y0;
    while y < y1 {
        let mut x = x0;
        while x < x1 {
            let p = image.pixel(x, y);
            let key = (u16::from(p[0] >> 4) << 8) | (u16::from(p[1] >> 4) << 4) | u16::from(p[2] >> 4);
            buckets.entry(key).or_insert((0, p)).0 += 1;
            x += 2;
        }
        y += 2;
    }
    let Some((_, bg)) = buckets.values().max_by_key(|(n, p)| (*n, p[0], p[1], p[2])).copied() else { return false };
    let ink = |x: u32, y: u32| contrast_ratio(image.pixel(x, y), bg) > 1.8;

    // An ellipsis stops a glyph or two short of the edge, so the strip is wider than a few pixels.
    let strip = (((20.0 * f64::from(snapshot.screen.density) * sx) as u32).max(3)).min((x1 - x0) / 4).max(3);
    let (mut inked, mut seen) = (0u32, 0u32);
    for y in top..bottom {
        for x in (x1 - strip)..x1 {
            seen += 1;
            if ink(x, y) {
                inked += 1;
            }
        }
    }
    // Some ink in the strip, and text in the rest of the box (an empty box is not text that is cut).
    let mut body = 0u32;
    for y in (top..bottom).step_by(2) {
        for x in (x0..x1 - strip).step_by(2) {
            if ink(x, y) {
                body += 1;
            }
        }
    }
    seen > 0 && inked * 100 >= seen * 3 && body > 8
}

/// In a right-to-left language the text of a screen sits on the right. When most of it still hugs the left edge, the layout
/// was not mirrored.
fn mirrored(snapshot: &LayoutSnapshot, audit: &mut Audit) {
    let (mut left, mut right) = (0u32, 0u32);
    let margin = (8.0 * snapshot.screen.density) as i32;
    snapshot.root.walk(&mut |n| {
        if text_of(n).is_none() || n.bounds.is_empty() || n.bounds.width * 10 > snapshot.screen.width * 6 {
            return;
        }
        let from_left = n.bounds.x;
        let from_right = snapshot.screen.width - (n.bounds.x + n.bounds.width);
        if from_left + margin < from_right {
            left += 1;
        } else if from_right + margin < from_left {
            right += 1;
        }
    });
    let total = left + right;
    if total >= 4 && left * 10 >= total * 7 {
        audit.findings.push(finding(
            &snapshot.root,
            "rtl-not-mirrored",
            Severity::Warning,
            format!("In a right-to-left language {left} of {total} texts are still on the left of the screen: the layout was not mirrored."),
            "1.3.2 Meaningful Sequence",
            "Use start and end instead of left and right (Android: supportsRtl=\"true\" and layout_gravity=\"start\"; iOS: leading and trailing).",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Platform;
    use crate::layout::Screen;
    use crate::layout::fixtures::node;

    /// A 400 × 400 screenshot, painted by `paint`.
    fn png(paint: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, 400, 400);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().unwrap();
            let mut data = Vec::new();
            for y in 0..400 {
                for x in 0..400 {
                    data.extend_from_slice(&paint(x, y));
                }
            }
            w.write_image_data(&data).unwrap();
        }
        out
    }

    fn snap(root: UiNode, screenshot: Vec<u8>) -> LayoutSnapshot {
        LayoutSnapshot::new(Platform::Android, root, Screen { width: 400, height: 400, density: 1.0 }, screenshot)
    }

    fn text(class: &str, bounds: (i32, i32, i32, i32), t: &str) -> UiNode {
        let mut n = node(class, bounds);
        n.text = Some(t.into());
        n
    }

    fn rules(a: &Audit) -> Vec<&str> {
        a.findings.iter().map(|f| f.rule.as_str()).collect()
    }

    #[test]
    fn right_to_left_languages_are_told_by_their_tag() {
        for rtl in ["ar", "ar-XB", "he-IL", "fa", "ur_PK", "AR"] {
            assert!(is_rtl(rtl), "{rtl}");
        }
        for ltr in ["en", "en-XA", "de-DE", "pt-BR", "zh-Hans", ""] {
            assert!(!is_rtl(ltr), "{ltr}");
        }
    }

    #[test]
    fn text_pushed_past_the_screen_is_found_unless_it_scrolls() {
        let mut root = node("FrameLayout", (0, 0, 400, 400));
        root.children.push(text("TextView", (300, 10, 200, 30), "A long label in another language"));
        let a = check(&snap(root.clone(), Vec::new()), Options::default());
        assert_eq!(rules(&a), ["text-off-screen"]);
        assert!(a.contrast_skipped);

        let mut list = node("HorizontalScrollView", (0, 0, 400, 100));
        list.scrollable = true;
        list.children.push(text("TextView", (300, 10, 200, 30), "Scrolls away"));
        let mut root = node("FrameLayout", (0, 0, 400, 400));
        root.children.push(list);
        assert!(check(&snap(root, Vec::new()), Options::default()).findings.is_empty());
    }

    #[test]
    fn text_a_container_cuts_is_found() {
        let mut card = node("LinearLayout", (20, 20, 150, 60));
        card.children.push(text("TextView", (30, 30, 200, 30), "Does not fit"));
        let mut root = node("FrameLayout", (0, 0, 400, 400));
        root.children.push(card);
        let a = check(&snap(root, Vec::new()), Options::default());
        assert_eq!(rules(&a), ["text-clipped-by-parent"]);
    }

    #[test]
    fn a_label_that_runs_to_the_edge_of_its_box_may_be_cut() {
        // A white screen; the box (20, 100) to (220, 130) has dark text across all of it, or only across its first half.
        let shot = |ink_to: u32| {
            png(move |x, y| {
                if (100..130).contains(&y) && (22..ink_to).contains(&x) && (x / 3 + y / 3) % 2 == 0 {
                    [20, 20, 20]
                } else {
                    [255, 255, 255]
                }
            })
        };
        let layout = |width: i32| {
            let mut root = node("FrameLayout", (0, 0, 400, 400));
            root.children.push(text("TextView", (20, 100, width, 30), "Order confirmation details"));
            root
        };
        // The box fills the screen's width (20 to 390 of 400) and the text runs to its end.
        let cut = check(&snap(layout(370), shot(389)), Options::default());
        assert_eq!(rules(&cut), ["text-cut-off"], "{:?}", cut.findings);
        assert!(!cut.contrast_skipped);
        let fits = check(&snap(layout(370), shot(120)), Options::default());
        assert!(fits.findings.is_empty(), "{:?}", fits.findings);
        // A box that hugs its text has ink at its edge by design: it is not cut.
        let hugs = check(&snap(layout(200), shot(219)), Options::default());
        assert!(hugs.findings.is_empty(), "{:?}", hugs.findings);

        // A label held back by the icon beside it is cut at the icon.
        let mut row = node("LinearLayout", (0, 0, 400, 400));
        row.children.push(text("TextView", (20, 100, 300, 30), "Order confirmation details"));
        row.children.push(node("ImageView", (322, 100, 40, 30)));
        let beside = check(&snap(row.clone(), shot(319)), Options::default());
        assert_eq!(rules(&beside), ["text-cut-off"], "{:?}", beside.findings);
        // The same label with room to spare before its neighbour is only hugging its text.
        row.children[1] = node("ImageView", (360, 100, 30, 30));
        assert!(check(&snap(row, shot(319)), Options::default()).findings.is_empty());

        // A box with several lines of text is not a single line, so it is left alone.
        let mut root = node("FrameLayout", (0, 0, 400, 400));
        root.children.push(text("TextView", (20, 100, 370, 120), "Order confirmation details"));
        assert!(check(&snap(root, shot(389)), Options::default()).findings.is_empty());
    }

    #[test]
    fn a_layout_that_was_not_mirrored_is_found_only_for_right_to_left_languages() {
        let mut root = node("FrameLayout", (0, 0, 400, 400));
        for i in 0..5 {
            root.children.push(text("TextView", (16, 20 + i * 40, 120, 30), &format!("Row {i}")));
        }
        let ltr = check(&snap(root.clone(), Vec::new()), Options { rtl: false });
        assert!(ltr.findings.is_empty());
        let rtl = check(&snap(root, Vec::new()), Options { rtl: true });
        assert_eq!(rules(&rtl), ["rtl-not-mirrored"]);
        assert!(rtl.findings[0].message.contains("5 of 5"));

        let mut mirrored = node("FrameLayout", (0, 0, 400, 400));
        for i in 0..5 {
            mirrored.children.push(text("TextView", (264, 20 + i * 40, 120, 30), &format!("Row {i}")));
        }
        assert!(check(&snap(mirrored, Vec::new()), Options { rtl: true }).findings.is_empty());
    }
}
