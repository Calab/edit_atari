//! The Preferences page: a small form drawn in the window.
//!
//! Changes apply immediately. They're written to the settings file by the
//! "Save Preferences" button, or on exit when "Save preferences on exit" is
//! ticked.

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas};
use sdl2::video::Window;

use crate::menu::THEME;
use crate::ui_font::UiFont;

/// The controls, in Tab order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Wrap,
    Zoom,
    Charset,
    Click,
    ControlKey,
    WarnOn,
    WarnCol,
    DirMode,
    DirPath,
    Browse,
    SaveOnExit,
    SaveNow,
    Close,
}

pub const ORDER: [Field; 13] = [
    Field::Wrap,
    Field::Zoom,
    Field::Charset,
    Field::Click,
    Field::ControlKey,
    Field::WarnOn,
    Field::WarnCol,
    Field::DirMode,
    Field::DirPath,
    Field::Browse,
    Field::SaveOnExit,
    Field::SaveNow,
    Field::Close,
];

/// What clicking a spot does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// Pick option `n` of a choice.
    Choose(usize),
    Toggle,
    Dec,
    Inc,
    Press,
    /// Just move the focus there (text fields).
    Focus,
}

/// The values the page shows.
pub struct View<'a> {
    pub native: bool,
    pub native_label: String,
    pub scale: u32,
    pub charsets: &'a [&'a str],
    pub charset: usize,
    pub key_click: bool,
    pub left_alt: bool,
    pub warn_on: bool,
    pub warn_col: usize,
    pub dir_fixed: bool,
    pub dir_path: &'a str,
    pub save_on_exit: bool,
}

#[derive(Default)]
pub struct Prefs {
    /// Index into [`ORDER`].
    pub focus: usize,
    /// Clickable spots from the last draw.
    hits: Vec<(Rect, Field, Op)>,
}

impl Prefs {
    pub fn focused(&self) -> Field {
        ORDER[self.focus]
    }

    pub fn focus_on(&mut self, f: Field) {
        self.focus = ORDER.iter().position(|&o| o == f).unwrap_or(0);
    }

    pub fn move_focus(&mut self, dir: isize) {
        let n = ORDER.len() as isize;
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
        let row = lh + pad * 3;
        let labels = [
            "Display width",
            "Zoom",
            "Character set",
            "Key click",
            "CONTROL key",
            "Line length warning",
            "Default folder",
            "",
            "Preferences",
        ];
        let label_w = labels.iter().map(|l| font.measure(l)).max().unwrap_or(0);
        let w = (label_w + pad * 90).min(width as i32 - pad * 4);
        let title_h = lh + pad * 2;
        let h = title_h + pad * 2 + row * labels.len() as i32 + pad * 3 + lh + pad * 6;
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
        };
        let lx = x0 + pad * 4;
        let cx = lx + label_w + pad * 6;
        let mut y = y0 + title_h + pad * 3;
        for (i, label) in labels.iter().enumerate() {
            ui.font.draw(ui.canvas, lx, y, label, t.text);
            match i {
                0 => {
                    ui.choice(
                        Field::Wrap,
                        cx,
                        y,
                        &[&v.native_label, "Window width"],
                        usize::from(!v.native),
                    );
                }
                1 => {
                    ui.spinner(Field::Zoom, cx, y, &format!("{}x", v.scale), 3);
                }
                2 => {
                    ui.choice(Field::Charset, cx, y, v.charsets, v.charset);
                }
                3 => {
                    ui.checkbox(Field::Click, cx, y, "On", v.key_click);
                }
                4 => {
                    ui.choice(
                        Field::ControlKey,
                        cx,
                        y,
                        &["RIGHT-CTRL", "LEFT-ALT"],
                        usize::from(v.left_alt),
                    );
                }
                5 => {
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
                6 => {
                    ui.choice(
                        Field::DirMode,
                        cx,
                        y,
                        &["Last folder used", "This folder:"],
                        usize::from(v.dir_fixed),
                    );
                }
                7 => {
                    let bw = ui.font.measure("Browse...") + pad * 4;
                    let fw = x0 + w - pad * 4 - cx - bw - pad * 2;
                    ui.text_field(Field::DirPath, cx, y, fw, v.dir_path);
                    ui.button(Field::Browse, cx + fw + pad * 2, y, "Browse...");
                }
                8 => {
                    ui.checkbox(
                        Field::SaveOnExit,
                        cx,
                        y,
                        "Save preferences on exit",
                        v.save_on_exit,
                    );
                }
                _ => {}
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

        // Focus ring around the focused control.
        let rings = std::mem::take(&mut ui.rings);
        for (f, r) in rings {
            if f == focused {
                ui.canvas.set_draw_color(t.highlight);
                let _ = ui.canvas.draw_rect(r);
                let _ = ui.canvas.draw_rect(Rect::new(
                    r.x() + 1,
                    r.y() + 1,
                    r.width().saturating_sub(2),
                    r.height().saturating_sub(2),
                ));
            }
        }
    }
}

/// Drawing helpers for the form's controls.
struct Ui<'a, 'c, 't> {
    canvas: &'c mut Canvas<Window>,
    font: &'c mut UiFont<'t>,
    pad: i32,
    lh: i32,
    focus: Field,
    hits: &'a mut Vec<(Rect, Field, Op)>,
    /// Each control's outline, for the focus ring.
    rings: Vec<(Field, Rect)>,
}

impl Ui<'_, '_, '_> {
    fn ring(&mut self, f: Field, x0: i32, y: i32, x1: i32) {
        let p = self.pad;
        self.rings.push((
            f,
            Rect::new(
                x0 - p,
                y - p,
                (x1 - x0 + p * 2) as u32,
                (self.lh + p * 2) as u32,
            ),
        ));
    }

    fn circle(&mut self, cx: i32, cy: i32, r: i32, color: Color) {
        self.canvas.set_draw_color(color);
        for dy in -r..=r {
            let dx = ((r * r - dy * dy) as f32).sqrt() as i32;
            let _ = self
                .canvas
                .draw_line((cx - dx, cy + dy), (cx + dx, cy + dy));
        }
    }

    /// Radio buttons in a row. Returns the x after the last one.
    fn choice(&mut self, f: Field, x: i32, y: i32, options: &[&str], selected: usize) -> i32 {
        let t = &THEME;
        let r = self.lh / 3;
        let mut cx = x;
        for (i, label) in options.iter().enumerate() {
            let mid = y + self.lh / 2;
            self.circle(cx + r, mid, r, t.border);
            self.circle(cx + r, mid, r - 1, Color::RGB(0xFF, 0xFF, 0xFF));
            if i == selected {
                self.circle(cx + r, mid, r / 2 + 1, t.highlight);
            }
            let lw = self
                .font
                .draw(self.canvas, cx + r * 2 + self.pad, y, label, t.text);
            let end = cx + r * 2 + self.pad + lw;
            self.hits.push((
                Rect::new(cx, y, (end - cx) as u32, self.lh as u32),
                f,
                Op::Choose(i),
            ));
            cx = end + self.pad * 5;
        }
        let end = cx - self.pad * 5;
        self.ring(f, x, y, end);
        end
    }

    /// A check box with a label. Returns the x after the label.
    fn checkbox(&mut self, f: Field, x: i32, y: i32, label: &str, on: bool) -> i32 {
        let t = &THEME;
        let s = self.lh * 2 / 3;
        let by = y + (self.lh - s) / 2;
        let b = Rect::new(x, by, s as u32, s as u32);
        self.canvas.set_draw_color(if on {
            t.highlight
        } else {
            Color::RGB(0xFF, 0xFF, 0xFF)
        });
        let _ = self.canvas.fill_rect(b);
        self.canvas
            .set_draw_color(if on { t.highlight } else { t.border });
        let _ = self.canvas.draw_rect(b);
        if on {
            let u = (s / 6).max(2);
            let (cx, cy) = (x + s / 4, by + s / 2);
            self.canvas.set_draw_color(Color::RGB(0xFF, 0xFF, 0xFF));
            for d in 0..(u / 2).max(1) {
                let _ = self.canvas.draw_line((cx, cy + d), (cx + u, cy + u + d));
                let _ = self
                    .canvas
                    .draw_line((cx + u, cy + u + d), (cx + u * 3, cy - u + d));
            }
        }
        let lw = self
            .font
            .draw(self.canvas, x + s + self.pad * 2, y, label, t.text);
        let end = x + s + self.pad * 2 + lw;
        self.hits.push((
            Rect::new(x, y, (end - x) as u32, self.lh as u32),
            f,
            Op::Toggle,
        ));
        self.ring(f, x, y, end);
        end
    }

    fn small_button(&mut self, f: Field, op: Op, x: i32, y: i32, label: &str) -> i32 {
        let t = &THEME;
        let w = self.lh;
        let r = Rect::new(x, y, w as u32, self.lh as u32);
        self.canvas.set_draw_color(t.bar);
        let _ = self.canvas.fill_rect(r);
        self.canvas.set_draw_color(t.border);
        let _ = self.canvas.draw_rect(r);
        let lw = self.font.measure(label);
        self.font
            .draw(self.canvas, x + (w - lw) / 2, y, label, t.text);
        self.hits.push((r, f, op));
        x + w
    }

    /// [-] value [+], the value box `chars` digits wide.
    fn spinner(&mut self, f: Field, x: i32, y: i32, value: &str, chars: usize) {
        let t = &THEME;
        let after_minus = self.small_button(f, Op::Dec, x, y, "-");
        let vw = self.font.measure(&"0".repeat(chars)) + self.pad * 3;
        let r = Rect::new(after_minus + self.pad, y, vw as u32, self.lh as u32);
        self.canvas.set_draw_color(Color::RGB(0xFF, 0xFF, 0xFF));
        let _ = self.canvas.fill_rect(r);
        self.canvas.set_draw_color(t.border);
        let _ = self.canvas.draw_rect(r);
        let tw = self.font.measure(value);
        self.font
            .draw(self.canvas, r.x() + (vw - tw) / 2, y, value, t.text);
        self.hits.push((r, f, Op::Focus));
        let end = self.small_button(f, Op::Inc, r.x() + vw + self.pad, y, "+");
        self.ring(f, x, y, end);
    }

    fn text_field(&mut self, f: Field, x: i32, y: i32, w: i32, text: &str) {
        let t = &THEME;
        let r = Rect::new(x, y, w.max(10) as u32, self.lh as u32);
        self.canvas.set_draw_color(Color::RGB(0xFF, 0xFF, 0xFF));
        let _ = self.canvas.fill_rect(r);
        self.canvas.set_draw_color(t.border);
        let _ = self.canvas.draw_rect(r);
        // Show the end of a long path, which is the informative part.
        let mut shown = text.to_string();
        let room = w - self.pad * 2;
        while !shown.is_empty() && self.font.measure(&shown) > room {
            shown.remove(0);
        }
        let tw = self.font.draw(self.canvas, x + self.pad, y, &shown, t.text);
        if self.focus == f {
            self.canvas.set_draw_color(t.text);
            let cx = x + self.pad + tw + 1;
            let _ = self.canvas.draw_line((cx, y + 2), (cx, y + self.lh - 3));
        }
        self.hits.push((r, f, Op::Focus));
        self.ring(f, x, y, x + w);
    }

    fn button(&mut self, f: Field, x: i32, y: i32, label: &str) {
        let w = self.font.measure(label) + self.pad * 4;
        self.button_w(f, x, y, w, label);
    }

    fn button_w(&mut self, f: Field, x: i32, y: i32, w: i32, label: &str) {
        let t = &THEME;
        let h = self.lh + self.pad * 2;
        let y = y - if f == Field::Browse { self.pad } else { 0 };
        let h = if f == Field::Browse {
            self.lh + self.pad * 2
        } else {
            h
        };
        let r = Rect::new(x, y, w as u32, h as u32);
        let focused = self.focus == f;
        self.canvas
            .set_draw_color(if focused { t.highlight } else { t.bar });
        let _ = self.canvas.fill_rect(r);
        self.canvas
            .set_draw_color(if focused { t.highlight } else { t.border });
        let _ = self.canvas.draw_rect(r);
        let lw = self.font.measure(label);
        let color = if focused { t.highlight_text } else { t.text };
        self.font.draw(
            self.canvas,
            x + (w - lw) / 2,
            y + (h - self.lh) / 2,
            label,
            color,
        );
        self.hits.push((r, f, Op::Press));
    }
}
