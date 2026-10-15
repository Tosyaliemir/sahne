//! Pure color math for ambient lighting: what each LED should show for a
//! palette and mode, and smooth transitions between frames.

use crate::artwork::palette::Palette;
use crate::settings::LightingMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn parse_hex(s: &str) -> Option<Self> {
        let s = s.strip_prefix('#')?;
        if s.len() != 6 {
            return None;
        }
        let v = u32::from_str_radix(s, 16).ok()?;
        Some(Self((v >> 16) as u8, (v >> 8) as u8, v as u8))
    }

    fn lerp(self, to: Rgb, k: f64) -> Rgb {
        let mix = |a: u8, b: u8| {
            (f64::from(a) + (f64::from(b) - f64::from(a)) * k)
                .round()
                .clamp(0.0, 255.0) as u8
        };
        Rgb(mix(self.0, to.0), mix(self.1, to.1), mix(self.2, to.2))
    }

    fn scale(self, k: f64) -> Rgb {
        Rgb::default().lerp(self, k)
    }
}

/// How long one "breath" takes in `nefes` mode.
pub const BREATH_PERIOD_S: f64 = 4.0;
/// Dimmest point of a breath, as a fraction of full brightness.
const BREATH_FLOOR: f64 = 0.3;

/// Colors for `n` LEDs at time `t_s` (seconds; only matters for `nefes`).
pub fn target_frame(mode: LightingMode, palette: &Palette, n: usize, t_s: f64) -> Vec<Rgb> {
    let color = |hex: &str| Rgb::parse_hex(hex).unwrap_or_default();
    let vibrant = color(&palette.vibrant);
    match mode {
        LightingMode::Sabit => vec![vibrant; n],
        LightingMode::Gradyan => {
            let stops = [vibrant, color(&palette.accent), color(&palette.dominant)];
            (0..n)
                .map(|i| {
                    let pos = if n > 1 {
                        i as f64 / (n - 1) as f64
                    } else {
                        0.0
                    };
                    let seg = pos * (stops.len() - 1) as f64;
                    let j = (seg.floor() as usize).min(stops.len() - 2);
                    stops[j].lerp(stops[j + 1], seg - j as f64)
                })
                .collect()
        }
        LightingMode::Nefes => {
            let phase = (t_s / BREATH_PERIOD_S) * std::f64::consts::TAU;
            let level = BREATH_FLOOR + (1.0 - BREATH_FLOOR) * (0.5 - 0.5 * phase.cos());
            vec![vibrant.scale(level); n]
        }
    }
}

/// Mixes two frames (`k` = 0 → `from`, 1 → `to`). A shorter `from` (e.g. a
/// device that just appeared) is treated as black for the missing LEDs.
pub fn blend(from: &[Rgb], to: &[Rgb], k: f64) -> Vec<Rgb> {
    let k = k.clamp(0.0, 1.0);
    to.iter()
        .enumerate()
        .map(|(i, &t)| from.get(i).copied().unwrap_or_default().lerp(t, k))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> Palette {
        Palette {
            dominant: "#0000ff".into(),
            vibrant: "#ff0000".into(),
            muted: "#808080".into(),
            dark_bg: "#101010".into(),
            light_text: "#f0f0f0".into(),
            accent: "#00ff00".into(),
        }
    }

    #[test]
    fn parses_hex() {
        assert_eq!(Rgb::parse_hex("#c81e28"), Some(Rgb(200, 30, 40)));
        assert_eq!(Rgb::parse_hex("c81e28"), None);
        assert_eq!(Rgb::parse_hex("#zzzzzz"), None);
    }

    #[test]
    fn solid_mode_uses_vibrant() {
        assert_eq!(
            target_frame(LightingMode::Sabit, &palette(), 3, 0.0),
            vec![Rgb(255, 0, 0); 3]
        );
    }

    #[test]
    fn gradient_spans_three_colors() {
        let f = target_frame(LightingMode::Gradyan, &palette(), 5, 0.0);
        assert_eq!(f[0], Rgb(255, 0, 0));
        assert_eq!(f[2], Rgb(0, 255, 0));
        assert_eq!(f[4], Rgb(0, 0, 255));
        assert_eq!(f[1], Rgb(128, 128, 0));
        assert_eq!(
            target_frame(LightingMode::Gradyan, &palette(), 1, 0.0),
            vec![Rgb(255, 0, 0)]
        );
        assert!(target_frame(LightingMode::Gradyan, &palette(), 0, 0.0).is_empty());
    }

    #[test]
    fn breathing_moves_between_floor_and_full() {
        let at = |t: f64| target_frame(LightingMode::Nefes, &palette(), 1, t)[0].0;
        assert_eq!(at(0.0), (255.0 * BREATH_FLOOR).round() as u8);
        assert_eq!(at(BREATH_PERIOD_S / 2.0), 255);
        assert_eq!(at(BREATH_PERIOD_S), at(0.0));
    }

    #[test]
    fn blending_is_endpoint_exact_and_pads_with_black() {
        let a = vec![Rgb(0, 0, 0), Rgb(100, 100, 100)];
        let b = vec![Rgb(200, 0, 0), Rgb(100, 100, 100), Rgb(50, 50, 50)];
        assert_eq!(
            blend(&a, &b, 0.0),
            vec![Rgb(0, 0, 0), Rgb(100, 100, 100), Rgb(0, 0, 0)]
        );
        assert_eq!(blend(&a, &b, 1.0), b);
        assert_eq!(blend(&a, &b, 0.5)[0], Rgb(100, 0, 0));
    }
}
