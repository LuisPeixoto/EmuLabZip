//! Comparing two screenshots: how much of the screen changed, where, and a picture that shows it.

use std::path::{Path, PathBuf};

use emulabzip_core::a11y::{Image, decode_png};
use emulabzip_core::{Error, Result};
use serde::{Deserialize, Serialize};

/// A rectangle of the screenshot, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Region {
    fn contains(&self, x: u32, y: u32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    /// How far a colour channel may differ (0 to 255) before the pixel counts as changed. Anti-aliasing and the
    /// encoder's rounding differ by a few levels.
    pub tolerance: u8,
    /// Parts of the screen that are allowed to change: a clock, an ad, a video.
    pub ignore: Vec<Region>,
}

impl Default for Options {
    fn default() -> Self {
        Options { tolerance: 8, ignore: Vec::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub width: u32,
    pub height: u32,
    pub changed: u64,
    /// Pixels that were compared (the screen minus the ignored parts).
    pub compared: u64,
    pub percent: f64,
    /// The smallest rectangle that holds every change.
    pub bounds: Option<Region>,
}

impl Comparison {
    /// Whether more than `threshold_percent` of the screen changed.
    pub fn exceeds(&self, threshold_percent: f64) -> bool {
        self.changed > 0 && self.percent > threshold_percent
    }
}

/// A comparison and the picture of it.
pub struct Compared {
    pub comparison: Comparison,
    /// A PNG: the changed pixels in red over a faded copy of the screen.
    pub diff: Vec<u8>,
}

fn decode(bytes: &[u8], what: &str) -> Result<Image> {
    decode_png(bytes).ok_or_else(|| Error::parse("a PNG screenshot", format!("{what} is not a readable PNG")))
}

pub fn compare(before: &[u8], after: &[u8], options: &Options) -> Result<Compared> {
    let a = decode(before, "the baseline")?;
    let b = decode(after, "the new screenshot")?;
    if (a.width, a.height) != (b.width, b.height) {
        return Err(Error::failed(format!(
            "The screenshots are not the same size ({}×{} and {}×{}): they come from different devices, orientations or display sizes.",
            a.width, a.height, b.width, b.height
        )));
    }
    let (w, h) = (a.width, a.height);
    let (ra, rb) = (a.rgba(), b.rgba());
    let mut diff = Vec::with_capacity(ra.len());
    let (mut changed, mut compared) = (0u64, 0u64);
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (u32::MAX, u32::MAX, 0u32, 0u32);
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let (pa, pb) = (&ra[i..i + 4], &rb[i..i + 4]);
            let ignored = options.ignore.iter().any(|r| r.contains(x, y));
            let differs = !ignored && pa[..3].iter().zip(&pb[..3]).any(|(p, q)| p.abs_diff(*q) > options.tolerance);
            if !ignored {
                compared += 1;
            }
            if differs {
                changed += 1;
                (min_x, min_y, max_x, max_y) = (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y));
                diff.extend_from_slice(&[255, 0, 64, 255]);
            } else {
                // The screen, faded, so the red stands out.
                let fade = |c: u8| ((u32::from(c) * 35 + 255 * 65) / 100) as u8;
                diff.extend_from_slice(&[fade(pb[0]), fade(pb[1]), fade(pb[2]), 255]);
            }
        }
    }
    let bounds =
        (changed > 0).then(|| Region { x: min_x, y: min_y, width: max_x - min_x + 1, height: max_y - min_y + 1 });
    let percent = if compared == 0 { 0.0 } else { changed as f64 * 100.0 / compared as f64 };
    let mut png_out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png_out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|e| Error::failed(format!("Could not write the picture: {e}")))?;
        writer.write_image_data(&diff).map_err(|e| Error::failed(format!("Could not write the picture: {e}")))?;
    }
    Ok(Compared { comparison: Comparison { width: w, height: h, changed, compared, percent, bounds }, diff: png_out })
}

// ---------------------------------------------------------------------------------------------
// Baselines
// ---------------------------------------------------------------------------------------------

/// A baseline's name is a file name: letters, digits, `.`, `_` and `-`.
pub fn is_baseline_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

pub fn baseline_path(dir: &Path, name: &str) -> Result<PathBuf> {
    if !is_baseline_name(name) {
        return Err(Error::invalid("baseline name (letters, digits, . _ and -)", name));
    }
    Ok(dir.join(format!("{name}.png")))
}

pub fn load_baseline(dir: &Path, name: &str) -> Result<Vec<u8>> {
    let path = baseline_path(dir, name)?;
    std::fs::read(&path).map_err(|_| {
        Error::NotFound(format!(
            "there is no baseline called {name} in {} (make one with `emulabzip visual capture {name}`)",
            dir.display()
        ))
    })
}

/// Writes a baseline. An existing one is only replaced when `replace` is set.
pub fn save_baseline(dir: &Path, name: &str, png: &[u8], replace: bool) -> Result<PathBuf> {
    let path = baseline_path(dir, name)?;
    if decode_png(png).is_none() {
        return Err(Error::failed("That is not a PNG screenshot."));
    }
    if path.exists() && !replace {
        return Err(Error::failed(format!("{} exists already; `visual update` replaces it", path.display())));
    }
    std::fs::create_dir_all(dir)?;
    std::fs::write(&path, png)?;
    Ok(path)
}

/// The baselines in a folder, with their sizes, by name.
pub fn list_baselines(dir: &Path) -> Vec<(String, u64)> {
    let mut out: Vec<(String, u64)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let file = e.file_name().to_string_lossy().into_owned();
            let name = file.strip_suffix(".png")?;
            is_baseline_name(name).then(|| (name.to_string(), e.metadata().map_or(0, |m| m.len())))
        })
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, paint: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, w, h);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            let mut wr = enc.write_header().unwrap();
            let mut data = Vec::new();
            for y in 0..h {
                for x in 0..w {
                    data.extend_from_slice(&paint(x, y));
                }
            }
            wr.write_image_data(&data).unwrap();
        }
        out
    }

    fn white(w: u32, h: u32) -> Vec<u8> {
        png(w, h, |_, _| [255, 255, 255])
    }

    #[test]
    fn identical_screens_have_no_changes() {
        let c = compare(&white(40, 30), &white(40, 30), &Options::default()).unwrap().comparison;
        assert_eq!((c.changed, c.compared, c.percent), (0, 1200, 0.0));
        assert_eq!(c.bounds, None);
        assert!(!c.exceeds(0.0));
    }

    #[test]
    fn a_change_is_counted_placed_and_drawn() {
        let a = white(40, 30);
        let b =
            png(40, 30, |x, y| if (10..14).contains(&x) && (5..7).contains(&y) { [0, 0, 0] } else { [255, 255, 255] });
        let done = compare(&a, &b, &Options::default()).unwrap();
        let c = &done.comparison;
        assert_eq!(c.changed, 8);
        assert_eq!(c.bounds, Some(Region { x: 10, y: 5, width: 4, height: 2 }));
        assert!((c.percent - 8.0 * 100.0 / 1200.0).abs() < 1e-9);
        assert!(c.exceeds(0.1) && !c.exceeds(1.0));
        // The picture has the same size, red where it changed and faded white elsewhere.
        let img = decode_png(&done.diff).unwrap();
        assert_eq!((img.width, img.height), (40, 30));
        assert_eq!(&img.rgba()[((5 * 40 + 10) * 4) as usize..][..3], &[255, 0, 64]);
        assert_eq!(&img.rgba()[..3], &[255, 255, 255]);
    }

    #[test]
    fn small_differences_and_ignored_parts_do_not_count() {
        let a = white(40, 30);
        let nearly = png(40, 30, |_, _| [250, 252, 255]);
        assert_eq!(compare(&a, &nearly, &Options::default()).unwrap().comparison.changed, 0);
        assert_eq!(compare(&a, &nearly, &Options { tolerance: 2, ignore: vec![] }).unwrap().comparison.changed, 1200);

        // A clock in the corner is ignored; a change elsewhere still counts.
        let clock = png(40, 30, |x, y| if x < 10 && y < 5 { [0, 0, 0] } else { [255, 255, 255] });
        let ignore = vec![Region { x: 0, y: 0, width: 10, height: 5 }];
        let c = compare(&a, &clock, &Options { tolerance: 8, ignore: ignore.clone() }).unwrap().comparison;
        assert_eq!((c.changed, c.compared), (0, 1200 - 50));
        let moved =
            png(40, 30, |x, y| if (x < 10 && y < 5) || x == 30 && y == 20 { [0, 0, 0] } else { [255, 255, 255] });
        assert_eq!(compare(&a, &moved, &Options { tolerance: 8, ignore }).unwrap().comparison.changed, 1);
    }

    #[test]
    fn screens_of_another_size_or_no_picture_are_explained() {
        let e = compare(&white(40, 30), &white(30, 40), &Options::default()).err().unwrap().to_string();
        assert!(e.contains("not the same size") && e.contains("40×30"), "{e}");
        let e = compare(b"nope", &white(4, 4), &Options::default()).err().unwrap().to_string();
        assert!(e.contains("baseline"), "{e}");
    }

    #[test]
    fn baselines_are_kept_by_name_and_never_replaced_by_accident() {
        let dir = std::env::temp_dir().join(format!("emulabzip-visual-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let shot = white(8, 8);
        let path = save_baseline(&dir, "home-screen", &shot, false).unwrap();
        assert!(path.ends_with("home-screen.png"));
        assert!(save_baseline(&dir, "home-screen", &shot, false).unwrap_err().to_string().contains("exists already"));
        assert!(save_baseline(&dir, "home-screen", &shot, true).is_ok());
        assert_eq!(load_baseline(&dir, "home-screen").unwrap(), shot);
        assert_eq!(list_baselines(&dir).iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["home-screen"]);
        assert!(load_baseline(&dir, "missing").unwrap_err().to_string().contains("visual capture missing"));
        for bad in ["", "../x", "a/b", ".hidden", "a b", "x;y"] {
            assert!(baseline_path(&dir, bad).is_err(), "{bad:?}");
        }
        assert!(save_baseline(&dir, "junk", b"not a png", false).is_err());
        std::fs::remove_dir_all(dir).ok();
    }
}
