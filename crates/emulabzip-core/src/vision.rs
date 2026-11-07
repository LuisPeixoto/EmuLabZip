//! How a screen looks to someone with colour blindness, and which text stops being readable.
//!
//! The simulation changes the screenshot with the matrices of Machado, Oliveira and Fernandes (2009), applied to linear
//! light at full severity, and Rec. 709 luminance for total colour blindness. The contrast check then runs on the changed
//! picture, so what it reports is text whose colours stop standing apart.

use serde::{Deserialize, Serialize};

use crate::a11y::{Audit, Image, Severity, audit, decode_png};
use crate::layout::LayoutSnapshot;
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Vision {
    /// No red cones: reds look dark and brownish.
    Protanopia,
    /// No green cones: the most common kind.
    Deuteranopia,
    /// No blue cones: blue and yellow are hard to tell apart.
    Tritanopia,
    /// No colour at all.
    Achromatopsia,
}

impl Vision {
    pub const ALL: [Vision; 4] = [Vision::Protanopia, Vision::Deuteranopia, Vision::Tritanopia, Vision::Achromatopsia];

    pub fn label(self) -> &'static str {
        match self {
            Vision::Protanopia => "protanopia",
            Vision::Deuteranopia => "deuteranopia",
            Vision::Tritanopia => "tritanopia",
            Vision::Achromatopsia => "achromatopsia",
        }
    }

    pub fn parse(text: &str) -> Option<Vision> {
        Vision::ALL.into_iter().find(|v| v.label() == text.trim().to_ascii_lowercase())
    }

    fn matrix(self) -> [[f64; 3]; 3] {
        match self {
            Vision::Protanopia => {
                [[0.152286, 1.052583, -0.204868], [0.114503, 0.786281, 0.099216], [-0.003882, -0.048116, 1.051998]]
            }
            Vision::Deuteranopia => {
                [[0.367322, 0.860646, -0.227968], [0.280085, 0.672501, 0.047413], [-0.011820, 0.042940, 0.968881]]
            }
            Vision::Tritanopia => {
                [[1.255528, -0.076749, -0.178779], [-0.078411, 0.930809, 0.147602], [0.004733, 0.691367, 0.303900]]
            }
            Vision::Achromatopsia => [[0.2126, 0.7152, 0.0722]; 3],
        }
    }
}

fn to_linear(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn to_srgb(c: f64) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let v = if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (v * 255.0).round().clamp(0.0, 255.0) as u8
}

/// One colour as `vision` sees it.
pub fn simulate_colour(rgb: [u8; 3], vision: Vision) -> [u8; 3] {
    let lin = [to_linear(rgb[0]), to_linear(rgb[1]), to_linear(rgb[2])];
    let m = vision.matrix();
    let row = |r: [f64; 3]| to_srgb(r[0] * lin[0] + r[1] * lin[1] + r[2] * lin[2]);
    [row(m[0]), row(m[1]), row(m[2])]
}

fn simulate_image(image: &Image, vision: Vision) -> Image {
    // Most screens use a few hundred colours, so each is worked out once.
    let mut cache: std::collections::HashMap<[u8; 3], [u8; 3]> = std::collections::HashMap::new();
    let mut out = Vec::with_capacity(image.rgba().len());
    for px in image.rgba().as_chunks::<4>().0 {
        let key = [px[0], px[1], px[2]];
        let c = *cache.entry(key).or_insert_with(|| simulate_colour(key, vision));
        out.extend_from_slice(&[c[0], c[1], c[2], px[3]]);
    }
    Image::from_rgba(image.width, image.height, out)
}

fn encode(image: &Image) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, image.width, image.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().map_err(|e| Error::failed(format!("Could not write the picture: {e}")))?;
        w.write_image_data(image.rgba()).map_err(|e| Error::failed(format!("Could not write the picture: {e}")))?;
    }
    Ok(out)
}

/// A screenshot as `vision` sees it, as a PNG.
pub fn simulate_png(png: &[u8], vision: Vision) -> Result<Vec<u8>> {
    let image = decode_png(png).ok_or_else(|| Error::failed("The screenshot could not be read."))?;
    encode(&simulate_image(&image, vision))
}

/// The screen as `vision` sees it, and the text that was readable normally and is not any more.
pub struct Simulation {
    pub png: Vec<u8>,
    pub audit: Audit,
}

pub fn simulate(snapshot: &LayoutSnapshot, vision: Vision) -> Result<Simulation> {
    let image = decode_png(&snapshot.screenshot).ok_or_else(|| {
        Error::failed("This layout came without a screenshot, so the screen cannot be shown as someone else sees it.")
    })?;
    let png = encode(&simulate_image(&image, vision))?;
    let normal: std::collections::HashSet<u32> =
        audit(snapshot).findings.iter().filter(|f| f.rule == "contrast").map(|f| f.node).collect();
    let changed = LayoutSnapshot { screenshot: png.clone(), ..snapshot.clone() };
    let mut result = audit(&changed);
    result.findings.retain(|f| f.rule == "contrast" && !normal.contains(&f.node));
    for f in &mut result.findings {
        f.rule = format!("contrast-{}", vision.label());
        f.severity = Severity::Warning;
        f.message = format!("With {}: {}", vision.label(), f.message);
        f.fix =
            "Do not rely on colour alone: change the lightness of the text or the background, not only the hue.".into();
    }
    Ok(Simulation { png, audit: result })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Platform;
    use crate::layout::Screen;
    use crate::layout::fixtures::node;

    #[test]
    fn vision_types_are_told_by_name() {
        assert_eq!(Vision::parse("Deuteranopia"), Some(Vision::Deuteranopia));
        assert_eq!(Vision::parse(" tritanopia "), Some(Vision::Tritanopia));
        assert_eq!(Vision::parse("blue"), None);
    }

    #[test]
    fn white_black_and_grey_stay_as_they_are() {
        for v in Vision::ALL {
            assert_eq!(simulate_colour([255, 255, 255], v), [255, 255, 255], "{v:?}");
            assert_eq!(simulate_colour([0, 0, 0], v), [0, 0, 0], "{v:?}");
            let g = simulate_colour([128, 128, 128], v);
            assert!(g.iter().all(|c| (i16::from(*c) - 128).abs() <= 2), "{v:?} {g:?}");
        }
    }

    #[test]
    fn red_and_green_move_together_for_red_green_colour_blindness() {
        let (red, green) = ([220, 40, 40], [40, 160, 40]);
        let apart =
            |a: [u8; 3], b: [u8; 3]| a.iter().zip(b).map(|(x, y)| (i32::from(*x) - i32::from(y)).abs()).sum::<i32>();
        let normal = apart(red, green);
        for v in [Vision::Protanopia, Vision::Deuteranopia] {
            let seen = apart(simulate_colour(red, v), simulate_colour(green, v));
            assert!(seen < normal / 2, "{v:?}: {seen} against {normal}");
        }
        // Total colour blindness keeps only lightness: all three channels equal.
        let grey = simulate_colour(red, Vision::Achromatopsia);
        assert!(grey[0] == grey[1] && grey[1] == grey[2]);
    }

    fn png_of(paint: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, 100, 100);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().unwrap();
            let mut data = Vec::new();
            for y in 0..100 {
                for x in 0..100 {
                    data.extend_from_slice(&paint(x, y));
                }
            }
            w.write_image_data(&data).unwrap();
        }
        out
    }

    #[test]
    fn red_text_on_black_stops_being_readable_and_a_picture_comes_back() {
        // Pure red on black has a contrast of 5.3:1, which passes. Without red cones red looks much darker, and it does not.
        let shot = png_of(|x, y| if (40..60).contains(&x) && (44..56).contains(&y) { [255, 0, 0] } else { [0, 0, 0] });
        let mut root = node("FrameLayout", (0, 0, 100, 100));
        // A box under 24 dp tall holds small text, which needs 4.5:1.
        let mut label = node("TextView", (20, 40, 60, 20));
        label.text = Some("Sale".into());
        root.children.push(label);
        let snapshot =
            LayoutSnapshot::new(Platform::Android, root, Screen { width: 100, height: 100, density: 1.0 }, shot);
        let sim = simulate(&snapshot, Vision::Protanopia).unwrap();
        assert_eq!(decode_png(&sim.png).map(|i| (i.width, i.height)), Some((100, 100)));
        assert_eq!(sim.audit.findings.len(), 1, "{:?}", sim.audit.findings);
        let f = &sim.audit.findings[0];
        assert_eq!(f.rule, "contrast-protanopia");
        assert!(f.message.starts_with("With protanopia: "));

        // A screen with no screenshot cannot be shown.
        let empty = LayoutSnapshot::new(
            Platform::Android,
            node("FrameLayout", (0, 0, 10, 10)),
            Screen { width: 10, height: 10, density: 1.0 },
            Vec::new(),
        );
        assert!(simulate(&empty, Vision::Protanopia).is_err());
        assert!(simulate_png(&[], Vision::Protanopia).is_err());
    }
}
