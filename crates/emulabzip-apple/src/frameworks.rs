//! Which UI frameworks an iOS Simulator app is built with, from the app bundle on this Mac.
//!
//! A simulator keeps installed apps as folders on disk, so the bundle can be read directly: Flutter ships
//! `Flutter.framework`, React Native ships a JavaScript bundle and often `hermes.framework`, and SwiftUI shows
//! in the libraries the executable is linked against.

use std::path::Path;

use emulabzip_core::frameworks::{Framework, add};
use emulabzip_core::{Error, Result};

use crate::simctl;

/// What the names in the bundle and the linked libraries say.
///
/// - `frameworks`: the names in the bundle's `Frameworks` folder.
/// - `root`: the names at the top of the bundle.
/// - `linked`: the output of `otool -L` for the executable and its `.debug.dylib`.
pub fn detect(frameworks: &[String], root: &[String], linked: &str) -> Vec<Framework> {
    let mut found = Vec::new();
    let has = |names: &[String], wanted: &str| names.iter().any(|n| n.eq_ignore_ascii_case(wanted));

    if has(frameworks, "Flutter.framework") {
        add(&mut found, Framework::new("flutter", "Frameworks/Flutter.framework"));
    }
    if has(root, "main.jsbundle") {
        add(&mut found, Framework::new("react-native", "main.jsbundle"));
    } else if frameworks.iter().any(|n| n == "React.framework" || n == "ReactNativeDependencies.framework") {
        add(&mut found, Framework::new("react-native", "Frameworks/React.framework"));
    }
    if has(frameworks, "hermes.framework") {
        add(&mut found, Framework::new("hermes", "Frameworks/hermes.framework"));
    }
    if linked.contains("/SwiftUI.framework/") {
        add(&mut found, Framework::new("swiftui", "linked with SwiftUI.framework"));
    }
    found
}

fn names(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|entries| entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default()
}

/// The libraries a Mach-O file is linked with, as `otool -L` lists them. Empty when it cannot be read.
async fn linked(file: &Path) -> String {
    match tokio::process::Command::new("otool").arg("-L").arg(file).output().await {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).into_owned(),
        _ => String::new(),
    }
}

/// Detects the frameworks of `bundle_id` on simulator `udid`.
pub async fn of_app(udid: &str, bundle_id: &str) -> Result<Vec<Framework>> {
    let container = simctl::run_text(["get_app_container", udid, bundle_id, "app"])
        .await
        .map_err(|_| Error::invalid("app (it is not installed on this simulator)", bundle_id))?;
    let app = Path::new(container.trim());
    let frameworks = names(&app.join("Frameworks"));
    let root = names(app);

    // The executable, and the library Xcode puts the code in for previews and debug builds.
    let mut libs = String::new();
    let mut to_read: Vec<String> = root.iter().filter(|n| n.ends_with(".debug.dylib")).cloned().collect();
    if let Ok(plist) = std::fs::read(app.join("Info.plist"))
        && let Ok(info) = simctl::plist_to_json(&plist).await
        && let Some(exe) = info.get("CFBundleExecutable").and_then(|v| v.as_str())
    {
        to_read.push(exe.to_string());
    }
    for name in to_read {
        libs.push_str(&linked(&app.join(name)).await);
    }
    Ok(detect(&frameworks, &root, &libs))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn ids(found: Vec<Framework>) -> Vec<String> {
        found.into_iter().map(|f| f.id).collect()
    }

    #[test]
    fn finds_flutter() {
        let found = detect(&v(&["App.framework", "Flutter.framework"]), &v(&["Info.plist"]), "");
        assert_eq!(ids(found), ["flutter"]);
    }

    #[test]
    fn finds_react_native_by_its_bundle_or_its_framework() {
        assert_eq!(ids(detect(&[], &v(&["main.jsbundle"]), "")), ["react-native"]);
        assert_eq!(ids(detect(&v(&["React.framework", "hermes.framework"]), &[], "")), ["react-native", "hermes"]);
    }

    #[test]
    fn finds_swiftui_in_the_linked_libraries() {
        let linked = "\t/System/Library/Frameworks/SwiftUI.framework/SwiftUI (compatibility version 1.0.0)\n\
                      \t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0)";
        assert_eq!(ids(detect(&[], &[], linked)), ["swiftui"]);
        assert!(detect(&[], &[], "\t/usr/lib/libSystem.B.dylib").is_empty());
    }

    #[test]
    fn a_flutter_app_with_swiftui_lists_both_in_a_steady_order() {
        let found = detect(&v(&["Flutter.framework"]), &[], "/SwiftUI.framework/SwiftUI");
        assert_eq!(ids(found), ["flutter", "swiftui"]);
    }
}
