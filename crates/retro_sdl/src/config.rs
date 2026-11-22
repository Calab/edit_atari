//! User settings, stored as `key = value` lines in the OS config folder:
//!
//! * Linux:   `$XDG_CONFIG_HOME/<app>/settings.txt` (or `~/.config/...`)
//! * Windows: `%APPDATA%\<app>\settings.txt`
//! * macOS:   `~/Library/Application Support/<app>/settings.txt`

use std::path::PathBuf;

use retro_core::WrapMode;

/// Which PC key acts as the machine's CONTROL key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKey {
    RightCtrl,
    LeftAlt,
}

impl ControlKey {
    pub fn label(self) -> &'static str {
        match self {
            ControlKey::RightCtrl => "RIGHT-CTRL",
            ControlKey::LeftAlt => "LEFT-ALT",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub control_key: ControlKey,
    pub wrap: WrapMode,
    /// Pixels per font pixel.
    pub scale: u32,
    pub key_click: bool,
    pub click_volume: f32,
    /// Characters past this column are drawn in the warning colour;
    /// 0 turns the warning off.
    pub line_warning: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            control_key: ControlKey::RightCtrl,
            wrap: WrapMode::Native,
            scale: 3,
            key_click: true,
            click_volume: 0.25,
            line_warning: DEFAULT_LINE_WARNING,
        }
    }
}

pub const MIN_SCALE: u32 = 1;
pub const MAX_SCALE: u32 = 8;
/// The Atari screen editor's logical line: 3 rows of 38 characters.
pub const DEFAULT_LINE_WARNING: usize = 114;
pub const MAX_LINE_WARNING: usize = 9999;

impl Config {
    pub fn path(app_id: &str) -> Option<PathBuf> {
        let env = |k: &str| {
            std::env::var_os(k)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        let base = if cfg!(windows) {
            env("APPDATA")?
        } else if cfg!(target_os = "macos") {
            env("HOME")?.join("Library/Application Support")
        } else {
            env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config")))?
        };
        Some(base.join(app_id).join("settings.txt"))
    }

    /// Load settings; missing or unreadable values fall back to defaults.
    pub fn load(app_id: &str) -> Self {
        Self::path(app_id)
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|s| Self::parse(&s))
            .unwrap_or_default()
    }

    pub fn parse(text: &str) -> Self {
        let mut c = Self::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim();
            match k.trim() {
                "control_key" => match v {
                    "right_ctrl" => c.control_key = ControlKey::RightCtrl,
                    "left_alt" => c.control_key = ControlKey::LeftAlt,
                    _ => {}
                },
                "wrap" => match v {
                    "native" => c.wrap = WrapMode::Native,
                    "window" => c.wrap = WrapMode::Window,
                    _ => {}
                },
                "scale" => {
                    if let Ok(n) = v.parse::<u32>() {
                        c.scale = n.clamp(MIN_SCALE, MAX_SCALE);
                    }
                }
                "key_click" => c.key_click = v != "off",
                "line_warning" => {
                    if let Ok(n) = v.parse::<usize>() {
                        c.line_warning = n.min(MAX_LINE_WARNING);
                    }
                }
                "click_volume" => {
                    if let Ok(f) = v.parse::<f32>() {
                        c.click_volume = f.clamp(0.0, 1.0);
                    }
                }
                _ => {}
            }
        }
        c
    }

    pub fn to_text(&self) -> String {
        format!(
            "# Retro editor settings\n\
             # control_key: right_ctrl | left_alt\n\
             control_key = {}\n\
             # wrap: native (40 columns) | window\n\
             wrap = {}\n\
             scale = {}\n\
             key_click = {}\n\
             click_volume = {}\n\
             # line_warning: highlight characters past this column (0 = off)\n\
             line_warning = {}\n",
            match self.control_key {
                ControlKey::RightCtrl => "right_ctrl",
                ControlKey::LeftAlt => "left_alt",
            },
            match self.wrap {
                WrapMode::Native => "native",
                WrapMode::Window => "window",
            },
            self.scale,
            if self.key_click { "on" } else { "off" },
            self.click_volume,
            self.line_warning,
        )
    }

    pub fn save(&self, app_id: &str) -> Result<(), String> {
        let path = Self::path(app_id).ok_or("no config folder")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, self.to_text()).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let c = Config {
            control_key: ControlKey::LeftAlt,
            wrap: WrapMode::Window,
            scale: 2,
            key_click: false,
            click_volume: 0.5,
            line_warning: 0,
        };
        assert_eq!(Config::parse(&c.to_text()), c);
    }

    #[test]
    fn bad_values_use_defaults() {
        let c = Config::parse("control_key = shift\nscale = 99\nnonsense");
        assert_eq!(c.control_key, ControlKey::RightCtrl);
        assert_eq!(c.scale, MAX_SCALE);
    }
}
