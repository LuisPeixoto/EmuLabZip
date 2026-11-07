//! The project's `emulabzip.toml`: limits a run has to stay under, and how to measure the start time.

use std::path::Path;

use emulabzip_core::{Error, Result, StartKind};
use serde::Deserialize;

/// The file name looked for in the project folder.
pub const FILE_NAME: &str = "emulabzip.toml";

/// Limits. Each one is optional: a run is only held to the limits that are written down.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    /// The median start time, in milliseconds. Needs `[startup]`.
    pub start_time_ms: Option<u64>,
    pub memory_peak_mb: Option<f64>,
    /// The average share of the processor the app used.
    pub cpu_avg_percent: Option<f64>,
    /// The highest share of janky frames in any sample.
    pub janky_frames_percent: Option<f64>,
    pub requests: Option<u64>,
    pub failed_requests: Option<u64>,
    pub log_errors: Option<u64>,
    pub crashes: Option<u64>,
}

/// Measure how long the app takes to open before the command runs.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartupConfig {
    #[serde(default = "default_runs")]
    pub runs: u32,
    #[serde(default = "default_kind")]
    pub kind: String,
}

fn default_runs() -> u32 {
    5
}

fn default_kind() -> String {
    "cold".into()
}

impl StartupConfig {
    pub fn kind(&self) -> Result<StartKind> {
        StartKind::parse(&self.kind).ok_or_else(|| Error::invalid("start kind (cold, warm or hot)", &self.kind))
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Baseline {
    /// How much worse than the baseline a number can be before it counts as a regression.
    #[serde(default = "default_tolerance")]
    pub tolerance_percent: f64,
}

fn default_tolerance() -> f64 {
    10.0
}

impl Default for Baseline {
    fn default() -> Self {
        Baseline { tolerance_percent: default_tolerance() }
    }
}

/// Limits for the size of a build (`emulabzip size --budget`, `emulabzip ci --size`). Sizes are the compressed sizes
/// in the file, in megabytes (1 MB = 1,048,576 bytes).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SizeConfig {
    pub max_total_mb: Option<f64>,
    pub max_dex_mb: Option<f64>,
    pub max_native_mb: Option<f64>,
    pub max_resources_mb: Option<f64>,
    /// Methods in all the dex files together; 65,536 is the limit of one dex file.
    pub max_methods: Option<u32>,
    /// How much bigger than a baseline build the total may be.
    pub max_growth_percent: Option<f64>,
}

impl SizeConfig {
    fn validate(&self) -> Result<()> {
        for (name, v) in [
            ("[size] max_total_mb (more than 0)", self.max_total_mb),
            ("[size] max_dex_mb (more than 0)", self.max_dex_mb),
            ("[size] max_native_mb (more than 0)", self.max_native_mb),
            ("[size] max_resources_mb (more than 0)", self.max_resources_mb),
        ] {
            if let Some(v) = v
                && !(v.is_finite() && v > 0.0)
            {
                return Err(Error::invalid(name, v.to_string()));
            }
        }
        if let Some(g) = self.max_growth_percent
            && !(0.0..=1000.0).contains(&g)
        {
            return Err(Error::invalid("[size] max_growth_percent (0 to 1000)", g.to_string()));
        }
        Ok(())
    }
}

/// A part of the screen that visual comparisons leave out, in pixels.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IgnoreRegion {
    /// What it is, for whoever reads the file: `clock`, `ad banner`.
    pub name: Option<String>,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// How screenshots are compared (`emulabzip visual`, `emulabzip ci --visual`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualConfig {
    /// How much of the screen may change before it counts as a difference.
    #[serde(default = "default_visual_threshold")]
    pub threshold_percent: f64,
    /// How far a colour channel may differ (0 to 255) before a pixel counts as changed.
    #[serde(default = "default_visual_tolerance")]
    pub tolerance: u8,
    /// Where the baselines are, relative to the project folder. Default `visual`.
    pub baseline_dir: Option<String>,
    #[serde(default)]
    pub ignore: Vec<IgnoreRegion>,
}

fn default_visual_threshold() -> f64 {
    0.1
}

fn default_visual_tolerance() -> u8 {
    8
}

impl Default for VisualConfig {
    fn default() -> Self {
        VisualConfig {
            threshold_percent: default_visual_threshold(),
            tolerance: default_visual_tolerance(),
            baseline_dir: None,
            ignore: Vec::new(),
        }
    }
}

/// A path that is absolute on any platform: `/tmp/x` is not one on Windows by itself, and `C:\\x` is not one elsewhere.
fn is_absolute_anywhere(path: &str) -> bool {
    path.starts_with(['/', '\\']) || path.as_bytes().get(1) == Some(&b':') || std::path::Path::new(path).is_absolute()
}

impl VisualConfig {
    fn validate(&self) -> Result<()> {
        if !(0.0..=100.0).contains(&self.threshold_percent) {
            return Err(Error::invalid("[visual] threshold_percent (0 to 100)", self.threshold_percent.to_string()));
        }
        if let Some(dir) = &self.baseline_dir
            && (dir.is_empty() || dir.contains("..") || is_absolute_anywhere(dir))
        {
            return Err(Error::invalid("[visual] baseline_dir (a folder inside the project)", dir.clone()));
        }
        for r in &self.ignore {
            if r.width == 0 || r.height == 0 {
                return Err(Error::invalid("[visual] ignore region (a width and height above 0)", format!("{r:?}")));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub budget: Budget,
    pub size: Option<SizeConfig>,
    pub visual: Option<VisualConfig>,
    pub startup: Option<StartupConfig>,
    #[serde(default)]
    pub baseline: Baseline,
}

impl Config {
    /// Reads the text of an `emulabzip.toml`. A key that is not known is an error, so a typo does not
    /// quietly leave a limit unchecked.
    pub fn parse(text: &str) -> Result<Config> {
        let config: Config = toml::from_str(text).map_err(|e| Error::parse(FILE_NAME, e.message().to_string()))?;
        if let Some(s) = &config.startup {
            s.kind()?;
            if s.runs == 0 || s.runs > emulabzip_core::startup::MAX_RUNS {
                return Err(Error::invalid("[startup] runs (1 to 50)", s.runs.to_string()));
            }
        }
        if let Some(size) = &config.size {
            size.validate()?;
        }
        if let Some(visual) = &config.visual {
            visual.validate()?;
        }
        if !(0.0..=1000.0).contains(&config.baseline.tolerance_percent) {
            return Err(Error::invalid(
                "[baseline] tolerance_percent (0 to 1000)",
                config.baseline.tolerance_percent.to_string(),
            ));
        }
        Ok(config)
    }

    pub fn load(path: &Path) -> Result<Config> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| Error::failed(format!("could not read {}: {e}", path.display())))?;
        Config::parse(&text).map_err(|e| Error::failed(format!("{}: {e}", path.display())))
    }

    /// The file in `dir`, when there is one.
    pub fn find(dir: &Path) -> Option<std::path::PathBuf> {
        let path = dir.join(FILE_NAME);
        path.is_file().then_some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = r#"
        [budget]
        start_time_ms = 800
        memory_peak_mb = 300.5
        cpu_avg_percent = 40
        janky_frames_percent = 5
        requests = 200
        failed_requests = 0
        log_errors = 10
        crashes = 0
        [startup]
        runs = 7
        kind = "warm"
        [baseline]
        tolerance_percent = 15
    "#;

    #[test]
    fn the_visual_section_is_read_and_checked() {
        let c = Config::parse(
            "[visual]\nthreshold_percent = 0.5\ntolerance = 12\nbaseline_dir = \"ui/baselines\"\n[[visual.ignore]]\nname = \"clock\"\nx = 0\ny = 0\nwidth = 300\nheight = 80\n",
        )
        .unwrap();
        let v = c.visual.unwrap();
        assert_eq!((v.threshold_percent, v.tolerance, v.baseline_dir.as_deref()), (0.5, 12, Some("ui/baselines")));
        assert_eq!((v.ignore[0].name.as_deref(), v.ignore[0].width), (Some("clock"), 300));
        let d = Config::parse("[visual]\n").unwrap().visual.unwrap();
        assert_eq!((d.threshold_percent, d.tolerance), (0.1, 8));
        for bad in [
            "[visual]\nthreshold_percent = 101\n",
            "[visual]\nbaseline_dir = \"../elsewhere\"\n",
            "[visual]\nbaseline_dir = \"/tmp/x\"\n",
            "[visual]\nbaseline_dir = \"C:\\\\x\"\n",
            "[visual]\nbaseline_dir = \"\\\\\\\\server\\\\share\"\n",
            "[visual]\ntolerance = 300\n",
            "[[visual.ignore]]\nx = 0\ny = 0\nwidth = 0\nheight = 5\n",
            "[visual]\nthreshhold = 1\n",
        ] {
            assert!(Config::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_size_section_is_read_and_checked() {
        let c =
            Config::parse("[size]\nmax_total_mb = 25\nmax_dex_mb = 8.5\nmax_methods = 60000\nmax_growth_percent = 5\n")
                .unwrap();
        let s = c.size.unwrap();
        assert_eq!(
            (s.max_total_mb, s.max_dex_mb, s.max_methods, s.max_growth_percent),
            (Some(25.0), Some(8.5), Some(60000), Some(5.0))
        );
        assert!(Config::parse("[size]\nmax_total_mb = 0\n").is_err());
        assert!(Config::parse("[size]\nmax_growth_percent = -1\n").is_err());
        assert!(Config::parse("[size]\nmax_total = 5\n").is_err(), "a typo is an error");
        assert!(Config::parse("").unwrap().size.is_none());
    }

    #[test]
    fn reads_every_key() {
        let c = Config::parse(FULL).unwrap();
        assert_eq!(
            (c.budget.start_time_ms, c.budget.memory_peak_mb, c.budget.crashes),
            (Some(800), Some(300.5), Some(0))
        );
        assert_eq!(c.startup.as_ref().map(|s| (s.runs, s.kind().unwrap())), Some((7, StartKind::Warm)));
        assert_eq!(c.baseline.tolerance_percent, 15.0);
    }

    #[test]
    fn an_empty_file_sets_no_limits() {
        let c = Config::parse("").unwrap();
        assert_eq!(c, Config::default());
        assert_eq!(c.baseline.tolerance_percent, 10.0);
    }

    #[test]
    fn a_misspelled_key_is_an_error_not_an_unchecked_limit() {
        for text in ["[budget]\nmemory_peak = 300", "[budgets]\ncrashes = 0", "[startup]\nrun = 3", "unknown = 1"] {
            assert!(Config::parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn values_are_checked() {
        assert!(Config::parse("[startup]\nruns = 0").is_err());
        assert!(Config::parse("[startup]\nruns = 51").is_err());
        assert!(Config::parse("[startup]\nkind = \"lukewarm\"").is_err());
        assert!(Config::parse("[budget]\ncrashes = -1").is_err());
        assert!(Config::parse("[baseline]\ntolerance_percent = -5").is_err());
        assert!(Config::parse("[budget]\nrequests = \"many\"").is_err());
    }

    #[test]
    fn startup_has_sensible_defaults() {
        let s = Config::parse("[startup]").unwrap().startup.unwrap();
        assert_eq!((s.runs, s.kind().unwrap()), (5, StartKind::Cold));
    }
}
