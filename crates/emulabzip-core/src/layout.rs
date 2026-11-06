//! What is on a device's screen: a tree of views with their bounds, names and states.
//!
//! Android fills it from `uiautomator`, iOS from the accessibility tree. Both end up as the same
//! [`UiNode`], so the viewer and the accessibility audit work on either.

use serde::{Deserialize, Serialize};

use crate::Platform;

/// A rectangle in the layout's own units: pixels on Android, points on iOS.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Bounds {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Bounds { x, y, width, height }
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }

    pub fn area(&self) -> i64 {
        i64::from(self.width.max(0)) * i64::from(self.height.max(0))
    }
}

/// One view.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiNode {
    /// Position in a walk of the tree from the root, so a node can be named from the UI. Set by
    /// [`LayoutSnapshot::new`].
    pub id: u32,
    /// The view class (`android.widget.Button`) or the accessibility role (`AXButton`).
    pub class: String,
    /// The text it shows.
    pub text: Option<String>,
    /// What a screen reader says: the content description on Android, the label on iOS.
    pub label: Option<String>,
    /// The hint of an input field, or a value.
    pub hint: Option<String>,
    pub resource_id: Option<String>,
    pub package: Option<String>,
    pub bounds: Bounds,
    pub clickable: bool,
    pub focusable: bool,
    pub enabled: bool,
    pub checked: Option<bool>,
    pub scrollable: bool,
    /// Text can be typed into it.
    pub editable: bool,
    /// The UI framework that drew this view when it is not the platform's own: `compose`, `flutter`,
    /// `react-native` or `swiftui`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>,
    pub children: Vec<UiNode>,
}

impl UiNode {
    /// What a person using a screen reader would hear for this view alone.
    pub fn own_name(&self) -> Option<&str> {
        [self.label.as_deref(), self.text.as_deref(), self.hint.as_deref()]
            .into_iter()
            .flatten()
            .map(str::trim)
            .find(|s| !s.is_empty())
    }

    /// The name of the view, or, failing that, of the first thing inside it that has one: a
    /// clickable row with a text child is announced by that text.
    pub fn name(&self) -> Option<&str> {
        self.own_name().or_else(|| self.children.iter().find_map(UiNode::name))
    }

    /// Whether it responds to a tap or the keyboard.
    pub fn interactive(&self) -> bool {
        self.enabled && (self.clickable || self.editable)
    }

    pub fn short_class(&self) -> &str {
        self.class.rsplit('.').next().unwrap_or(&self.class)
    }

    /// Visits the node and everything under it, parents first.
    pub fn walk<'a>(&'a self, visit: &mut impl FnMut(&'a UiNode)) {
        visit(self);
        for child in &self.children {
            child.walk(visit);
        }
    }

    pub fn find(&self, id: u32) -> Option<&UiNode> {
        let mut found = None;
        self.walk(&mut |n| {
            if n.id == id {
                found = Some(n);
            }
        });
        found
    }

    /// The deepest node at a point, which is what a tap there would reach.
    pub fn at(&self, x: i32, y: i32) -> Option<&UiNode> {
        if !self.bounds.contains(x, y) {
            return None;
        }
        // Later siblings are drawn on top.
        self.children.iter().rev().find_map(|c| c.at(x, y)).or(Some(self))
    }

    pub fn count(&self) -> usize {
        1 + self.children.iter().map(UiNode::count).sum::<usize>()
    }

    /// Marks this view and everything under it as drawn by `framework`.
    pub fn mark_all(&mut self, framework: &str) {
        self.framework = Some(framework.to_string());
        for child in &mut self.children {
            child.mark_all(framework);
        }
    }

    /// Marks the views under the host views that frameworks put on screen: Compose's `ComposeView`, Flutter's
    /// `FlutterView`, React Native's `ReactRootView`. The host view itself is marked too.
    pub fn mark_hosts(&mut self) {
        if let Some(framework) = host_framework(&self.class) {
            self.mark_all(framework);
            return;
        }
        for child in &mut self.children {
            child.mark_hosts();
        }
    }
}

/// The framework a host view class belongs to.
pub fn host_framework(class: &str) -> Option<&'static str> {
    match class {
        "androidx.compose.ui.platform.ComposeView" | "androidx.compose.ui.platform.AndroidComposeView" => {
            Some("compose")
        }
        "io.flutter.embedding.android.FlutterView"
        | "io.flutter.embedding.android.FlutterSurfaceView"
        | "io.flutter.embedding.android.FlutterTextureView"
        | "io.flutter.embedding.android.FlutterImageView"
        | "io.flutter.view.FlutterView" => Some("flutter"),
        "com.facebook.react.ReactRootView" | "com.facebook.react.ReactFragment" => Some("react-native"),
        _ => None,
    }
}

/// Which framework to mark a whole screen with, given the frameworks an app is built with, for platforms (iOS) where
/// views have no host class to tell them apart. The framework that draws its own UI wins over the platform's.
pub fn screen_framework(frameworks: &[crate::Framework]) -> Option<&str> {
    ["flutter", "react-native", "compose", "swiftui"].into_iter().find(|id| frameworks.iter().any(|f| f.id == *id))
}

/// The screen the layout was read from.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Screen {
    /// The size of the screen in the units of the bounds.
    pub width: i32,
    pub height: i32,
    /// Units per density-independent pixel: 2.625 on a 420 dpi phone, 1 on iOS (points already).
    pub density: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutSnapshot {
    pub platform: Platform,
    pub root: UiNode,
    pub screen: Screen,
    /// A screenshot taken at the same moment, as PNG. Not part of the JSON; ask for it separately.
    #[serde(skip)]
    pub screenshot: Vec<u8>,
}

impl LayoutSnapshot {
    pub fn new(platform: Platform, mut root: UiNode, screen: Screen, screenshot: Vec<u8>) -> Self {
        let mut next = 0;
        number(&mut root, &mut next);
        LayoutSnapshot { platform, root, screen, screenshot }
    }

    /// The smallest touch target the platform recommends, in the units of the bounds.
    pub fn min_target(&self) -> f32 {
        let dp = match self.platform {
            Platform::Android => 48.0,
            Platform::Ios => 44.0,
        };
        dp * self.screen.density
    }
}

fn number(node: &mut UiNode, next: &mut u32) {
    node.id = *next;
    *next += 1;
    for child in &mut node.children {
        number(child, next);
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;

    pub fn node(class: &str, bounds: (i32, i32, i32, i32)) -> UiNode {
        UiNode {
            class: class.into(),
            bounds: Bounds::new(bounds.0, bounds.1, bounds.2, bounds.3),
            enabled: true,
            ..Default::default()
        }
    }

    pub fn snapshot(root: UiNode) -> LayoutSnapshot {
        LayoutSnapshot::new(Platform::Android, root, Screen { width: 1080, height: 2400, density: 2.625 }, Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;

    fn tree() -> LayoutSnapshot {
        let mut root = node("FrameLayout", (0, 0, 1080, 2400));
        let mut row = node("LinearLayout", (0, 100, 1080, 200));
        row.clickable = true;
        let mut title = node("TextView", (20, 120, 400, 60));
        title.text = Some("Wi-Fi".into());
        row.children.push(title);
        root.children.push(row);
        root.children.push(node("ImageView", (900, 100, 100, 100)));
        snapshot(root)
    }

    #[test]
    fn views_under_a_framework_host_are_marked() {
        let mut root = node("FrameLayout", (0, 0, 1080, 2400));
        let mut host = node("androidx.compose.ui.platform.ComposeView", (0, 0, 1080, 2000));
        let mut inner = node("View", (0, 0, 500, 500));
        inner.children.push(node("TextView", (0, 0, 100, 50)));
        host.children.push(inner);
        root.children.push(host);
        root.children.push(node("TextView", (0, 2000, 1080, 100)));
        let mut flutter = node("io.flutter.embedding.android.FlutterView", (0, 2100, 1080, 300));
        flutter.children.push(node("View", (0, 2100, 100, 100)));
        root.children.push(flutter);
        root.mark_hosts();

        let marks: Vec<(String, Option<String>)> = {
            let mut v = Vec::new();
            root.walk(&mut |n| v.push((n.short_class().to_string(), n.framework.clone())));
            v
        };
        let compose = Some("compose".to_string());
        assert_eq!(marks[0], ("FrameLayout".into(), None));
        assert_eq!(marks[1], ("ComposeView".into(), compose.clone()));
        assert_eq!(marks[2], ("View".into(), compose.clone()));
        assert_eq!(marks[3], ("TextView".into(), compose));
        assert_eq!(marks[4], ("TextView".into(), None), "a view beside the host is the platform's");
        assert_eq!(marks[5].1.as_deref(), Some("flutter"));
        assert_eq!(marks[6].1.as_deref(), Some("flutter"));
    }

    #[test]
    fn a_whole_screen_takes_the_framework_that_draws_its_own_ui() {
        use crate::Framework;
        let f = |ids: &[&str]| ids.iter().map(|i| Framework::new(i, "")).collect::<Vec<_>>();
        assert_eq!(screen_framework(&f(&["swiftui"])), Some("swiftui"));
        assert_eq!(screen_framework(&f(&["swiftui", "flutter"])), Some("flutter"));
        assert_eq!(screen_framework(&f(&["hermes", "react-native"])), Some("react-native"));
        assert_eq!(screen_framework(&f(&["hermes"])), None);
        assert_eq!(screen_framework(&[]), None);
    }

    #[test]
    fn the_framework_is_not_written_when_there_is_none() {
        let json = serde_json::to_string(&node("View", (0, 0, 1, 1))).unwrap();
        assert!(!json.contains("framework"), "{json}");
        let mut marked = node("View", (0, 0, 1, 1));
        marked.mark_all("swiftui");
        assert!(serde_json::to_string(&marked).unwrap().contains("\"framework\":\"swiftui\""));
    }

    #[test]
    fn nodes_are_numbered_parents_first() {
        let s = tree();
        let mut ids = Vec::new();
        s.root.walk(&mut |n| ids.push((n.id, n.short_class().to_string())));
        assert_eq!(
            ids,
            [(0, "FrameLayout".into()), (1, "LinearLayout".into()), (2, "TextView".into()), (3, "ImageView".into())]
        );
        assert_eq!(s.root.count(), 4);
        assert_eq!(s.root.find(2).unwrap().text.as_deref(), Some("Wi-Fi"));
    }

    #[test]
    fn a_row_is_named_by_the_text_inside_it() {
        let s = tree();
        assert_eq!(s.root.children[0].name(), Some("Wi-Fi"));
        assert_eq!(s.root.children[0].own_name(), None);
        assert_eq!(s.root.children[1].name(), None);
    }

    #[test]
    fn a_point_finds_the_deepest_node_on_top() {
        let s = tree();
        assert_eq!(s.root.at(50, 130).unwrap().id, 2);
        assert_eq!(s.root.at(500, 250).unwrap().id, 1);
        assert_eq!(s.root.at(950, 150).unwrap().id, 3);
        assert_eq!(s.root.at(5000, 5000), None);
    }

    #[test]
    fn the_target_size_follows_the_density() {
        assert!((tree().min_target() - 126.0).abs() < 0.01);
    }
}
