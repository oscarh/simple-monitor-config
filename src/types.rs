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

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hz = (self.refresh as f64 / 1000.0).round() as i32;
        write!(f, "{}x{} @ {}Hz", self.width, self.height, hz)
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
        assert_eq!(mode.to_string(), "1920x1080 @ 60Hz");
    }

    #[test]
    fn mode_display_144hz() {
        let mode = Mode {
            width: 2560,
            height: 1440,
            refresh: 144000,
        };
        assert_eq!(mode.to_string(), "2560x1440 @ 144Hz");
    }

    #[test]
    fn mode_display_rounds_refresh() {
        let mode = Mode {
            width: 1920,
            height: 1080,
            refresh: 60027,
        };
        assert_eq!(mode.to_string(), "1920x1080 @ 60Hz");
    }
}
