use ratatui::style::Color;
use std::env;

const LOW_RGB: (u8, u8, u8) = (0, 200, 90);
const MID_RGB: (u8, u8, u8) = (240, 200, 0);
const HIGH_RGB: (u8, u8, u8) = (235, 60, 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorLevel {
    /// No colour at all (NO_COLOR is set).
    Mono,
    /// The 16 named ANSI colours. Safe on legacy cmd.exe.
    Basic,
    /// The 256-colour indexed palette.
    Ansi256,
    /// 24-bit RGB.
    TrueColor,
}

impl ColorLevel {
    pub fn detect() -> Self {
        let var = |key: &str| env::var(key).ok();

        if env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
            return Self::Mono;
        }

        // Windows Terminal sets WT_SESSION and renders 24-bit colour.
        if var("WT_SESSION").is_some() {
            return Self::TrueColor;
        }

        // Terminals embedded in editors that are known to support 24-bit colour.
        if matches!(
            var("TERM_PROGRAM").as_deref(),
            Some("vscode" | "WezTerm" | "iTerm.app" | "Hyper")
        ) {
            return Self::TrueColor;
        }

        // Legacy conhost / cmd.exe: stay on the 16 named colours to be safe.
        if cfg!(windows) {
            return Self::Basic;
        }

        if matches!(
            var("COLORTERM").as_deref(),
            Some("truecolor" | "24bit")
        ) {
            return Self::TrueColor;
        }

        match var("TERM") {
            Some(term) if term.contains("256color") => Self::Ansi256,
            _ => Self::Basic,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub level: ColorLevel,
    pub title: Color,
    pub border: Color,
    pub text: Color,
    pub increase: Color,
    pub decrease: Color,
    pub gauge_low: Color,
    pub gauge_mid: Color,
    pub gauge_high: Color,
}

impl Theme {
    pub fn detect() -> Self {
        Self::for_level(ColorLevel::detect())
    }

    pub fn for_level(level: ColorLevel) -> Self {
        match level {
            ColorLevel::Mono => Self {
                level,
                title: Color::Reset,
                border: Color::Reset,
                text: Color::Reset,
                increase: Color::Reset,
                decrease: Color::Reset,
                gauge_low: Color::Reset,
                gauge_mid: Color::Reset,
                gauge_high: Color::Reset,
            },
            ColorLevel::Basic => Self {
                level,
                title: Color::Cyan,
                border: Color::DarkGray,
                text: Color::Reset,
                increase: Color::LightRed,
                decrease: Color::LightGreen,
                gauge_low: Color::Green,
                gauge_mid: Color::Yellow,
                gauge_high: Color::Red,
            },
            ColorLevel::Ansi256 => Self {
                level,
                title: Color::Indexed(117),
                border: Color::Indexed(60),
                text: Color::Reset,
                increase: Color::Indexed(209),
                decrease: Color::Indexed(114),
                gauge_low: Color::Indexed(78),
                gauge_mid: Color::Indexed(221),
                gauge_high: Color::Indexed(203),
            },
            ColorLevel::TrueColor => Self {
                level,
                title: Color::Rgb(120, 190, 255),
                border: Color::Rgb(80, 100, 130),
                text: Color::Reset,
                increase: Color::Rgb(255, 120, 100),
                decrease: Color::Rgb(90, 220, 150),
                gauge_low: Color::Rgb(
                    LOW_RGB.0, LOW_RGB.1, LOW_RGB.2,
                ),
                gauge_mid: Color::Rgb(
                    MID_RGB.0, MID_RGB.1, MID_RGB.2,
                ),
                gauge_high: Color::Rgb(
                    HIGH_RGB.0, HIGH_RGB.1, HIGH_RGB.2,
                ),
            },
        }
    }

    /// Colour for a memory usage percentage (0-100).
    pub fn usage_color(
        &self,
        percent: f64,
    ) -> Color {
        let p = percent.clamp(0.0, 100.0);
        match self.level {
            ColorLevel::Mono => Color::Reset,
            ColorLevel::TrueColor => {
                let t = p / 100.0;
                if t < 0.5 {
                    lerp(LOW_RGB, MID_RGB, t * 2.0)
                } else {
                    lerp(MID_RGB, HIGH_RGB, (t - 0.5) * 2.0)
                }
            }
            ColorLevel::Basic | ColorLevel::Ansi256 => {
                if p < 60.0 {
                    self.gauge_low
                } else if p < 85.0 {
                    self.gauge_mid
                } else {
                    self.gauge_high
                }
            }
        }
    }
}

fn lerp(
    a: (u8, u8, u8),
    b: (u8, u8, u8),
    t: f64,
) -> Color {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| {
        (x as f64 + (y as f64 - x as f64) * t).round() as u8
    };
    Color::Rgb(mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2))
}
