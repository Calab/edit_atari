//! Form controls shared by the Preferences and Print pages: radio
//! buttons, check boxes, spinners, text fields and buttons, with a focus
//! ring and click targets.

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::video::Window;

use crate::menu::THEME;
use crate::ui_font::UiFont;

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

/// Drawing helpers for the form's controls.
pub struct Ui<'a, 'c, 't, F> {
    pub canvas: &'c mut Canvas<Window>,
    pub font: &'c mut UiFont<'t>,
    pub pad: i32,
    pub lh: i32,
    pub focus: F,
    pub hits: &'a mut Vec<(Rect, F, Op)>,
    /// Each control's outline, for the focus ring.
    pub rings: Vec<(F, Rect)>,
    /// Buttons that sit in a row of the form (a line high), rather than at
    /// the bottom.
    pub in_row: fn(F) -> bool,
}

impl<F: Copy + PartialEq> Ui<'_, '_, '_, F> {
    fn ring(&mut self, f: F, x0: i32, y: i32, x1: i32) {
        let p = self.pad;
        self.rings.push((
            f,
            Rect::new(
                x0 - p,
                y - p / 2,
                (x1 - x0 + p * 2) as u32,
                (self.lh + p) as u32,
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
    pub fn choice(&mut self, f: F, x: i32, y: i32, options: &[&str], selected: usize) -> i32 {
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
    pub fn checkbox(&mut self, f: F, x: i32, y: i32, label: &str, on: bool) -> i32 {
        self.checkbox_if(f, x, y, label, on, true)
    }

    /// A check box that can be greyed out (`enabled` false): drawn dim,
    /// and not clickable or focusable.
    pub fn checkbox_if(
        &mut self,
        f: F,
        x: i32,
        y: i32,
        label: &str,
        on: bool,
        enabled: bool,
    ) -> i32 {
        let t = &THEME;
        // A greyed box keeps its tick, dimmed.
        let tick = if enabled { t.highlight } else { t.dim };
        let s = self.lh * 2 / 3;
        let by = y + (self.lh - s) / 2;
        let b = Rect::new(x, by, s as u32, s as u32);
        self.canvas.set_draw_color(if on {
            tick
        } else {
            Color::RGB(0xFF, 0xFF, 0xFF)
        });
        let _ = self.canvas.fill_rect(b);
        self.canvas.set_draw_color(if on {
            tick
        } else if enabled {
            t.border
        } else {
            t.dim
        });
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
        let color = if enabled { t.text } else { t.dim };
        let lw = self
            .font
            .draw(self.canvas, x + s + self.pad * 2, y, label, color);
        let end = x + s + self.pad * 2 + lw;
        if enabled {
            self.hits.push((
                Rect::new(x, y, (end - x) as u32, self.lh as u32),
                f,
                Op::Toggle,
            ));
            self.ring(f, x, y, end);
        }
        end
    }

    fn small_button(&mut self, f: F, op: Op, x: i32, y: i32, label: &str) -> i32 {
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
    pub fn spinner(&mut self, f: F, x: i32, y: i32, value: &str, chars: usize) -> i32 {
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
        end
    }

    pub fn text_field(&mut self, f: F, x: i32, y: i32, w: i32, text: &str) {
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

    /// Colour sample, hue and luminance spinners, and the colour's name.
    /// Returns the x after it.
    #[allow(clippy::too_many_arguments)]
    pub fn color_row(
        &mut self,
        x: i32,
        y: i32,
        (hue_f, lum_f): (F, F),
        hue: u8,
        lum: u8,
        color: Color,
        name: &str,
    ) -> i32 {
        let t = &THEME;
        let sw = self.lh * 2;
        let swatch = Rect::new(x, y, sw as u32, self.lh as u32);
        self.canvas.set_draw_color(color);
        let _ = self.canvas.fill_rect(swatch);
        self.canvas.set_draw_color(t.border);
        let _ = self.canvas.draw_rect(swatch);
        let mut cx = x + sw + self.pad * 3;
        cx += self.font.draw(self.canvas, cx, y, "Hue", t.text) + self.pad * 2;
        cx = self.spinner(hue_f, cx, y, &hue.to_string(), 2) + self.pad * 4;
        cx += self.font.draw(self.canvas, cx, y, "Lum", t.text) + self.pad * 2;
        cx = self.spinner(lum_f, cx, y, &lum.to_string(), 2) + self.pad * 3;
        cx + self.font.draw(self.canvas, cx, y, name, t.dim)
    }

    pub fn button(&mut self, f: F, x: i32, y: i32, label: &str) {
        let w = self.font.measure(label) + self.pad * 4;
        self.button_w(f, x, y, w, label);
    }

    pub fn button_w(&mut self, f: F, x: i32, y: i32, w: i32, label: &str) {
        let t = &THEME;
        let in_row = (self.in_row)(f);
        // Buttons within a row get the row's height; the bottom ones are
        // a little taller.
        let (h, y) = if in_row {
            (self.lh + self.pad, y - self.pad / 2)
        } else {
            (self.lh + self.pad * 2, y)
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

    /// An empty framed box for a field the caller fills in (the Atari
    /// text fields): clicking it focuses `f`.
    pub fn field_box(&mut self, f: F, r: Rect) {
        let t = &THEME;
        self.canvas.set_draw_color(t.border);
        let _ = self.canvas.draw_rect(r);
        self.hits.push((r, f, Op::Focus));
        let p = self.pad;
        self.rings.push((
            f,
            Rect::new(
                r.x() - p,
                r.y() - p / 2,
                r.width() + p as u32 * 2,
                r.height() + p as u32,
            ),
        ));
    }

    /// Draw the focus ring around the focused control.
    pub fn draw_focus(&mut self) {
        let t = &THEME;
        for (f, r) in std::mem::take(&mut self.rings) {
            if f == self.focus {
                self.canvas.set_draw_color(t.highlight);
                let _ = self.canvas.draw_rect(r);
                let _ = self.canvas.draw_rect(Rect::new(
                    r.x() + 1,
                    r.y() + 1,
                    r.width().saturating_sub(2),
                    r.height().saturating_sub(2),
                ));
            }
        }
    }
}
