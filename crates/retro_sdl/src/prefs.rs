//! The Preferences page: a small form drawn in the window.
//!
//! Changes apply immediately. They're written to the settings file by the
//! "Save Preferences" button, or on exit when "Save preferences on exit" is
//! ticked.

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas};
use sdl2::video::Window;

use crate::config::ControlKey;
use crate::menu::THEME;
use crate::ui_font::UiFont;

/// The controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Wrap,
    LineNumbers,
    Zoom,
    FgHue,
    FgLum,
    BgHue,
    BgLum,
    BrdHue,
    BrdLum,
    ResetColors,
    Charset,
    Click,
    CursorFlash,
    ControlAlt,
    ControlSide,
    WarnOn,
    WarnCol,
    AttractOn,
    AttractMin,
    DirMode,
    DirPath,
    Browse,
    SaveOnExit,
    Defaults,
    SaveNow,
    Close,
}

/// The controls in Tab order. Colour controls only appear when the
/// machine has a colour model.
fn order(colors: bool) -> Vec<Field> {
    use Field::*;
    let mut v = vec![Wrap, LineNumbers, Zoom];
    if colors {
        v.extend([FgHue, FgLum, BgHue, BgLum, BrdHue, BrdLum, ResetColors]);
    }
    v.extend([
        Charset,
        Click,
        CursorFlash,
        ControlAlt,
        ControlSide,
        WarnOn,
        WarnCol,
    ]);
    if colors {
        v.extend([AttractOn, AttractMin]);
    }
    v.extend([
        DirMode, DirPath, Browse, SaveOnExit, Defaults, SaveNow, Close,
    ]);
    v
}

pub use crate::widgets::Op;
use crate::widgets::Ui;

/// The values the page shows.
pub struct View<'a> {
    pub native: bool,
    pub native_label: String,
    pub line_numbers: bool,
    pub scale: u32,
    pub charsets: &'a [&'a str],
    pub charset: usize,
    pub key_click: bool,
    pub cursor_flash: bool,
    pub control_alt: bool,
    pub control_right: bool,
    pub warn_on: bool,
    pub warn_col: usize,
    pub dir_fixed: bool,
    pub dir_path: &'a str,
    pub save_on_exit: bool,
    pub colors: Option<ColorView>,
    pub attract_on: bool,
    pub attract_min: u32,
}

/// The text, background and border colour choices.
pub struct ColorView {
    /// (hue, luminance as displayed, colour, label such as "$94").
    pub text: (u8, u8, Color, String),
    pub background: (u8, u8, Color, String),
    pub border: (u8, u8, Color, String),
}

pub struct Prefs {
    order: Vec<Field>,
    /// Index into `order`.
    focus: usize,
    /// Clickable spots from the last draw.
    hits: Vec<(Rect, Field, Op)>,
}

impl Prefs {
    pub fn new(colors: bool) -> Self {
        Self {
            order: order(colors),
            focus: 0,
            hits: Vec::new(),
        }
    }

    pub fn focused(&self) -> Field {
        self.order[self.focus]
    }

    pub fn focus_on(&mut self, f: Field) {
        self.focus = self.order.iter().position(|&o| o == f).unwrap_or(0);
    }

    pub fn move_focus(&mut self, dir: isize) {
        let n = self.order.len() as isize;
        self.focus = (self.focus as isize + dir).rem_euclid(n) as usize;
    }

    pub fn hit(&self, x: i32, y: i32) -> Option<(Field, Op)> {
        self.hits
            .iter()
            .find(|(r, ..)| r.contains_point((x, y)))
            .map(|&(_, f, op)| (f, op))
    }

    pub fn draw(
        &mut self,
        canvas: &mut Canvas<Window>,
        font: &mut UiFont,
        v: &View,
        (width, height): (u32, u32),
        pad: i32,
    ) {
        let t = &THEME;
        canvas.set_blend_mode(BlendMode::BLEND);
        canvas.set_draw_color(Color::RGBA(0, 0, 0, 90));
        let _ = canvas.fill_rect(None);
        canvas.set_blend_mode(BlendMode::NONE);

        let lh = font.line_height();
        let row = row_height(lh, pad);
        let rows = rows(v.colors.is_some());
        let label_w = label_width(font, &rows);
        let (full_w, h) = page_size(font, pad, v);
        let w = full_w.min(width as i32 - pad * 4);
        let title_h = lh + pad * 2;
        let x0 = (width as i32 - w) / 2;
        let y0 = ((height as i32 - h) / 2).max(0);

        let frame = Rect::new(x0, y0, w as u32, h as u32);
        canvas.set_draw_color(Color::RGB(0xFF, 0xFF, 0xFF));
        let _ = canvas.fill_rect(frame);
        canvas.set_draw_color(t.bar);
        let _ = canvas.fill_rect(Rect::new(x0, y0, w as u32, title_h as u32));
        canvas.set_draw_color(t.border);
        let _ = canvas.draw_line((x0, y0 + title_h), (x0 + w - 1, y0 + title_h));
        let _ = canvas.draw_rect(frame);
        font.draw(canvas, x0 + pad * 4, y0 + pad, "Preferences", t.text);

        self.hits.clear();
        let focused = self.focused();
        let mut ui = Ui {
            canvas,
            font,
            pad,
            lh,
            focus: focused,
            hits: &mut self.hits,
            rings: Vec::new(),
            in_row: |f| matches!(f, Field::Browse | Field::ResetColors),
        };
        let lx = x0 + pad * 4;
        let cx = lx + label_w + pad * 6;
        let mut y = y0 + title_h + pad * 3;
        for (label, r) in &rows {
            ui.font.draw(ui.canvas, lx, y, label, t.text);
            match r {
                Row::Wrap => {
                    let end = ui.choice(
                        Field::Wrap,
                        cx,
                        y,
                        &[&v.native_label, "Window width"],
                        usize::from(!v.native),
                    );
                    ui.checkbox(
                        Field::LineNumbers,
                        end + pad * 8,
                        y,
                        LINE_NUMBERS,
                        v.line_numbers,
                    );
                }
                Row::Zoom => {
                    ui.spinner(Field::Zoom, cx, y, &format!("{}x", v.scale), 3);
                }
                Row::Text | Row::Background | Row::Border => {
                    let c = v.colors.as_ref().expect("row only shown with colours");
                    let (fields, (hue, lum, color, name)) = match r {
                        Row::Text => ((Field::FgHue, Field::FgLum), &c.text),
                        Row::Background => ((Field::BgHue, Field::BgLum), &c.background),
                        _ => ((Field::BrdHue, Field::BrdLum), &c.border),
                    };
                    let end = ui.color_row(cx, y, fields, *hue, *lum, *color, name);
                    if *r == Row::Border {
                        ui.button(Field::ResetColors, end + pad * 4, y, RESET_COLORS);
                    }
                }
                Row::Charset => {
                    ui.choice(Field::Charset, cx, y, v.charsets, v.charset);
                }
                Row::Click => {
                    ui.checkbox(Field::Click, cx, y, "On", v.key_click);
                }
                Row::Cursor => {
                    ui.checkbox(Field::CursorFlash, cx, y, "Flashing", v.cursor_flash);
                }
                Row::ControlKey => {
                    let end = ui.choice(
                        Field::ControlAlt,
                        cx,
                        y,
                        &control_modifiers(),
                        usize::from(v.control_alt),
                    );
                    ui.choice(
                        Field::ControlSide,
                        end + pad * 8,
                        y,
                        &control_sides(),
                        usize::from(v.control_right),
                    );
                }
                Row::Warn => {
                    let after = ui.checkbox(Field::WarnOn, cx, y, "On", v.warn_on);
                    let x = after + pad * 4;
                    let lw = ui.font.draw(ui.canvas, x, y, "after column", t.text);
                    let col = if v.warn_col == 0 {
                        String::new()
                    } else {
                        v.warn_col.to_string()
                    };
                    ui.spinner(Field::WarnCol, x + lw + pad * 2, y, &col, 4);
                }
                Row::Attract => {
                    let after = ui.checkbox(Field::AttractOn, cx, y, "On", v.attract_on);
                    let x = after + pad * 4;
                    let lw = ui.font.draw(ui.canvas, x, y, "after", t.text);
                    let n = if v.attract_min == 0 {
                        String::new()
                    } else {
                        v.attract_min.to_string()
                    };
                    let end = ui.spinner(Field::AttractMin, x + lw + pad * 2, y, &n, 2);
                    ui.font
                        .draw(ui.canvas, end + pad * 2, y, "minutes without input", t.text);
                }
                Row::Dir => {
                    ui.choice(
                        Field::DirMode,
                        cx,
                        y,
                        &["Last folder used", "This folder:"],
                        usize::from(v.dir_fixed),
                    );
                }
                Row::DirPath => {
                    let bw = ui.font.measure("Browse...") + pad * 4;
                    let fw = x0 + w - pad * 4 - cx - bw - pad * 2;
                    ui.text_field(Field::DirPath, cx, y, fw, v.dir_path);
                    ui.button(Field::Browse, cx + fw + pad * 2, y, "Browse...");
                }
                Row::Save => {
                    ui.checkbox(
                        Field::SaveOnExit,
                        cx,
                        y,
                        "Save preferences on exit",
                        v.save_on_exit,
                    );
                }
            }
            y += row;
        }

        // Buttons, right-aligned.
        let by = y0 + h - pad * 3 - (lh + pad * 2);
        let close_w = ui.font.measure("Close") + pad * 8;
        let save_w = ui.font.measure("Save Preferences") + pad * 4;
        let bx = x0 + w - pad * 4 - close_w;
        ui.button_w(Field::Close, bx, by, close_w, "Close");
        ui.button_w(
            Field::SaveNow,
            bx - pad * 2 - save_w,
            by,
            save_w,
            "Save Preferences",
        );
        let defaults_w = ui.font.measure("Defaults") + pad * 8;
        ui.button_w(Field::Defaults, lx, by, defaults_w, "Defaults");

        // `row_width` must agree with what was drawn, or controls would
        // spill past the page's edge.
        debug_assert!(
            full_w > w
                || ui
                    .hits
                    .iter()
                    .all(|(r, ..)| r.right() <= x0 + w - pad * 4 + 1),
            "Preferences control drawn past the page edge"
        );

        ui.draw_focus();
    }
}

#[derive(PartialEq)]
enum Row {
    Wrap,
    Zoom,
    Text,
    Background,
    Border,
    Charset,
    Click,
    Cursor,
    ControlKey,
    Warn,
    Attract,
    Dir,
    DirPath,
    Save,
}

/// The labelled rows of the form, top to bottom.
fn rows(colors: bool) -> Vec<(&'static str, Row)> {
    let mut rows = vec![("Display width", Row::Wrap), ("Zoom", Row::Zoom)];
    if colors {
        rows.push(("Text color", Row::Text));
        rows.push(("Background color", Row::Background));
        rows.push(("Border color", Row::Border));
    }
    rows.extend([
        ("Character set", Row::Charset),
        ("Key click", Row::Click),
        ("Cursor", Row::Cursor),
        ("CONTROL key", Row::ControlKey),
        ("Line length warning", Row::Warn),
    ]);
    if colors {
        rows.push(("Attract mode", Row::Attract));
    }
    rows.extend([
        ("Default folder", Row::Dir),
        ("", Row::DirPath),
        ("Preferences", Row::Save),
    ]);
    rows
}

/// Distance from one row to the next. Controls are a line high with half a
/// pad of breathing room above and below.
fn row_height(lh: i32, pad: i32) -> i32 {
    lh + pad + pad / 2
}

fn label_width(font: &mut UiFont, rows: &[(&str, Row)]) -> i32 {
    rows.iter().map(|(l, _)| font.measure(l)).max().unwrap_or(0)
}

const RESET_COLORS: &str = "Default";
const LINE_NUMBERS: &str = "Line numbers";

/// The CONTROL key choices: which modifier, and which side.
fn control_modifiers() -> [&'static str; 2] {
    [
        ControlKey::modifier_label(false),
        ControlKey::modifier_label(true),
    ]
}

fn control_sides() -> [&'static str; 2] {
    [ControlKey::side_label(false), ControlKey::side_label(true)]
}

/// The page's full size in pixels, before any clipping to the window:
/// wide enough for its widest row.
pub fn page_size(font: &mut UiFont, pad: i32, v: &View) -> (i32, i32) {
    let lh = font.line_height();
    let rows = rows(v.colors.is_some());
    let label_w = label_width(font, &rows);
    let widest = rows
        .iter()
        .map(|(_, r)| row_width(font, pad, v, r))
        .max()
        .unwrap_or(0);
    let buttons = font.measure("Defaults")
        + font.measure("Save Preferences")
        + font.measure("Close")
        + pad * 30;
    let w = (label_w + pad * 6 + widest).max(buttons) + pad * 8;
    let title_h = lh + pad * 2;
    // Title, rows, a small gap, the buttons and the bottom margin.
    let h = title_h + pad * 3 + row_height(lh, pad) * rows.len() as i32 + pad + lh + pad * 5;
    (w, h)
}

/// The width of a row's controls, matching what `Ui` draws.
fn row_width(font: &mut UiFont, p: i32, v: &View, row: &Row) -> i32 {
    let lh = font.line_height();
    let spinner =
        |font: &mut UiFont, chars: usize| lh * 2 + p * 5 + font.measure(&"0".repeat(chars));
    let checkbox = |font: &mut UiFont, label: &str| lh * 2 / 3 + p * 2 + font.measure(label);
    let choice = |font: &mut UiFont, options: &[&str]| {
        let r = lh / 3;
        options
            .iter()
            .map(|o| r * 2 + p + font.measure(o))
            .sum::<i32>()
            + p * 5 * (options.len() as i32 - 1).max(0)
    };
    let color = |font: &mut UiFont, name: &str| {
        lh * 2
            + p * 14
            + font.measure("Hue")
            + font.measure("Lum")
            + spinner(font, 2) * 2
            + font.measure(name)
    };
    match row {
        Row::Wrap => {
            choice(font, &[&v.native_label, "Window width"]) + p * 8 + checkbox(font, LINE_NUMBERS)
        }
        Row::Zoom => spinner(font, 3),
        Row::Text | Row::Background | Row::Border => {
            let Some(c) = &v.colors else { return 0 };
            let name = match row {
                Row::Text => &c.text.3,
                Row::Background => &c.background.3,
                _ => &c.border.3,
            };
            let mut w = color(font, name);
            if *row == Row::Border {
                w += p * 8 + font.measure(RESET_COLORS);
            }
            w
        }
        Row::Charset => choice(font, v.charsets),
        Row::Click => checkbox(font, "On"),
        Row::Cursor => checkbox(font, "Flashing"),
        Row::ControlKey => {
            choice(font, &control_modifiers()) + p * 8 + choice(font, &control_sides())
        }
        Row::Warn => checkbox(font, "On") + p * 6 + font.measure("after column") + spinner(font, 4),
        Row::Attract => {
            checkbox(font, "On")
                + p * 8
                + font.measure("after")
                + spinner(font, 2)
                + font.measure("minutes without input")
        }
        Row::Dir => choice(font, &["Last folder used", "This folder:"]),
        // The path field stretches to fill whatever room there is.
        Row::DirPath => font.measure("Browse...") + p * 6 + lh * 8,
        Row::Save => checkbox(font, "Save preferences on exit"),
    }
}
