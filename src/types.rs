#[derive(Debug, Clone, PartialEq)]
pub struct OutputInfo {
    pub name: String,
    pub make: String,
    pub model: String,
    pub serial: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub modes: Vec<Mode>,
    pub current_mode: Mode,
    pub scale: f64,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mode {
    pub width: i32,
    pub height: i32,
    pub refresh: i32,
}

impl OutputInfo {
    // Sway exposes neither the EDID preferred mode nor the physical size, so
    // the largest mode is the best available guess at the panel's native one.
    pub fn native_mode(&self) -> Option<&Mode> {
        self.modes
            .iter()
            .max_by_key(|m| i64::from(m.width) * i64::from(m.height))
    }
}

const KNOWN_RATIOS: &[(i32, i32)] = &[
    (1, 1),
    (5, 4),
    (4, 3),
    (3, 2),
    (16, 10),
    (16, 9),
    (17, 9),
    (21, 9),
    (32, 9),
];

// Marketing names like 21:9 are only approximations of the real pixel ratio
// (3440x1440 is 43:18), so those are matched loosely and marked with "~".
const APPROX_TOLERANCE: f64 = 0.03;

fn gcd(a: i32, b: i32) -> i32 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

impl Mode {
    pub fn refresh_hz(&self) -> i32 {
        (self.refresh as f64 / 1000.0).round() as i32
    }

    pub fn aspect_ratio(&self) -> String {
        let (w, h) = (self.width, self.height);
        if w <= 0 || h <= 0 {
            return "?".to_string();
        }

        if let Some((kw, kh)) = KNOWN_RATIOS.iter().find(|(kw, kh)| w * kh == h * kw) {
            return format!("{}:{}", kw, kh);
        }

        let d = gcd(w, h);
        if w / d <= 32 && h / d <= 32 {
            return format!("{}:{}", w / d, h / d);
        }

        let ratio = w as f64 / h as f64;
        let nearest = KNOWN_RATIOS.iter().min_by(|a, b| {
            let da = (a.0 as f64 / a.1 as f64 - ratio).abs();
            let db = (b.0 as f64 / b.1 as f64 - ratio).abs();
            da.total_cmp(&db)
        });
        if let Some((kw, kh)) = nearest {
            let known = *kw as f64 / *kh as f64;
            if (known - ratio).abs() / known <= APPROX_TOLERANCE {
                return format!("~{}:{}", kw, kh);
            }
        }

        format!("{:.2}:1", ratio)
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}x{} ({}) @ {}Hz",
            self.width,
            self.height,
            self.aspect_ratio(),
            self.refresh_hz()
        )
    }
}

#[cfg(test)]
pub fn test_output(name: &str, x: i32, y: i32, w: i32, h: i32) -> OutputInfo {
    let mode = Mode {
        width: w,
        height: h,
        refresh: 60000,
    };
    OutputInfo {
        name: name.to_string(),
        make: String::new(),
        model: String::new(),
        serial: String::new(),
        x,
        y,
        width: w,
        height: h,
        modes: vec![mode.clone()],
        current_mode: mode,
        scale: 1.0,
        active: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_display_60hz() {
        let mode = Mode {
            width: 1920,
            height: 1080,
            refresh: 60000,
        };
        assert_eq!(mode.to_string(), "1920x1080 (16:9) @ 60Hz");
    }

    #[test]
    fn mode_display_144hz() {
        let mode = Mode {
            width: 2560,
            height: 1440,
            refresh: 144000,
        };
        assert_eq!(mode.to_string(), "2560x1440 (16:9) @ 144Hz");
    }

    #[test]
    fn mode_display_rounds_refresh() {
        let mode = Mode {
            width: 1920,
            height: 1080,
            refresh: 60027,
        };
        assert_eq!(mode.to_string(), "1920x1080 (16:9) @ 60Hz");
    }

    fn ratio(width: i32, height: i32) -> String {
        Mode {
            width,
            height,
            refresh: 60000,
        }
        .aspect_ratio()
    }

    #[test]
    fn aspect_ratio_common_exact() {
        assert_eq!(ratio(1920, 1080), "16:9");
        assert_eq!(ratio(1920, 1200), "16:10");
        assert_eq!(ratio(1024, 768), "4:3");
        assert_eq!(ratio(1280, 1024), "5:4");
        assert_eq!(ratio(5120, 1440), "32:9");
    }

    #[test]
    fn aspect_ratio_small_uncommon_is_reduced() {
        assert_eq!(ratio(720, 400), "9:5");
    }

    #[test]
    fn aspect_ratio_approximates_marketing_names() {
        assert_eq!(ratio(1366, 768), "~16:9");
        assert_eq!(ratio(3440, 1440), "~21:9");
        assert_eq!(ratio(2560, 1080), "~21:9");
        assert_eq!(ratio(2048, 1080), "~17:9");
    }

    #[test]
    fn aspect_ratio_falls_back_to_decimal() {
        assert_eq!(ratio(1000, 293), "3.41:1");
    }

    #[test]
    fn aspect_ratio_zero_size() {
        assert_eq!(ratio(0, 0), "?");
    }

    #[test]
    fn native_mode_is_largest() {
        let mut o = test_output("DP-1", 0, 0, 1920, 1080);
        o.modes = vec![
            Mode {
                width: 1920,
                height: 1080,
                refresh: 60000,
            },
            Mode {
                width: 3440,
                height: 1440,
                refresh: 60000,
            },
            Mode {
                width: 2560,
                height: 1440,
                refresh: 60000,
            },
        ];
        assert_eq!(o.native_mode().map(|m| m.width), Some(3440));
    }
}
