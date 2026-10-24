//! The views on an Android screen, read with `uiautomator dump`.
//!
//! `uiautomator` is part of every Android image and writes the accessibility tree of the window in
//! front as XML. That is exactly what TalkBack sees, so the same tree feeds the accessibility audit.

use emulabzip_adb::AdbClient;
use emulabzip_core::{Bounds, Error, LayoutSnapshot, Platform, Result, Screen, UiNode};

const DUMP_FILE: &str = "/data/local/tmp/emulabzip-ui.xml";

/// Reads the screen: the view tree and a screenshot taken at the same moment.
pub async fn snapshot(adb: &AdbClient, serial: &str) -> Result<LayoutSnapshot> {
    let (xml, shot, density) = tokio::join!(dump(adb, serial), adb.exec(serial, "screencap -p"), density(adb, serial));
    let shot = shot?;
    if !shot.starts_with(b"\x89PNG") {
        return Err(Error::Adb(format!("screencap failed: {}", String::from_utf8_lossy(&shot).trim())));
    }
    let root = parse(&xml?)?;
    // The screenshot is in the orientation the device is held in, as the bounds are.
    let (width, height) = png_size(&shot).unwrap_or_else(|| {
        let b = root.bounds;
        ((b.x + b.width) as u32, (b.y + b.height) as u32)
    });
    Ok(LayoutSnapshot::new(
        Platform::Android,
        root,
        Screen { width: width as i32, height: height as i32, density: density.unwrap_or(2.625) },
        shot,
    ))
}

/// The XML of the window in front. It is printed to the terminal where the device allows, and
/// written to a file where it does not.
async fn dump(adb: &AdbClient, serial: &str) -> Result<String> {
    let out = adb.exec(serial, "uiautomator dump /dev/tty").await?;
    if let Some(xml) = extract_xml(&String::from_utf8_lossy(&out)) {
        return Ok(xml);
    }
    let note =
        adb.shell(serial, &format!("uiautomator dump {DUMP_FILE} && cat {DUMP_FILE}; rm -f {DUMP_FILE}")).await?;
    extract_xml(&note).ok_or_else(|| {
        Error::Adb(format!(
            "uiautomator did not produce a layout. It cannot read screens that block it, such as a secure window. {}",
            note.lines().next().unwrap_or_default().trim()
        ))
    })
}

/// The document inside whatever else `uiautomator` prints around it.
pub fn extract_xml(text: &str) -> Option<String> {
    let start = text.find("<?xml").or_else(|| text.find("<hierarchy"))?;
    let end = text.rfind("</hierarchy>")? + "</hierarchy>".len();
    (end > start).then(|| text[start..end].to_string())
}

/// Pixels per dp, from `wm density` (an override wins over the physical value).
async fn density(adb: &AdbClient, serial: &str) -> Option<f32> {
    let text = adb.shell(serial, "wm density").await.ok()?;
    parse_density(&text)
}

pub fn parse_density(text: &str) -> Option<f32> {
    let find =
        |key: &str| text.lines().find_map(|l| l.trim().strip_prefix(key)).and_then(|v| v.trim().parse::<f32>().ok());
    find("Override density:").or_else(|| find("Physical density:")).filter(|d| *d > 0.0).map(|dpi| dpi / 160.0)
}

fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    let ihdr = png.get(16..24)?;
    Some((u32::from_be_bytes(ihdr[..4].try_into().ok()?), u32::from_be_bytes(ihdr[4..].try_into().ok()?)))
}

/// `[0,63][1080,210]`.
fn bounds(text: &str) -> Option<Bounds> {
    let numbers: Vec<i32> =
        text.split(['[', ']', ',']).filter(|s| !s.is_empty()).map(|s| s.parse().ok()).collect::<Option<_>>()?;
    let [l, t, r, b] = numbers[..] else { return None };
    Some(Bounds::new(l, t, r - l, b - t))
}

fn flag(node: roxmltree::Node, name: &str) -> bool {
    node.attribute(name) == Some("true")
}

fn text(node: roxmltree::Node, name: &str) -> Option<String> {
    node.attribute(name).filter(|s| !s.is_empty()).map(str::to_string)
}

fn convert(node: roxmltree::Node) -> UiNode {
    let class = node.attribute("class").unwrap_or_default().to_string();
    let editable = class.ends_with("EditText") || class.ends_with("AutoCompleteTextView");
    UiNode {
        id: 0,
        editable,
        class,
        text: text(node, "text"),
        label: text(node, "content-desc"),
        hint: None,
        resource_id: text(node, "resource-id"),
        package: text(node, "package"),
        bounds: node.attribute("bounds").and_then(bounds).unwrap_or_default(),
        clickable: flag(node, "clickable") || flag(node, "long-clickable"),
        focusable: flag(node, "focusable"),
        enabled: node.attribute("enabled") != Some("false"),
        checked: flag(node, "checkable").then(|| flag(node, "checked")),
        scrollable: flag(node, "scrollable"),
        children: node.children().filter(|c| c.has_tag_name("node")).map(convert).collect(),
    }
}

/// Parses the XML `uiautomator dump` writes into one tree. When the screen has several windows,
/// they become the children of a root that covers them all.
pub fn parse(xml: &str) -> Result<UiNode> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| Error::parse("uiautomator layout", e.to_string()))?;
    let hierarchy = doc.root_element();
    if !hierarchy.has_tag_name("hierarchy") {
        return Err(Error::parse("uiautomator layout", "there is no hierarchy element"));
    }
    let mut windows: Vec<UiNode> = hierarchy.children().filter(|c| c.has_tag_name("node")).map(convert).collect();
    match windows.len() {
        0 => Err(Error::parse("uiautomator layout", "the screen has no views")),
        1 => Ok(windows.remove(0)),
        _ => {
            let (right, bottom) = windows
                .iter()
                .fold((0, 0), |(r, b), w| (r.max(w.bounds.x + w.bounds.width), b.max(w.bounds.y + w.bounds.height)));
            Ok(UiNode {
                class: "hierarchy".into(),
                bounds: Bounds::new(0, 0, right, bottom),
                enabled: true,
                children: windows,
                ..Default::default()
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL: &str = r#"<?xml version='1.0' encoding='UTF-8' standalone='yes' ?>
<hierarchy rotation="0">
  <node index="0" text="" resource-id="" class="android.widget.FrameLayout" package="com.android.settings" content-desc="" checkable="false" checked="false" clickable="false" enabled="true" focusable="false" focused="false" scrollable="false" long-clickable="false" password="false" selected="false" bounds="[0,0][1080,2400]">
    <node index="0" text="Network &amp; internet" resource-id="android:id/title" class="android.widget.TextView" package="com.android.settings" content-desc="" checkable="false" checked="false" clickable="false" enabled="true" focusable="false" focused="false" scrollable="false" long-clickable="false" password="false" selected="false" bounds="[168,300][700,370]" />
    <node index="1" text="" resource-id="com.android.settings:id/search" class="android.widget.ImageButton" package="com.android.settings" content-desc="Search settings" checkable="false" checked="false" clickable="true" enabled="true" focusable="true" focused="false" scrollable="false" long-clickable="false" password="false" selected="false" bounds="[900,100][1000,200]" />
    <node index="2" text="" resource-id="" class="android.widget.Switch" package="com.android.settings" content-desc="" checkable="true" checked="true" clickable="true" enabled="true" focusable="true" focused="false" scrollable="false" long-clickable="false" password="false" selected="false" bounds="[900,400][1000,470]" />
  </node>
</hierarchy>"#;

    #[test]
    fn reads_the_tree_with_names_flags_and_bounds() {
        let root = parse(SMALL).unwrap();
        assert_eq!((root.class.as_str(), root.children.len()), ("android.widget.FrameLayout", 3));
        assert_eq!(root.bounds, Bounds::new(0, 0, 1080, 2400));
        let title = &root.children[0];
        assert_eq!(
            (title.text.as_deref(), title.resource_id.as_deref()),
            (Some("Network & internet"), Some("android:id/title"))
        );
        assert_eq!(title.bounds, Bounds::new(168, 300, 532, 70));
        let search = &root.children[1];
        assert_eq!(
            (search.label.as_deref(), search.clickable, search.focusable),
            (Some("Search settings"), true, true)
        );
        let toggle = &root.children[2];
        assert_eq!((toggle.checked, toggle.label.clone()), (Some(true), None));
        assert_eq!(root.children[0].checked, None);
    }

    #[test]
    fn several_windows_get_a_common_root() {
        let xml = r#"<hierarchy rotation="0"><node class="a" bounds="[0,0][100,200]" enabled="true"/><node class="b" bounds="[0,0][300,100]" enabled="true"/></hierarchy>"#;
        let root = parse(xml).unwrap();
        assert_eq!(
            (root.class.as_str(), root.children.len(), root.bounds.width, root.bounds.height),
            ("hierarchy", 2, 300, 200)
        );
    }

    #[test]
    fn an_empty_or_foreign_document_is_explained() {
        assert!(parse("<hierarchy rotation=\"0\"/>").unwrap_err().to_string().contains("no views"));
        assert!(parse("<html/>").unwrap_err().to_string().contains("hierarchy"));
        assert!(parse("not xml").is_err());
    }

    #[test]
    fn the_document_is_found_among_the_text_uiautomator_prints() {
        let noisy = format!("{SMALL}\nUI hierarchy dumped to: /dev/tty");
        assert_eq!(extract_xml(&noisy).unwrap(), SMALL);
        assert_eq!(extract_xml("ERROR: null root node returned by UiTestAutomationBridge."), None);
    }

    #[test]
    fn the_density_prefers_an_override() {
        assert_eq!(parse_density("Physical density: 420\n"), Some(2.625));
        assert_eq!(parse_density("Physical density: 420\nOverride density: 320\n"), Some(2.0));
        assert_eq!(parse_density("nope"), None);
    }

    #[test]
    fn bounds_need_four_numbers() {
        assert_eq!(bounds("[10,20][110,70]"), Some(Bounds::new(10, 20, 100, 50)));
        assert_eq!(bounds("[10,20]"), None);
        assert_eq!(bounds("garbage"), None);
    }

    #[test]
    fn edit_texts_can_be_typed_into() {
        let xml = r#"<hierarchy><node class="android.widget.EditText" text="Search" bounds="[0,0][500,100]" enabled="true" clickable="true" focusable="true"/></hierarchy>"#;
        assert!(parse(xml).unwrap().editable);
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::*;

    /// A real dump of the Settings app on an Android 14 emulator.
    const SETTINGS: &str = include_str!("../tests/fixtures/uiautomator-settings.xml");

    #[test]
    fn reads_a_real_dump_of_the_settings_app() {
        let root = parse(SETTINGS).unwrap();
        assert!(root.count() > 40, "{} nodes", root.count());
        let mut texts = Vec::new();
        root.walk(&mut |n| texts.extend(n.text.clone()));
        assert!(texts.iter().any(|t| t == "Network & internet"), "{texts:?}");
        let mut clickable = 0;
        root.walk(&mut |n| clickable += usize::from(n.clickable));
        assert!(clickable > 5);
        assert_eq!(root.package.as_deref(), Some("com.android.settings"));
    }

    #[test]
    fn the_real_dump_is_found_inside_what_uiautomator_prints() {
        let raw = format!("{SETTINGS}UI hierarchy dumped to: /dev/tty");
        assert_eq!(parse(&extract_xml(&raw).unwrap()).unwrap().count(), parse(SETTINGS).unwrap().count());
    }
}
