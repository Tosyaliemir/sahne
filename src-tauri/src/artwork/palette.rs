//! Album-art palette extraction: k-means in OKLab, then role assignment with
//! a guaranteed WCAG contrast between text and background.

use image::{imageops::FilterType, DynamicImage};
use serde::Serialize;

const K: usize = 6;
const ITERATIONS: usize = 12;
const SAMPLE_SIZE: u32 = 64;
/// WCAG AA for normal text is 4.5:1; we aim higher so dimmed text stays readable.
const TARGET_CONTRAST: f64 = 7.0;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Palette {
    pub dominant: String,
    pub vibrant: String,
    pub muted: String,
    pub dark_bg: String,
    pub light_text: String,
    pub accent: String,
}

impl Default for Palette {
    /// Neutral palette used when there is no artwork.
    fn default() -> Self {
        from_clusters(&[])
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lab {
    pub l: f64,
    pub a: f64,
    pub b: f64,
}

impl Lab {
    pub fn chroma(&self) -> f64 {
        self.a.hypot(self.b)
    }

    fn hue(&self) -> f64 {
        self.b.atan2(self.a)
    }

    fn from_lch(l: f64, c: f64, h: f64) -> Self {
        Self {
            l,
            a: c * h.cos(),
            b: c * h.sin(),
        }
    }

    fn dist2(&self, o: &Lab) -> f64 {
        (self.l - o.l).powi(2) + (self.a - o.a).powi(2) + (self.b - o.b).powi(2)
    }
}

pub fn extract(img: &DynamicImage) -> Palette {
    let small = img
        .resize_exact(SAMPLE_SIZE, SAMPLE_SIZE, FilterType::Triangle)
        .to_rgba8();
    let pixels: Vec<Lab> = small
        .pixels()
        .filter(|p| p[3] >= 128)
        .map(|p| srgb8_to_oklab([p[0], p[1], p[2]]))
        .collect();
    from_clusters(&kmeans(&pixels))
}

/// Deterministic k-means: seeds are spread across the lightness range.
fn kmeans(pixels: &[Lab]) -> Vec<(Lab, f64)> {
    if pixels.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<Lab> = pixels.to_vec();
    sorted.sort_by(|x, y| x.l.total_cmp(&y.l));
    let mut centers: Vec<Lab> = (0..K)
        .map(|i| sorted[(i * 2 + 1) * sorted.len() / (K * 2)])
        .collect();
    let mut assignment = vec![0usize; pixels.len()];

    for _ in 0..ITERATIONS {
        for (i, p) in pixels.iter().enumerate() {
            assignment[i] = nearest(&centers, p);
        }
        let mut sums = vec![(0.0, 0.0, 0.0, 0usize); centers.len()];
        for (p, &c) in pixels.iter().zip(&assignment) {
            let s = &mut sums[c];
            s.0 += p.l;
            s.1 += p.a;
            s.2 += p.b;
            s.3 += 1;
        }
        for (center, (l, a, b, n)) in centers.iter_mut().zip(sums) {
            if n > 0 {
                let n = n as f64;
                *center = Lab {
                    l: l / n,
                    a: a / n,
                    b: b / n,
                };
            }
        }
    }

    let mut counts = vec![0usize; centers.len()];
    for &c in &assignment {
        counts[c] += 1;
    }
    let total = pixels.len() as f64;
    let mut clusters: Vec<(Lab, f64)> = centers
        .into_iter()
        .zip(counts)
        .filter(|(_, n)| *n > 0)
        .map(|(c, n)| (c, n as f64 / total))
        .collect();
    // Seeds can converge onto the same color; merge near-duplicates.
    clusters.sort_by(|x, y| y.1.total_cmp(&x.1));
    let mut merged: Vec<(Lab, f64)> = Vec::new();
    for (c, share) in clusters {
        match merged
            .iter_mut()
            .find(|(m, _)| m.dist2(&c) < 0.02f64.powi(2))
        {
            Some(m) => m.1 += share,
            None => merged.push((c, share)),
        }
    }
    merged
}

fn nearest(centers: &[Lab], p: &Lab) -> usize {
    centers
        .iter()
        .enumerate()
        .min_by(|(_, x), (_, y)| x.dist2(p).total_cmp(&y.dist2(p)))
        .map_or(0, |(i, _)| i)
}

/// Assigns palette roles from clusters sorted by share (largest first).
fn from_clusters(clusters: &[(Lab, f64)]) -> Palette {
    let neutral = Lab {
        l: 0.6,
        a: 0.0,
        b: 0.0,
    };
    let dominant = clusters.first().map_or(neutral, |c| c.0);

    // Too dark, too light or too grey clusters can't carry color.
    let usable: Vec<&(Lab, f64)> = clusters
        .iter()
        .filter(|(c, share)| (0.25..=0.92).contains(&c.l) && *share >= 0.02)
        .collect();
    let vividness = |(c, share): &&(Lab, f64)| c.chroma() * share.sqrt();

    let vibrant = usable
        .iter()
        .filter(|(c, _)| c.chroma() >= 0.08)
        .max_by(|x, y| vividness(x).total_cmp(&vividness(y)))
        .map(|(c, _)| *c)
        .unwrap_or_else(|| with_lightness(dominant, 0.7));

    let muted = usable
        .iter()
        .filter(|(c, _)| (0.02..0.1).contains(&c.chroma()))
        .max_by(|x, y| x.1.total_cmp(&y.1))
        .map(|(c, _)| *c)
        .unwrap_or_else(|| {
            let c = dominant.chroma().min(0.05);
            Lab::from_lch(0.55, c, dominant.hue())
        });

    // Accent: the most vivid color that clearly differs from the dominant one.
    let accent = usable
        .iter()
        .filter(|(c, _)| c.chroma() >= 0.08 && c.dist2(&dominant) > 0.12f64.powi(2))
        .max_by(|x, y| vividness(x).total_cmp(&vividness(y)))
        .map_or(vibrant, |(c, _)| *c);
    let accent = with_lightness(accent, accent.l.clamp(0.65, 0.85));

    // Background: a deep, low-chroma version of the dominant hue.
    let mut bg = Lab::from_lch(0.22, (dominant.chroma() * 0.6).min(0.06), dominant.hue());
    let mut text = Lab::from_lch(0.96, dominant.chroma().min(0.02), dominant.hue());
    // Darken the background (then brighten the text) until contrast is met.
    for _ in 0..40 {
        if contrast(&bg, &text) >= TARGET_CONTRAST {
            break;
        }
        if bg.l > 0.08 {
            bg = with_lightness(bg, bg.l - 0.01);
        } else if text.l < 0.995 {
            text = with_lightness(text, (text.l + 0.005).min(0.995));
        } else {
            break;
        }
    }

    Palette {
        dominant: hex(dominant),
        vibrant: hex(vibrant),
        muted: hex(muted),
        dark_bg: hex(bg),
        light_text: hex(text),
        accent: hex(accent),
    }
}

fn with_lightness(c: Lab, l: f64) -> Lab {
    Lab { l, ..c }
}

// --- color math -----------------------------------------------------------

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f64) -> f64 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

pub fn srgb8_to_oklab(rgb: [u8; 3]) -> Lab {
    let [r, g, b] = rgb.map(|c| srgb_to_linear(f64::from(c) / 255.0));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    Lab {
        l: 0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        a: 1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        b: 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    }
}

/// Linear sRGB, possibly out of [0, 1].
fn oklab_to_linear(c: Lab) -> [f64; 3] {
    let l = (c.l + 0.396_337_777_4 * c.a + 0.215_803_757_3 * c.b).powi(3);
    let m = (c.l - 0.105_561_345_8 * c.a - 0.063_854_172_8 * c.b).powi(3);
    let s = (c.l - 0.089_484_177_5 * c.a - 1.291_485_548 * c.b).powi(3);
    [
        4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
        -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
        -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s,
    ]
}

/// Maps into the sRGB gamut by reducing chroma (keeps lightness and hue).
fn to_gamut(c: Lab) -> [f64; 3] {
    let in_gamut = |rgb: &[f64; 3]| rgb.iter().all(|v| (-1e-4..=1.0 + 1e-4).contains(v));
    let rgb = oklab_to_linear(c);
    if in_gamut(&rgb) {
        return rgb.map(|v| v.clamp(0.0, 1.0));
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..20 {
        let mid = (lo + hi) / 2.0;
        if in_gamut(&oklab_to_linear(Lab {
            l: c.l,
            a: c.a * mid,
            b: c.b * mid,
        })) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    oklab_to_linear(Lab {
        l: c.l,
        a: c.a * lo,
        b: c.b * lo,
    })
    .map(|v| v.clamp(0.0, 1.0))
}

pub fn to_srgb8(c: Lab) -> [u8; 3] {
    to_gamut(c).map(|v| (linear_to_srgb(v) * 255.0).round().clamp(0.0, 255.0) as u8)
}

fn hex(c: Lab) -> String {
    let [r, g, b] = to_srgb8(c);
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// WCAG relative luminance of the gamut-mapped color.
fn luminance(c: &Lab) -> f64 {
    // Round-trip through 8-bit so we measure exactly what will be displayed.
    let [r, g, b] = to_srgb8(*c).map(|v| srgb_to_linear(f64::from(v) / 255.0));
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

pub fn contrast(x: &Lab, y: &Lab) -> f64 {
    let (a, b) = (luminance(x), luminance(y));
    let (hi, lo) = if a > b { (a, b) } else { (b, a) };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
pub fn parse_hex(s: &str) -> Lab {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap();
    srgb8_to_oklab([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn image_from(f: impl Fn(u32, u32) -> [u8; 3]) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(100, 100, |x, y| Rgb(f(x, y))))
    }

    const MIN_CONTRAST: f64 = 4.5;

    fn assert_readable(p: &Palette) {
        let c = contrast(&parse_hex(&p.dark_bg), &parse_hex(&p.light_text));
        assert!(c >= MIN_CONTRAST, "contrast {c:.2} too low for {p:?}");
    }

    fn hue_deg(hex: &str) -> f64 {
        parse_hex(hex).hue().to_degrees().rem_euclid(360.0)
    }

    #[test]
    fn oklab_roundtrip() {
        for rgb in [
            [0, 0, 0],
            [255, 255, 255],
            [255, 0, 0],
            [12, 200, 99],
            [30, 30, 200],
        ] {
            assert_eq!(to_srgb8(srgb8_to_oklab(rgb)), rgb);
        }
        let white = srgb8_to_oklab([255, 255, 255]);
        assert!((white.l - 1.0).abs() < 1e-3 && white.chroma() < 1e-3);
    }

    #[test]
    fn solid_color_image() {
        let p = extract(&image_from(|_, _| [200, 30, 40]));
        assert_eq!(p.dominant, "#c81e28");
        assert_eq!(p.vibrant, "#c81e28");
        // Background and accent keep the red hue.
        let red = hue_deg("#c81e28");
        for hex in [&p.dark_bg, &p.accent] {
            assert!((hue_deg(hex) - red).abs() < 10.0, "{hex} drifted from red");
        }
        assert_readable(&p);
    }

    #[test]
    fn two_color_image() {
        // 70% blue, 30% yellow.
        let p = extract(&image_from(|x, _| {
            if x < 70 {
                [30, 60, 200]
            } else {
                [240, 210, 40]
            }
        }));
        assert!(
            parse_hex(&p.dominant).dist2(&srgb8_to_oklab([30, 60, 200])) < 0.03f64.powi(2),
            "{p:?}"
        );
        // The accent picks the other, clearly different color.
        assert!(
            (hue_deg(&p.accent) - hue_deg("#f0d228")).abs() < 10.0,
            "{p:?}"
        );
        assert_readable(&p);
    }

    #[test]
    fn black_and_white_image() {
        let p = extract(&image_from(|x, y| {
            if (x / 10 + y / 10) % 2 == 0 {
                [0, 0, 0]
            } else {
                [255, 255, 255]
            }
        }));
        for hex in [&p.dark_bg, &p.light_text, &p.vibrant, &p.accent, &p.muted] {
            assert!(
                parse_hex(hex).chroma() < 0.01,
                "{hex} should be grey in {p:?}"
            );
        }
        assert_readable(&p);
    }

    #[test]
    fn default_palette_is_readable() {
        assert_readable(&Palette::default());
    }

    #[test]
    fn very_light_image_still_gets_dark_background() {
        let p = extract(&image_from(|_, _| [250, 245, 235]));
        assert!(parse_hex(&p.dark_bg).l < 0.3);
        assert_readable(&p);
    }
}
