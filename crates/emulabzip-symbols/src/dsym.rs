//! Addresses in an iOS app, looked up in its `.dSYM` with `atos`.
//!
//! A crash report of a release app lists bare addresses: where each image was loaded (`base`) and the address of the
//! frame. The `.dSYM` the build wrote next to the app has the names and the lines. `atos` (part of Xcode's command
//! line tools) joins the two. It only exists on a Mac, so on any other system a lookup says so instead of failing in a
//! confusing way.
//!
//! A `.dSYM` belongs to one build of the app, which is tied to it by a UUID. The UUID is checked, so a crash of one
//! version is never read with the symbols of another.

use std::path::{Path, PathBuf};

/// What `atos` knew about an address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub function: String,
    pub file: Option<String>,
    pub line: Option<u32>,
}

/// Why a lookup could not be done.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// `atos` and `dwarfdump` only exist on a Mac.
    NeedsAMac,
    /// The folder is not a `.dSYM`, or holds no binary.
    NotADsym(PathBuf),
    /// A tool was not found, or failed. The text says what it printed.
    Tool(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NeedsAMac => write!(f, "dSYM files are read with `atos`, which only exists on a Mac"),
            Error::NotADsym(p) => write!(f, "{} is not a .dSYM with a binary inside", p.display()),
            Error::Tool(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for Error {}

/// A `.dSYM` folder.
#[derive(Debug, Clone)]
pub struct Dsym {
    path: PathBuf,
    binary: PathBuf,
}

/// The binary with the debug information, inside a `.dSYM`: `Contents/Resources/DWARF/<name>`.
pub fn binary_of(dsym: &Path) -> Option<PathBuf> {
    let dir = dsym.join("Contents").join("Resources").join("DWARF");
    let mut files: Vec<PathBuf> =
        std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
    files.sort();
    files.into_iter().next()
}

/// Reads the lines of `dwarfdump --uuid`: `UUID: 4C4C44B2-5555-3144-A1BE-F9C7C8B1B2A0 (x86_64) /path/App`.
pub fn parse_uuids(output: &str) -> Vec<(String, String)> {
    output
        .lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("UUID: ")?;
            let (uuid, rest) = rest.split_once(' ')?;
            let arch = rest.strip_prefix('(')?.split_once(')')?.0;
            Some((uuid.to_ascii_lowercase(), arch.to_string()))
        })
        .collect()
}

/// Reads one line `atos` prints:
///
/// ```text
/// explode(depth:) (in Crashy) (main.swift:38)
/// _dispatch_client_callout (in libdispatch.dylib) + 6
/// 0x000000010c2a1b30
/// ```
///
/// An address it could not name comes back as `None`.
pub fn parse_atos_line(line: &str) -> Option<Symbol> {
    let line = line.trim();
    if line.is_empty() || line.starts_with("0x") {
        return None;
    }
    // The name is everything before ` (in `, which a Swift or Objective-C name can itself contain parentheses of.
    let (function, rest) = line.split_once(" (in ")?;
    let after_image = rest.split_once(')').map(|(_, r)| r.trim()).unwrap_or("");
    let (file, line_no) = match after_image.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
        Some(place) => match place.rsplit_once(':') {
            Some((file, n)) => match n.parse() {
                Ok(n) => (Some(file.to_string()), Some(n)),
                Err(_) => (Some(place.to_string()), None),
            },
            None => (Some(place.to_string()), None),
        },
        None => (None, None),
    };
    Some(Symbol { function: function.trim().to_string(), file, line: line_no })
}

impl Dsym {
    /// Opens a `.dSYM` folder.
    pub fn open(path: &Path) -> Result<Dsym, Error> {
        let binary = binary_of(path).ok_or_else(|| Error::NotADsym(path.to_path_buf()))?;
        Ok(Dsym { path: path.to_path_buf(), binary })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The UUID of each architecture the `.dSYM` holds, lower-case, as `(uuid, arch)`.
    pub fn uuids(&self) -> Result<Vec<(String, String)>, Error> {
        run("dwarfdump", &["--uuid".into(), self.binary.display().to_string()]).map(|out| parse_uuids(&out))
    }

    /// Whether this `.dSYM` is the one of the build with this UUID (as a crash report prints it, with or without the
    /// dashes, in any case).
    pub fn is_for(&self, uuid: &str) -> Result<bool, Error> {
        let wanted: String = uuid.chars().filter(|c| *c != '-').collect::<String>().to_ascii_lowercase();
        Ok(self.uuids()?.iter().any(|(u, _)| u.replace('-', "") == wanted))
    }

    /// Looks `addresses` up for an image loaded at `base`. The answers come in the order of the addresses.
    pub fn symbolicate(&self, arch: &str, base: u64, addresses: &[u64]) -> Result<Vec<Option<Symbol>>, Error> {
        if addresses.is_empty() {
            return Ok(Vec::new());
        }
        let mut args: Vec<String> = vec![
            "-o".into(),
            self.binary.display().to_string(),
            "-arch".into(),
            arch.into(),
            "-l".into(),
            format!("{base:#x}"),
        ];
        args.extend(addresses.iter().map(|a| format!("{a:#x}")));
        let out = run("atos", &args)?;
        let mut lines = out.lines();
        Ok(addresses.iter().map(|_| lines.next().and_then(parse_atos_line)).collect())
    }
}

#[cfg(target_os = "macos")]
fn run(tool: &str, args: &[String]) -> Result<String, Error> {
    let out = std::process::Command::new(tool).args(args).output().map_err(|e| {
        Error::Tool(format!("could not run `{tool}`: {e}. Install Xcode's command line tools: xcode-select --install"))
    })?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(Error::Tool(format!("`{tool}` failed: {}", err.lines().next().unwrap_or("").trim())));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(not(target_os = "macos"))]
fn run(_tool: &str, _args: &[String]) -> Result<String, Error> {
    Err(Error::NeedsAMac)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_atos_prints() {
        let s = parse_atos_line("explode(depth:) (in Crashy) (main.swift:38)").unwrap();
        assert_eq!((s.function.as_str(), s.file.as_deref(), s.line), ("explode(depth:)", Some("main.swift"), Some(38)));
        let s =
            parse_atos_line("-[AppDelegate application:didFinishLaunchingWithOptions:] (in Foo) (AppDelegate.m:12)")
                .unwrap();
        assert_eq!(
            (s.function.as_str(), s.line),
            ("-[AppDelegate application:didFinishLaunchingWithOptions:]", Some(12))
        );
        let s = parse_atos_line("closure #1 in View.body.getter (in App) (ContentView.swift:44)").unwrap();
        assert_eq!(s.file.as_deref(), Some("ContentView.swift"));
        let s = parse_atos_line("_dispatch_client_callout (in libdispatch.dylib) + 6").unwrap();
        assert_eq!((s.function.as_str(), s.file, s.line), ("_dispatch_client_callout", None, None));
        let s = parse_atos_line("main (in Crashy) (<compiler-generated>)").unwrap();
        assert_eq!((s.file.as_deref(), s.line), (Some("<compiler-generated>"), None));
        for none in ["", "0x000000010c2a1b30", "   ", "no image here"] {
            assert_eq!(parse_atos_line(none), None, "{none:?}");
        }
    }

    #[test]
    fn reads_the_uuids_of_a_dsym() {
        let out = "UUID: 4C4C44B2-5555-3144-A1BE-F9C7C8B1B2A0 (x86_64) /x/Crashy.dSYM/Contents/Resources/DWARF/Crashy\nUUID: 0A0B0C0D-0000-1111-2222-333344445555 (arm64) /x/Crashy\nnot a uuid line\n";
        assert_eq!(
            parse_uuids(out),
            [
                ("4c4c44b2-5555-3144-a1be-f9c7c8b1b2a0".to_string(), "x86_64".to_string()),
                ("0a0b0c0d-0000-1111-2222-333344445555".to_string(), "arm64".to_string())
            ]
        );
        assert!(parse_uuids("").is_empty());
    }

    #[test]
    fn finds_the_binary_inside_a_dsym() {
        let dir = std::env::temp_dir().join(format!("emulabzip-dsym-{}", std::process::id()));
        let dwarf = dir.join("Crashy.dSYM/Contents/Resources/DWARF");
        std::fs::create_dir_all(&dwarf).unwrap();
        std::fs::write(dwarf.join("Crashy"), b"x").unwrap();
        assert_eq!(binary_of(&dir.join("Crashy.dSYM")), Some(dwarf.join("Crashy")));
        assert!(Dsym::open(&dir.join("Crashy.dSYM")).is_ok());
        assert!(matches!(Dsym::open(&dir), Err(Error::NotADsym(_))));
        assert!(matches!(Dsym::open(&dir.join("missing.dSYM")), Err(Error::NotADsym(_))));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn says_it_needs_a_mac_elsewhere() {
        let dir = std::env::temp_dir().join(format!("emulabzip-dsym2-{}", std::process::id()));
        let dwarf = dir.join("A.dSYM/Contents/Resources/DWARF");
        std::fs::create_dir_all(&dwarf).unwrap();
        std::fs::write(dwarf.join("A"), b"x").unwrap();
        let d = Dsym::open(&dir.join("A.dSYM")).unwrap();
        assert_eq!(d.symbolicate("arm64", 0x1000, &[0x1010]).unwrap_err(), Error::NeedsAMac);
        assert!(d.uuids().is_err());
        assert!(Error::NeedsAMac.to_string().contains("Mac"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
