//! Looking inside an APK, an AAB or an IPA: where the bytes are, and what changed between two builds.
//!
//! All three are zip files, so this reads them on this computer with no tool from the SDK or Xcode. Sizes are the
//! compressed sizes as stored in the file (what a download costs) and the sizes once unpacked (what the device keeps).

use std::collections::BTreeMap;
use std::io::{Read, Seek};
use std::path::Path;

use emulabzip_core::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    Apk,
    Aab,
    Ipa,
}

impl Format {
    pub fn label(self) -> &'static str {
        match self {
            Format::Apk => "APK",
            Format::Aab => "AAB",
            Format::Ipa => "IPA",
        }
    }
}

/// What a file in the package is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// Compiled Java and Kotlin: `classes*.dex`.
    Dex,
    /// Native code: `lib/<abi>/*.so`, and on iOS the executable and `Frameworks/`.
    Native,
    /// `res/`, `resources.arsc`, and on iOS asset catalogs, nibs and localizations.
    Resources,
    /// `assets/` and the other files the app ships.
    Assets,
    /// `AndroidManifest.xml`, `Info.plist`.
    Manifest,
    /// Signatures and bundle metadata: `META-INF/`, `_CodeSignature/`.
    Signing,
    Other,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Dex => "dex",
            Kind::Native => "native",
            Kind::Resources => "resources",
            Kind::Assets => "assets",
            Kind::Manifest => "manifest",
            Kind::Signing => "signing",
            Kind::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Size {
    pub compressed: u64,
    pub uncompressed: u64,
    pub files: u64,
}

impl Size {
    fn add(&mut self, compressed: u64, uncompressed: u64) {
        self.compressed += compressed;
        self.uncompressed += uncompressed;
        self.files += 1;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub path: String,
    pub kind: Kind,
    pub compressed: u64,
    pub uncompressed: u64,
}

/// A `classes*.dex` file, from its header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dex {
    pub path: String,
    pub bytes: u64,
    pub classes: u32,
    pub methods: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub format: Format,
    /// The file's own size on disk.
    pub file_bytes: u64,
    pub total: Size,
    pub kinds: BTreeMap<Kind, Size>,
    /// By top-level folder; files at the top are under `(root)`.
    pub folders: BTreeMap<String, Size>,
    /// Native code per ABI (Android) or `Frameworks` (iOS).
    pub native: BTreeMap<String, Size>,
    /// AAB only: size per module.
    pub modules: BTreeMap<String, Size>,
    pub dex: Vec<Dex>,
    /// Every file, for the diff and for finding the biggest.
    pub entries: Vec<Entry>,
}

/// `.aab` and `.ipa` are told from their contents, not their names, and a bare zip with an Android manifest is an APK.
fn detect(names: &[String], hint: Option<&str>) -> Option<Format> {
    if names.iter().any(|n| n.starts_with("Payload/") && n.contains(".app/")) {
        return Some(Format::Ipa);
    }
    if names.iter().any(|n| n == "base/manifest/AndroidManifest.xml" || n == "BundleConfig.pb") {
        return Some(Format::Aab);
    }
    if names.iter().any(|n| n == "AndroidManifest.xml") {
        return Some(Format::Apk);
    }
    match hint.map(str::to_ascii_lowercase).as_deref() {
        Some("apk") => Some(Format::Apk),
        Some("aab") => Some(Format::Aab),
        Some("ipa") => Some(Format::Ipa),
        _ => None,
    }
}

/// The path inside the app: the module is dropped from an AAB's, and `Payload/<name>.app/` from an IPA's.
fn inner(format: Format, path: &str) -> (Option<String>, String) {
    match format {
        Format::Apk => (None, path.to_string()),
        Format::Aab => match path.split_once('/') {
            Some((module, rest)) if !matches!(module, "META-INF" | "BUNDLE-METADATA") => {
                (Some(module.to_string()), rest.to_string())
            }
            _ => (None, path.to_string()),
        },
        Format::Ipa => {
            let rest = path.strip_prefix("Payload/").unwrap_or(path);
            match rest.split_once(".app/") {
                Some((_, inside)) => (None, inside.to_string()),
                None => (None, rest.to_string()),
            }
        }
    }
}

fn kind_of(format: Format, inner: &str, app_name: &str) -> Kind {
    let top = inner.split('/').next().unwrap_or("");
    match format {
        Format::Ipa => {
            if inner == "Info.plist" {
                Kind::Manifest
            } else if top == "_CodeSignature" || inner.ends_with("embedded.mobileprovision") {
                Kind::Signing
            } else if top == "Frameworks" || top == "PlugIns" || inner == app_name {
                Kind::Native
            } else if inner.ends_with(".car")
                || inner.ends_with(".nib")
                || inner.contains(".lproj/")
                || inner.contains(".storyboardc/")
                || inner.ends_with(".storyboardc")
                || inner.ends_with(".strings")
            {
                Kind::Resources
            } else {
                Kind::Assets
            }
        }
        _ => {
            if top == "META-INF" || top == "BUNDLE-METADATA" {
                Kind::Signing
            } else if inner.ends_with(".dex") && !inner.contains('/') || inner.starts_with("dex/") {
                Kind::Dex
            } else if top == "lib" || top == "jni" {
                Kind::Native
            } else if top == "res" || inner == "resources.arsc" || inner == "resources.pb" {
                Kind::Resources
            } else if top == "assets" {
                Kind::Assets
            } else if inner == "AndroidManifest.xml" || inner.starts_with("manifest/") {
                Kind::Manifest
            } else {
                Kind::Other
            }
        }
    }
}

/// Counts from a dex file's header: classes and methods, to see how close the app is to the 64K method limit.
pub fn dex_header(head: &[u8]) -> Option<(u32, u32)> {
    if head.len() < 0x70 || &head[..4] != b"dex\n" {
        return None;
    }
    let u32_at = |o: usize| u32::from_le_bytes([head[o], head[o + 1], head[o + 2], head[o + 3]]);
    Some((u32_at(0x60), u32_at(0x58)))
}

pub fn analyze_file(path: &Path) -> Result<Package> {
    let file =
        std::fs::File::open(path).map_err(|e| Error::failed(format!("Could not open {}: {e}", path.display())))?;
    let bytes = file.metadata().map(|m| m.len()).unwrap_or(0);
    analyze(file, bytes, path.extension().and_then(|e| e.to_str()))
}

pub fn analyze<R: Read + Seek>(reader: R, file_bytes: u64, hint: Option<&str>) -> Result<Package> {
    let mut zip = zip::ZipArchive::new(reader)
        .map_err(|e| Error::parse("an APK, AAB or IPA", format!("it is not a zip file ({e})")))?;
    let mut raw: Vec<(String, u64, u64)> = Vec::new();
    for i in 0..zip.len() {
        let Ok(f) = zip.by_index_raw(i) else { continue };
        if f.is_dir() {
            continue;
        }
        let name =
            f.name().map(|n| n.into_owned()).unwrap_or_else(|_| String::from_utf8_lossy(f.name_raw()).into_owned());
        raw.push((name, f.compressed_size(), f.size()));
    }
    let names: Vec<String> = raw.iter().map(|(n, _, _)| n.clone()).collect();
    let format = detect(&names, hint).ok_or_else(|| {
        Error::parse("an APK, AAB or IPA", "it has no AndroidManifest.xml, base module or Payload folder")
    })?;

    // An IPA's main executable has the app's own name.
    let app_name = names
        .iter()
        .find_map(|n| n.strip_prefix("Payload/").and_then(|r| r.split_once(".app/")).map(|(a, _)| a.to_string()))
        .unwrap_or_default();

    let mut pkg = Package {
        format,
        file_bytes,
        total: Size::default(),
        kinds: BTreeMap::new(),
        folders: BTreeMap::new(),
        native: BTreeMap::new(),
        modules: BTreeMap::new(),
        dex: Vec::new(),
        entries: Vec::new(),
    };
    for (path, compressed, size) in raw {
        let (module, inside) = inner(format, &path);
        let kind = kind_of(format, &inside, &app_name);
        pkg.total.add(compressed, size);
        pkg.kinds.entry(kind).or_default().add(compressed, size);
        let folder = inside.split_once('/').map_or("(root)", |(f, _)| f).to_string();
        pkg.folders.entry(folder).or_default().add(compressed, size);
        if let Some(m) = &module {
            pkg.modules.entry(m.clone()).or_default().add(compressed, size);
        }
        if kind == Kind::Native {
            let key = match format {
                Format::Ipa => inside
                    .split('/')
                    .next()
                    .filter(|f| *f == "Frameworks" || *f == "PlugIns")
                    .unwrap_or("executable")
                    .to_string(),
                _ => inside.split('/').nth(1).unwrap_or("(other)").to_string(),
            };
            pkg.native.entry(key).or_default().add(compressed, size);
        }
        if kind == Kind::Dex && inside.ends_with(".dex") {
            let mut head = [0u8; 0x70];
            let ok = zip.by_name(&path).ok().is_some_and(|mut f| f.read_exact(&mut head).is_ok());
            if let Some((classes, methods)) = ok.then(|| dex_header(&head)).flatten() {
                pkg.dex.push(Dex { path: path.clone(), bytes: size, classes, methods });
            }
        }
        pkg.entries.push(Entry { path, kind, compressed, uncompressed: size });
    }
    pkg.dex.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(pkg)
}

impl Package {
    /// The biggest files by compressed size.
    pub fn largest(&self, n: usize) -> Vec<&Entry> {
        let mut all: Vec<&Entry> = self.entries.iter().collect();
        all.sort_by(|a, b| b.compressed.cmp(&a.compressed).then_with(|| a.path.cmp(&b.path)));
        all.truncate(n);
        all
    }

    pub fn methods(&self) -> u32 {
        self.dex.iter().map(|d| d.methods).sum()
    }
}

/// A file that is in only one of two builds, or in both with another size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    pub kind: Kind,
    pub before: Option<u64>,
    pub after: Option<u64>,
    /// After minus before, in compressed bytes.
    pub delta: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diff {
    pub before: u64,
    pub after: u64,
    pub delta: i64,
    pub percent: f64,
    pub kinds: Vec<(Kind, i64)>,
    pub folders: Vec<(String, i64)>,
    pub added: usize,
    pub removed: usize,
    /// The biggest changes first.
    pub files: Vec<FileChange>,
    pub methods_before: u32,
    pub methods_after: u32,
}

/// What changed from `a` to `b`, in compressed bytes.
pub fn diff(a: &Package, b: &Package) -> Diff {
    let before: BTreeMap<&str, &Entry> = a.entries.iter().map(|e| (e.path.as_str(), e)).collect();
    let after: BTreeMap<&str, &Entry> = b.entries.iter().map(|e| (e.path.as_str(), e)).collect();
    let mut files = Vec::new();
    let (mut added, mut removed) = (0, 0);
    for (path, e) in &before {
        match after.get(path) {
            None => {
                removed += 1;
                files.push(FileChange {
                    path: path.to_string(),
                    kind: e.kind,
                    before: Some(e.compressed),
                    after: None,
                    delta: -(e.compressed as i64),
                });
            }
            Some(n) if n.compressed != e.compressed => files.push(FileChange {
                path: path.to_string(),
                kind: n.kind,
                before: Some(e.compressed),
                after: Some(n.compressed),
                delta: n.compressed as i64 - e.compressed as i64,
            }),
            _ => {}
        }
    }
    for (path, n) in &after {
        if !before.contains_key(path) {
            added += 1;
            files.push(FileChange {
                path: path.to_string(),
                kind: n.kind,
                before: None,
                after: Some(n.compressed),
                delta: n.compressed as i64,
            });
        }
    }
    files.sort_by(|x, y| y.delta.abs().cmp(&x.delta.abs()).then_with(|| x.path.cmp(&y.path)));

    let delta_of = |x: &BTreeMap<Kind, Size>, y: &BTreeMap<Kind, Size>| -> Vec<(Kind, i64)> {
        let mut keys: Vec<Kind> = x.keys().chain(y.keys()).copied().collect();
        keys.sort();
        keys.dedup();
        keys.into_iter()
            .map(|k| (k, y.get(&k).map_or(0, |s| s.compressed) as i64 - x.get(&k).map_or(0, |s| s.compressed) as i64))
            .filter(|(_, d)| *d != 0)
            .collect()
    };
    let mut folder_names: Vec<&String> = a.folders.keys().chain(b.folders.keys()).collect();
    folder_names.sort();
    folder_names.dedup();
    let mut folders: Vec<(String, i64)> = folder_names
        .into_iter()
        .map(|f| {
            (
                f.clone(),
                b.folders.get(f).map_or(0, |s| s.compressed) as i64
                    - a.folders.get(f).map_or(0, |s| s.compressed) as i64,
            )
        })
        .filter(|(_, d)| *d != 0)
        .collect();
    folders.sort_by_key(|(_, d)| std::cmp::Reverse(d.abs()));

    let delta = b.total.compressed as i64 - a.total.compressed as i64;
    Diff {
        before: a.total.compressed,
        after: b.total.compressed,
        delta,
        percent: if a.total.compressed == 0 { 0.0 } else { delta as f64 * 100.0 / a.total.compressed as f64 },
        kinds: delta_of(&a.kinds, &b.kinds),
        folders,
        added,
        removed,
        files,
        methods_before: a.methods(),
        methods_after: b.methods(),
    }
}

const MB: f64 = 1_048_576.0;

/// The limits a build is held to, in megabytes of compressed size. Each one is optional.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Limits {
    pub total_mb: Option<f64>,
    pub dex_mb: Option<f64>,
    pub native_mb: Option<f64>,
    pub resources_mb: Option<f64>,
    pub methods: Option<u32>,
    pub growth_percent: Option<f64>,
}

/// A limit a build went over.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Breach {
    pub what: String,
    pub limit: String,
    pub actual: String,
}

fn mb(bytes: u64) -> f64 {
    bytes as f64 / MB
}

/// What `package` is over. `baseline` is the build to measure growth against; without it a growth limit cannot be checked,
/// and that is reported rather than passed.
pub fn check(package: &Package, limits: &Limits, baseline: Option<&Package>) -> Vec<Breach> {
    let mut out = Vec::new();
    let kind = |k: Kind| package.kinds.get(&k).map_or(0, |s| s.compressed);
    for (what, limit, actual) in [
        ("total size", limits.total_mb, mb(package.total.compressed)),
        ("dex size", limits.dex_mb, mb(kind(Kind::Dex))),
        ("native code size", limits.native_mb, mb(kind(Kind::Native))),
        ("resources size", limits.resources_mb, mb(kind(Kind::Resources))),
    ] {
        if let Some(limit) = limit
            && actual > limit
        {
            out.push(Breach { what: what.into(), limit: format!("{limit:.2} MB"), actual: format!("{actual:.2} MB") });
        }
    }
    if let Some(limit) = limits.methods
        && package.methods() > limit
    {
        out.push(Breach { what: "methods".into(), limit: limit.to_string(), actual: package.methods().to_string() });
    }
    if let Some(limit) = limits.growth_percent {
        match baseline {
            Some(base) => {
                let d = diff(base, package);
                if d.percent > limit {
                    out.push(Breach {
                        what: "growth since the baseline".into(),
                        limit: format!("{limit:.1}%"),
                        actual: format!("{:.1}%", d.percent),
                    });
                }
            }
            None => out.push(Breach {
                what: "growth since the baseline".into(),
                limit: format!("{limit:.1}%"),
                actual: "no baseline build was given".into(),
            }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use super::*;

    fn dex(classes: u32, methods: u32, pad: usize) -> Vec<u8> {
        let mut d = vec![0u8; 0x70 + pad];
        d[..8].copy_from_slice(b"dex\n035\0");
        d[0x58..0x5c].copy_from_slice(&methods.to_le_bytes());
        d[0x60..0x64].copy_from_slice(&classes.to_le_bytes());
        d
    }

    fn zip_of(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        let mut w = zip::ZipWriter::new(&mut out);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (name, bytes) in files {
            w.start_file(*name, opts).unwrap();
            w.write_all(bytes).unwrap();
        }
        w.finish().unwrap();
        out.into_inner()
    }

    fn open(bytes: Vec<u8>, hint: &str) -> Package {
        let n = bytes.len() as u64;
        analyze(Cursor::new(bytes), n, Some(hint)).unwrap()
    }

    fn apk(res_pad: usize, extra: bool) -> Package {
        let mut files: Vec<(&str, Vec<u8>)> = vec![
            ("AndroidManifest.xml", vec![0; 100]),
            ("classes.dex", dex(120, 900, 2000)),
            ("classes2.dex", dex(10, 70, 100)),
            ("lib/arm64-v8a/libapp.so", vec![0; 5000]),
            ("lib/x86_64/libapp.so", vec![0; 6000]),
            ("res/layout/main.xml", vec![0; res_pad]),
            ("resources.arsc", vec![0; 300]),
            ("assets/data.bin", vec![0; 1000]),
            ("META-INF/CERT.RSA", vec![0; 50]),
        ];
        if extra {
            files.push(("assets/new.bin", vec![0; 4000]));
        }
        open(zip_of(&files), "apk")
    }

    #[test]
    fn an_apk_is_broken_down_by_kind_folder_and_abi() {
        let p = apk(400, false);
        assert_eq!(p.format, Format::Apk);
        assert_eq!(p.total.files, 9);
        assert_eq!(p.kinds[&Kind::Dex].uncompressed, 0x70 + 2000 + 0x70 + 100);
        assert_eq!(p.kinds[&Kind::Native].uncompressed, 11_000);
        assert_eq!(p.kinds[&Kind::Resources].files, 2);
        assert_eq!(p.kinds[&Kind::Assets].uncompressed, 1000);
        assert_eq!(p.kinds[&Kind::Signing].files, 1);
        assert_eq!(p.kinds[&Kind::Manifest].files, 1);
        assert_eq!(p.native["arm64-v8a"].uncompressed, 5000);
        assert_eq!(p.native["x86_64"].uncompressed, 6000);
        assert_eq!(p.folders["lib"].files, 2);
        assert_eq!(p.folders["(root)"].files, 4);
        assert_eq!(p.methods(), 970);
        assert_eq!(p.dex.iter().map(|d| d.classes).sum::<u32>(), 130);
        assert_eq!(p.largest(1)[0].path, "lib/x86_64/libapp.so");
    }

    #[test]
    fn an_aab_is_told_by_its_base_module_and_split_by_module() {
        let files: Vec<(&str, Vec<u8>)> = vec![
            ("base/manifest/AndroidManifest.xml", vec![0; 100]),
            ("base/dex/classes.dex", dex(5, 50, 500)),
            ("base/lib/arm64-v8a/libx.so", vec![0; 2000]),
            ("base/res/a.png", vec![0; 700]),
            ("feature_maps/assets/map.bin", vec![0; 9000]),
            ("BundleConfig.pb", vec![0; 20]),
            ("META-INF/MANIFEST.MF", vec![0; 30]),
        ];
        let p = open(zip_of(&files), "aab");
        assert_eq!(p.format, Format::Aab);
        assert_eq!(p.modules["base"].files, 4);
        assert_eq!(p.modules["feature_maps"].uncompressed, 9000);
        assert_eq!(p.kinds[&Kind::Dex].files, 1);
        assert_eq!(p.native["arm64-v8a"].uncompressed, 2000);
        assert_eq!(p.kinds[&Kind::Assets].uncompressed, 9000);
        assert_eq!(p.methods(), 50);
    }

    #[test]
    fn an_ipa_counts_frameworks_and_the_executable_as_native() {
        let files: Vec<(&str, Vec<u8>)> = vec![
            ("Payload/Shop.app/Info.plist", vec![0; 200]),
            ("Payload/Shop.app/Shop", vec![0; 8000]),
            ("Payload/Shop.app/Frameworks/Lib.framework/Lib", vec![0; 3000]),
            ("Payload/Shop.app/Assets.car", vec![0; 1500]),
            ("Payload/Shop.app/en.lproj/Localizable.strings", vec![0; 100]),
            ("Payload/Shop.app/_CodeSignature/CodeResources", vec![0; 60]),
            ("Payload/Shop.app/data.json", vec![0; 400]),
        ];
        let p = open(zip_of(&files), "ipa");
        assert_eq!(p.format, Format::Ipa);
        assert_eq!(p.kinds[&Kind::Native].uncompressed, 11_000);
        assert_eq!(p.native["Frameworks"].uncompressed, 3000);
        assert_eq!(p.native["executable"].uncompressed, 8000);
        assert_eq!(p.kinds[&Kind::Resources].files, 2);
        assert_eq!(p.kinds[&Kind::Manifest].files, 1);
        assert_eq!(p.kinds[&Kind::Signing].files, 1);
        assert_eq!(p.kinds[&Kind::Assets].uncompressed, 400);
    }

    #[test]
    fn a_zip_that_is_no_package_is_explained() {
        let bytes = zip_of(&[("readme.txt", b"hi".to_vec())]);
        let n = bytes.len() as u64;
        let e = analyze(Cursor::new(bytes), n, Some("zip")).unwrap_err().to_string();
        assert!(e.contains("AndroidManifest.xml"), "{e}");
        let e = analyze(Cursor::new(b"not a zip".to_vec()), 9, None).unwrap_err().to_string();
        assert!(e.contains("not a zip"), "{e}");
        assert!(dex_header(b"nope").is_none() && dex_header(&[0u8; 0x80]).is_none());
    }

    #[test]
    fn the_budget_names_what_is_over() {
        let p = apk(400, false);
        let total_mb = p.total.compressed as f64 / MB;
        assert!(check(&p, &Limits::default(), None).is_empty());
        assert!(
            check(&p, &Limits { total_mb: Some(total_mb + 1.0), methods: Some(1000), ..Limits::default() }, None)
                .is_empty()
        );
        let over = check(
            &p,
            &Limits { total_mb: Some(total_mb / 2.0), dex_mb: Some(0.000001), methods: Some(100), ..Limits::default() },
            None,
        );
        let what: Vec<_> = over.iter().map(|b| b.what.as_str()).collect();
        assert_eq!(what, ["total size", "dex size", "methods"]);
        assert_eq!(over[2].actual, "970");

        let big = apk(400, true);
        let growth = Limits { growth_percent: Some(5.0), ..Limits::default() };
        assert_eq!(check(&big, &growth, Some(&p)).len(), 1);
        assert!(check(&p, &growth, Some(&big)).is_empty(), "shrinking is not growth");
        assert!(check(&p, &growth, None)[0].actual.contains("no baseline"));
    }

    #[test]
    fn two_builds_are_compared_by_file() {
        let a = apk(400, false);
        let b = apk(1400, true);
        let d = diff(&a, &b);
        assert_eq!(d.delta, 1000 + 4000);
        assert_eq!((d.added, d.removed), (1, 0));
        assert_eq!(d.files[0].path, "assets/new.bin");
        assert_eq!(d.files[0].delta, 4000);
        assert_eq!(d.files[1].path, "res/layout/main.xml");
        assert!(d.kinds.contains(&(Kind::Assets, 4000)) && d.kinds.contains(&(Kind::Resources, 1000)), "{:?}", d.kinds);
        assert_eq!(d.folders[0], ("assets".to_string(), 4000));
        assert!((d.percent - 5000.0 * 100.0 / a.total.compressed as f64).abs() < 1e-9);
        assert_eq!(diff(&a, &a).delta, 0);
        assert!(diff(&a, &a).files.is_empty());
    }
}
