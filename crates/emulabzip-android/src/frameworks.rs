//! Which UI frameworks an Android app is built with, from the files in its APK.
//!
//! Each framework leaves a mark: Flutter ships `libflutter.so`, React Native ships its JNI library and a
//! JavaScript bundle (and Hermes, when it uses it), and Jetpack Compose libraries write
//! `META-INF/androidx.compose.*.version` files into the APK.

use emulabzip_adb::AdbClient;
use emulabzip_core::frameworks::{Framework, add};
use emulabzip_core::{Error, Result};

/// The files of every APK of `app` (a base APK and its splits), after a line `APK <path>` for each.
pub fn listing_command(app: &str) -> String {
    format!("for f in $(pm path {app} | cut -d: -f2); do echo \"APK $f\"; unzip -l \"$f\" 2>/dev/null; done")
}

/// Reads one small file out of an APK.
pub fn read_command(apk: &str, entry: &str) -> String {
    format!("unzip -p \"{apk}\" {entry} 2>/dev/null")
}

/// What the file names in `listing` say. The listing is the output of [`listing_command`].
pub fn detect(listing: &str) -> Vec<Framework> {
    let mut found = Vec::new();
    // The last column of `unzip -l` is the name of the file.
    let names: Vec<&str> = listing.lines().filter_map(|l| l.split_whitespace().last()).collect();
    let find = |wanted: &dyn Fn(&str) -> bool| names.iter().copied().find(|n| wanted(n));

    if let Some(name) = find(&|n| n.ends_with("/libflutter.so")).or_else(|| find(&|n| n.contains("flutter_assets/"))) {
        add(&mut found, Framework::new("flutter", name));
    }
    let rn = find(&|n| {
        n.ends_with("/libreactnativejni.so")
            || n.ends_with("/libreactnative.so")
            || n.ends_with("/libhermes_executor.so")
            || n.ends_with("index.android.bundle")
    });
    if let Some(name) = rn {
        add(&mut found, Framework::new("react-native", name));
    }
    if let Some(name) = find(&|n| n.ends_with("/libhermes.so") || n.ends_with("/libhermesvm.so")) {
        add(&mut found, Framework::new("hermes", name));
    }
    if let Some(name) = find(&|n| n.starts_with("META-INF/androidx.compose.")) {
        add(&mut found, Framework::new("compose", name));
    }
    found
}

/// The first APK path in a listing: where to read version files from.
pub fn first_apk(listing: &str) -> Option<&str> {
    listing.lines().find_map(|l| l.strip_prefix("APK ")).map(str::trim)
}

/// Detects the frameworks of `app` on `serial`.
pub async fn of_app(adb: &AdbClient, serial: &str, app: &str) -> Result<Vec<Framework>> {
    let listing = adb.shell(serial, &listing_command(app)).await?;
    let Some(apk) = first_apk(&listing) else {
        return Err(Error::invalid("app (it is not installed)", app));
    };
    let mut found = detect(&listing);
    if let Some(compose) = found.iter_mut().find(|f| f.id == "compose") {
        // The version file holds the version of that library, such as `1.7.0`.
        let entry = compose.evidence.clone();
        let text = adb.shell(serial, &read_command(apk, &entry)).await.unwrap_or_default();
        compose.version = Some(text.trim().to_string()).filter(|v| !v.is_empty() && v.len() < 24);
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{}/tests/fixtures/frameworks/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn ids(listing: &str) -> Vec<String> {
        detect(listing).into_iter().map(|f| f.id).collect()
    }

    #[test]
    fn finds_flutter() {
        let listing = fixture("flutter-apk.txt");
        assert_eq!(ids(&listing), ["flutter"]);
        assert_eq!(detect(&listing)[0].evidence, "lib/arm64-v8a/libflutter.so");
    }

    #[test]
    fn finds_react_native_and_hermes() {
        assert_eq!(ids(&fixture("react-native-apk.txt")), ["react-native", "hermes"]);
    }

    #[test]
    fn finds_compose_from_its_version_files() {
        let listing = fixture("compose-apk.txt");
        assert_eq!(ids(&listing), ["compose"]);
        assert_eq!(detect(&listing)[0].evidence, "META-INF/androidx.compose.runtime_runtime.version");
    }

    #[test]
    fn a_plain_app_has_none() {
        assert!(detect(&fixture("plain-apk.txt")).is_empty());
        assert!(detect("").is_empty());
    }

    #[test]
    fn finds_where_the_first_apk_is() {
        assert_eq!(first_apk(&fixture("plain-apk.txt")), Some("/data/app/~~e==/com.example.plain-f==/base.apk"));
        assert_eq!(first_apk("nothing"), None);
    }

    #[test]
    fn the_commands_use_only_what_was_given() {
        assert!(listing_command("com.example.app").contains("pm path com.example.app"));
        assert_eq!(
            read_command("/a/b.apk", "META-INF/x.version"),
            "unzip -p \"/a/b.apk\" META-INF/x.version 2>/dev/null"
        );
    }
}
