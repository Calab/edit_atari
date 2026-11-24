//! User settings, stored as `key = value` lines in the OS config folder:
//!
//! * Linux:   `$XDG_CONFIG_HOME/<app>/settings.txt` (or `~/.config/...`)
//! * Windows: `%APPDATA%\<app>\settings.txt`
//! * macOS:   `~/Library/Application Support/<app>/settings.txt`

use std::path::PathBuf;

use retro_core::WrapMode;
use retro_core::print::Paper;

/// Which PC key acts as the machine's CONTROL key: Ctrl or Alt (on a
/// Mac, control or Option), on the left or the right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlKey {
    pub alt: bool,
    pub right: bool,
}

impl ControlKey {
    /// Right Ctrl on Windows and Linux (left Ctrl is for the editor's
    /// shortcuts); on a Mac, whose shortcuts use Command, the left control
    /// key (laptops have no right one).
    pub const DEFAULT: ControlKey = ControlKey {
        alt: false,
        right: !cfg!(target_os = "macos"),
    };

    /// "CTRL" or "ALT" ("CONTROL" or "OPTION" on a Mac).
    pub fn modifier_label(alt: bool) -> &'static str {
        match (alt, cfg!(target_os = "macos")) {
            (false, false) => "CTRL",
            (true, false) => "ALT",
            (false, true) => "CONTROL",
            (true, true) => "OPTION",
        }
    }

    pub fn side_label(right: bool) -> &'static str {
        if right { "Right" } else { "Left" }
    }

    /// E.g. "RIGHT-CTRL".
    pub fn label(self) -> String {
        format!(
            "{}-{}",
            Self::side_label(self.right).to_uppercase(),
            Self::modifier_label(self.alt)
        )
    }
}

/// Where Open and Save As dialogs start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirMode {
    /// The folder last used in a file dialog.
    Last,
    /// Always `Config::default_dir`.
    Fixed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub control_key: ControlKey,
    pub wrap: WrapMode,
    /// Pixels per font pixel.
    pub scale: u32,
    pub key_click: bool,
    pub click_volume: f32,
    /// Draw characters past `line_warning` in the warning colour.
    pub line_warning_on: bool,
    /// The last column allowed before the warning (1-based count).
    pub line_warning: usize,
    /// Character set index (0 = the machine's standard set).
    pub charset: usize,
    pub dir_mode: DirMode,
    /// The fixed starting folder (`DirMode::Fixed`).
    pub default_dir: String,
    /// The folder last used in a file dialog (`DirMode::Last`).
    pub last_dir: String,
    /// Write the settings file when the program exits.
    pub save_on_exit: bool,
    /// Text and background colours as (hue, luminance); `None` = default.
    pub text_color: Option<(u8, u8)>,
    pub background_color: Option<(u8, u8)>,
    pub border_color: Option<(u8, u8)>,
    /// Flash the cursor (the Atari's doesn't).
    pub cursor_flash: bool,
    /// Show editor line numbers in a column left of the text.
    pub line_numbers: bool,
    /// The font file last loaded for each character set (empty: none
    /// yet), kept while the built-in set is in use.
    pub fonts: [String; 2],
    /// Use the built-in character set rather than the font file.
    pub font_builtin: [bool; 2],
    /// Cycle dim colours after `attract_minutes` without input.
    pub attract_on: bool,
    pub attract_minutes: u32,
    /// File > Print: paper, rows as wide as the page (else 40 characters),
    /// character size in points, and the page header.
    pub print_paper: Paper,
    pub print_fill: bool,
    pub print_size: u32,
    pub print_header: bool,
    /// Open the PDF in the system's viewer after Print to PDF saves it.
    pub print_open: bool,
    /// Print editor line numbers left of the text.
    pub print_line_numbers: bool,
    /// Go to Line counts lines in the editor, not by program line number.
    pub goto_editor_line: bool,
    /// Files opened or saved lately, most recent first.
    pub recent: Vec<String>,
    /// The window's size when it was last closed, in desktop units (not
    /// maximized; `None` = work it out from the zoom), and whether it was
    /// maximized. Kept like `recent`, whatever `save_on_exit` says.
    pub window_size: Option<(u32, u32)>,
    pub window_maximized: bool,
}

/// How many recent files the File menu lists.
pub const MAX_RECENT: usize = 6;

impl Config {
    /// The warning column, or `None` when the warning is off.
    pub fn warning_column(&self) -> Option<usize> {
        (self.line_warning_on && self.line_warning > 0).then_some(self.line_warning)
    }

    /// The folder file dialogs should start in, if it exists.
    pub fn start_dir(&self) -> Option<PathBuf> {
        let d = match self.dir_mode {
            DirMode::Last => &self.last_dir,
            DirMode::Fixed => &self.default_dir,
        };
        let p = PathBuf::from(d);
        (!d.is_empty() && p.is_dir()).then_some(p)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            control_key: ControlKey::DEFAULT,
            wrap: WrapMode::Window,
            scale: 2,
            key_click: true,
            click_volume: 0.25,
            line_warning_on: true,
            line_warning: DEFAULT_LINE_WARNING,
            charset: 0,
            dir_mode: DirMode::Last,
            default_dir: String::new(),
            last_dir: String::new(),
            save_on_exit: true,
            text_color: None,
            background_color: None,
            border_color: None,
            cursor_flash: false,
            line_numbers: false,
            fonts: Default::default(),
            font_builtin: [true, true],
            attract_on: true,
            attract_minutes: DEFAULT_ATTRACT_MINUTES,
            print_paper: Paper::Letter,
            print_fill: true,
            print_size: DEFAULT_PRINT_SIZE,
            print_header: true,
            print_open: true,
            print_line_numbers: false,
            goto_editor_line: true,
            recent: Vec::new(),
            window_size: None,
            window_maximized: false,
        }
    }
}

pub const MIN_SCALE: u32 = 1;
pub const MAX_SCALE: u32 = 8;
/// The Atari screen editor's logical line: 3 rows of 38 characters.
pub const DEFAULT_LINE_WARNING: usize = 114;
pub const MAX_LINE_WARNING: usize = 9999;
pub const DEFAULT_ATTRACT_MINUTES: u32 = 5;
pub const MAX_ATTRACT_MINUTES: u32 = 60;
/// Print character sizes, in points.
pub const DEFAULT_PRINT_SIZE: u32 = 12;
pub const MIN_PRINT_SIZE: u32 = 5;
pub const MAX_PRINT_SIZE: u32 = 24;

/// The smallest window size kept in the settings, so a damaged file
/// can't open a window too small to use (the minimum size applies too).
const MIN_WINDOW: u32 = 200;
const MAX_WINDOW: u32 = 20_000;

/// "1280x800" -> (1280, 800).
fn parse_size(v: &str) -> Option<(u32, u32)> {
    let (w, h) = v.split_once('x')?;
    let (w, h): (u32, u32) = (w.trim().parse().ok()?, h.trim().parse().ok()?);
    let ok = |n: u32| (MIN_WINDOW..=MAX_WINDOW).contains(&n);
    (ok(w) && ok(h)).then_some((w, h))
}

/// "9,2" -> (9, 2).
fn parse_color(v: &str) -> Option<(u8, u8)> {
    let (h, l) = v.split_once(',')?;
    Some((h.trim().parse().ok()?, l.trim().parse().ok()?))
}

fn color_text(c: Option<(u8, u8)>) -> String {
    c.map(|(h, l)| format!("{h},{l}")).unwrap_or_default()
}

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
                // Before 1.2: right_ctrl or left_alt. On a Mac right_ctrl
                // meant the control key, which laptops have on the left.
                "control_key" => match v {
                    "right_ctrl" => c.control_key = ControlKey::DEFAULT,
                    "left_alt" => {
                        c.control_key = ControlKey {
                            alt: true,
                            right: false,
                        }
                    }
                    _ => {}
                },
                "control_key_modifier" => match v {
                    "ctrl" => c.control_key.alt = false,
                    "alt" => c.control_key.alt = true,
                    _ => {}
                },
                "control_key_side" => match v {
                    "left" => c.control_key.right = false,
                    "right" => c.control_key.right = true,
                    _ => {}
                },
                "cursor_flash" => c.cursor_flash = v == "on",
                "line_numbers" => c.line_numbers = v == "on",
                // A font file means it's in use, unless the line after
                // says built-in (settings from before that line existed
                // had no other way to say so).
                "font" => {
                    c.fonts[0] = v.to_string();
                    c.font_builtin[0] = v.is_empty();
                }
                "font_international" => {
                    c.fonts[1] = v.to_string();
                    c.font_builtin[1] = v.is_empty();
                }
                "font_builtin" => c.font_builtin[0] = v != "off",
                "font_international_builtin" => c.font_builtin[1] = v != "off",
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
                "charset" => {
                    if let Ok(n) = v.parse::<usize>() {
                        c.charset = n.min(15);
                    }
                }
                "line_warning" => {
                    if let Ok(n) = v.parse::<usize>() {
                        // Older files used 0 for "off".
                        if n == 0 {
                            c.line_warning_on = false;
                        } else {
                            c.line_warning = n.min(MAX_LINE_WARNING);
                        }
                    }
                }
                "line_warning_on" => c.line_warning_on = v != "off",
                "dir_mode" => match v {
                    "last" => c.dir_mode = DirMode::Last,
                    "fixed" => c.dir_mode = DirMode::Fixed,
                    _ => {}
                },
                "default_dir" => c.default_dir = v.to_string(),
                "last_dir" => c.last_dir = v.to_string(),
                "save_on_exit" => c.save_on_exit = v != "off",
                "text_color" => c.text_color = parse_color(v),
                "background_color" => c.background_color = parse_color(v),
                "border_color" => c.border_color = parse_color(v),
                "attract" => c.attract_on = v != "off",
                "attract_minutes" => {
                    if let Ok(n) = v.parse::<u32>() {
                        c.attract_minutes = n.clamp(1, MAX_ATTRACT_MINUTES);
                    }
                }
                "print_paper" => match v {
                    "letter" => c.print_paper = Paper::Letter,
                    "a4" => c.print_paper = Paper::A4,
                    _ => {}
                },
                "print_width" => c.print_fill = v == "page",
                "print_size" => {
                    if let Ok(n) = v.parse::<u32>() {
                        c.print_size = n.clamp(MIN_PRINT_SIZE, MAX_PRINT_SIZE);
                    }
                }
                "print_header" => c.print_header = v != "off",
                "print_open" => c.print_open = v != "off",
                "print_line_numbers" => c.print_line_numbers = v == "on",
                "goto_editor_line" => c.goto_editor_line = v != "off",
                "recent" => {
                    if !v.is_empty() && c.recent.len() < MAX_RECENT {
                        c.recent.push(v.to_string());
                    }
                }
                "window_size" => c.window_size = parse_size(v),
                "window_maximized" => c.window_maximized = v == "on",
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
             # The Atari CONTROL key: ctrl | alt, on the left | right\n\
             control_key_modifier = {}\n\
             control_key_side = {}\n\
             cursor_flash = {}\n\
             line_numbers = {}\n\
             # Font files (.FNT) for the standard and international sets\n\
             font = {}\n\
             font_builtin = {}\n\
             font_international = {}\n\
             font_international_builtin = {}\n\
             # wrap: native (40 columns) | window\n\
             wrap = {}\n\
             scale = {}\n\
             key_click = {}\n\
             click_volume = {}\n\
             # Highlight characters past line_warning\n\
             line_warning_on = {}\n\
             line_warning = {}\n\
             # charset: 0 = standard (graphics), 1 = international (Atari)\n\
             charset = {}\n\
             # dir_mode: last (folder last used) | fixed (default_dir)\n\
             dir_mode = {}\n\
             default_dir = {}\n\
             last_dir = {}\n\
             save_on_exit = {}\n\
             # Colours as hue,luminance-level (empty = the machine's default)\n\
             text_color = {}\n\
             background_color = {}\n\
             border_color = {}\n\
             attract = {}\n\
             attract_minutes = {}\n\
             # File > Print: paper letter | a4; width 40 | page; size in points\n\
             print_paper = {}\n\
             print_width = {}\n\
             print_size = {}\n\
             print_header = {}\n\
             print_open = {}\n\
             print_line_numbers = {}\n\
             # Go to Line: on = line in the editor, off = program line number\n\
             goto_editor_line = {}\n\
             # The window when last closed: width x height (empty = from the zoom)\n\
             window_size = {}\n\
             window_maximized = {}\n\
             # Recent files, most recent first\n\
             {}",
            if self.control_key.alt { "alt" } else { "ctrl" },
            if self.control_key.right {
                "right"
            } else {
                "left"
            },
            on_off(self.cursor_flash),
            on_off(self.line_numbers),
            self.fonts[0],
            on_off(self.font_builtin[0]),
            self.fonts[1],
            on_off(self.font_builtin[1]),
            match self.wrap {
                WrapMode::Native => "native",
                WrapMode::Window => "window",
            },
            self.scale,
            on_off(self.key_click),
            self.click_volume,
            on_off(self.line_warning_on),
            self.line_warning,
            self.charset,
            match self.dir_mode {
                DirMode::Last => "last",
                DirMode::Fixed => "fixed",
            },
            self.default_dir,
            self.last_dir,
            on_off(self.save_on_exit),
            color_text(self.text_color),
            color_text(self.background_color),
            color_text(self.border_color),
            on_off(self.attract_on),
            self.attract_minutes,
            match self.print_paper {
                Paper::Letter => "letter",
                Paper::A4 => "a4",
            },
            if self.print_fill { "page" } else { "40" },
            self.print_size,
            on_off(self.print_header),
            on_off(self.print_open),
            on_off(self.print_line_numbers),
            on_off(self.goto_editor_line),
            self.window_size
                .map(|(w, h)| format!("{w}x{h}"))
                .unwrap_or_default(),
            on_off(self.window_maximized),
            self.recent
                .iter()
                .map(|p| format!("recent = {p}\n"))
                .collect::<String>(),
        )
    }

    /// The settings to write for a window that started with `initial` and
    /// now has `self`, when the file now holds `on_disk` (another window
    /// may have saved since): the settings this window changed, over the
    /// ones on disk. `opened` are the files this window opened or saved,
    /// most recent first; they go before the others in the recent list.
    /// The window size is always this window's: the last one closed wins.
    pub fn merged(&self, initial: &Config, on_disk: Config, opened: &[String]) -> Config {
        let mut out = on_disk;
        // Every field, so a new one can't be forgotten here.
        let Config {
            control_key,
            wrap,
            scale,
            key_click,
            click_volume,
            line_warning_on,
            line_warning,
            charset,
            dir_mode,
            default_dir,
            last_dir,
            save_on_exit,
            text_color,
            background_color,
            border_color,
            cursor_flash,
            line_numbers,
            fonts,
            font_builtin,
            attract_on,
            attract_minutes,
            print_paper,
            print_fill,
            print_size,
            print_header,
            print_open,
            print_line_numbers,
            goto_editor_line,
            recent: _,
            window_size,
            window_maximized,
        } = self;
        macro_rules! changed {
            ($($f:ident),*) => {
                $(if *$f != initial.$f {
                    out.$f = $f.clone();
                })*
            };
        }
        changed!(
            control_key,
            wrap,
            scale,
            key_click,
            click_volume,
            line_warning_on,
            line_warning,
            charset,
            dir_mode,
            default_dir,
            last_dir,
            save_on_exit,
            text_color,
            background_color,
            border_color,
            cursor_flash,
            line_numbers,
            fonts,
            font_builtin,
            attract_on,
            attract_minutes,
            print_paper,
            print_fill,
            print_size,
            print_header,
            print_open,
            print_line_numbers,
            goto_editor_line
        );
        let mut recent: Vec<String> = opened.to_vec();
        for p in std::mem::take(&mut out.recent) {
            if !recent.contains(&p) {
                recent.push(p);
            }
        }
        recent.truncate(MAX_RECENT);
        out.recent = recent;
        out.window_size = *window_size;
        out.window_maximized = *window_maximized;
        out
    }

    pub fn save(&self, app_id: &str) -> Result<(), String> {
        let path = Self::path(app_id).ok_or("no config folder")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, self.to_text()).map_err(|e| e.to_string())
    }
}

fn on_off(b: bool) -> &'static str {
    if b { "on" } else { "off" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let c = Config {
            control_key: ControlKey {
                alt: true,
                right: true,
            },
            cursor_flash: true,
            line_numbers: true,
            fonts: ["/home/me/BIG.FNT".into(), "/home/me/X.FNT".into()],
            font_builtin: [false, true],
            wrap: WrapMode::Window,
            scale: 2,
            key_click: false,
            click_volume: 0.5,
            line_warning_on: false,
            line_warning: 80,
            charset: 1,
            dir_mode: DirMode::Fixed,
            default_dir: "/home/me/atari files".into(),
            last_dir: "C:\\Atari".into(),
            save_on_exit: false,
            text_color: Some((12, 7)),
            background_color: None,
            border_color: Some((3, 4)),
            attract_on: false,
            attract_minutes: 9,
            print_paper: Paper::A4,
            print_fill: true,
            print_size: 8,
            print_header: false,
            print_open: false,
            print_line_numbers: true,
            goto_editor_line: true,
            recent: vec!["/home/me/PROG.BAS".into(), "C:\\Atari\\GAME.TUR".into()],
            window_size: Some((1280, 800)),
            window_maximized: true,
        };
        assert_eq!(Config::parse(&c.to_text()), c);
    }

    #[test]
    fn merging_keeps_other_windows_changes() {
        let initial = Config::default();
        // Another window changed the zoom and opened a file since.
        let on_disk = Config {
            scale: 4,
            recent: vec!["B.BAS".into(), "A.BAS".into()],
            ..Config::default()
        };
        // This one only turned the key click off, and opened C.BAS.
        let mine = Config {
            key_click: false,
            ..Config::default()
        };
        let m = mine.merged(&initial, on_disk, &["C.BAS".into(), "A.BAS".into()]);
        assert_eq!(m.scale, 4);
        assert!(!m.key_click);
        assert_eq!(m.recent, ["C.BAS", "A.BAS", "B.BAS"]);
    }

    #[test]
    fn window_size_is_the_last_window_closed() {
        let initial = Config::default();
        let on_disk = Config {
            window_size: Some((900, 700)),
            ..Config::default()
        };
        let mine = Config {
            window_size: Some((1280, 800)),
            window_maximized: true,
            ..Config::default()
        };
        let m = mine.merged(&initial, on_disk, &[]);
        assert_eq!(m.window_size, Some((1280, 800)));
        assert!(m.window_maximized);
    }

    #[test]
    fn bad_window_sizes_are_ignored() {
        for v in [
            "",
            "1280",
            "1280x",
            "x800",
            "0x0",
            "50x50",
            "1280x99999",
            "axb",
        ] {
            assert_eq!(
                Config::parse(&format!("window_size = {v}")).window_size,
                None,
                "{v}"
            );
        }
        assert_eq!(
            Config::parse("window_size = 1280 x 800").window_size,
            Some((1280, 800))
        );
    }

    #[test]
    fn old_control_key_settings_load() {
        let c = Config::parse("control_key = left_alt");
        assert_eq!(
            c.control_key,
            ControlKey {
                alt: true,
                right: false
            }
        );
        assert_eq!(
            Config::parse("control_key = right_ctrl").control_key,
            ControlKey::DEFAULT
        );
    }

    #[test]
    fn older_font_settings_mean_in_use() {
        let c = Config::parse("font = /f/BIG.FNT");
        assert_eq!(c.font_builtin, [false, true]);
    }

    #[test]
    fn defaults() {
        let c = Config::default();
        // Printing: Letter, page width, 12 point, header, open after saving.
        assert_eq!(c.print_paper, Paper::Letter);
        assert!(c.print_fill && c.print_header && c.print_open);
        assert_eq!(c.print_size, 12);
        // Preferences.
        assert_eq!(c.scale, 2);
        assert_eq!(
            (c.text_color, c.background_color, c.border_color),
            (None, None, None)
        );
        assert_eq!(c.charset, 0);
        assert_eq!(c.wrap, WrapMode::Window);
        assert!(c.key_click);
        assert!(!c.control_key.alt);
        assert_eq!(c.control_key.right, !cfg!(target_os = "macos"));
        assert!(c.line_warning_on);
        assert_eq!(c.line_warning, 114);
        assert!(c.attract_on);
        assert_eq!(c.attract_minutes, 5);
        assert_eq!(c.dir_mode, DirMode::Last);
        assert!(c.save_on_exit);
        // Go to Line counts editor lines.
        assert!(c.goto_editor_line);
        // Fonts: the built-in ones, no files.
        assert_eq!(c.fonts, [String::new(), String::new()]);
        assert_eq!(c.font_builtin, [true, true]);
    }

    #[test]
    fn bad_values_use_defaults() {
        let c = Config::parse("control_key = shift\nscale = 99\nnonsense");
        assert_eq!(c.control_key, ControlKey::DEFAULT);
        assert_eq!(c.scale, MAX_SCALE);
    }

    #[test]
    fn old_zero_warning_means_off() {
        let c = Config::parse("line_warning = 0");
        assert!(!c.line_warning_on);
        assert_eq!(c.line_warning, DEFAULT_LINE_WARNING);
        assert_eq!(c.warning_column(), None);
    }
}
