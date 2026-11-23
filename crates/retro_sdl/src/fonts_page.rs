//! The Fonts page (Options > Fonts): for each of the machine's character
//! sets, the font file last loaded, a Browse button, a Built-in check box
//! (unticking it brings the font file back; greyed out until there is
//! one), and a preview of the characters in use. Changes apply at once.

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas};
use sdl2::video::Window;

use crate::menu::THEME;
use crate::ui_font::UiFont;
use crate::widgets::{Op, Ui};

/// The controls: Browse and Built-in for each character set, and Close.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Browse(usize),
    BuiltIn(usize),
    Close,
}

/// One character set's row.
pub struct SetView {
    /// The character set's name, e.g. "Graphics".
    pub name: &'static str,
    /// The font file's name, if one has been loaded.
    pub file: Option<String>,
    /// The built-in set is in use.
    pub builtin: bool,
    /// Its first 128 characters, 8 bytes each.
    pub glyphs: Vec<[u8; 8]>,
}

pub struct View {
    pub sets: Vec<SetView>,
    pub background: Color,
    pub text: Color,
}

/// Characters per preview row.
const PER_ROW: usize = 32;

const BROWSE: &str = "Browse...";
const BUILT_IN: &str = "Built-in";
const CLOSE: &str = "Close";

pub struct FontsPage {
    focus: usize,
    /// Which sets have a font file to go back to (their Built-in box can
    /// be unticked), from the last draw.
    has_file: Vec<bool>,
    hits: Vec<(Rect, Field, Op)>,
}

impl FontsPage {
    pub fn new(sets: usize) -> Self {
        Self {
            focus: 0,
            has_file: vec![false; sets],
            hits: Vec::new(),
        }
    }

    /// The controls in Tab order, leaving out greyed-out boxes.
    fn order(&self) -> Vec<Field> {
        let mut v: Vec<Field> = Vec::new();
        for (i, &has_file) in self.has_file.iter().enumerate() {
            v.push(Field::Browse(i));
            if has_file {
                v.push(Field::BuiltIn(i));
            }
        }
        v.push(Field::Close);
        v
    }

    pub fn focused(&self) -> Field {
        let order = self.order();
        order[self.focus.min(order.len() - 1)]
    }

    pub fn focus_on(&mut self, f: Field) {
        self.focus = self.order().iter().position(|&o| o == f).unwrap_or(0);
    }

    pub fn move_focus(&mut self, dir: isize) {
        let n = self.order().len() as isize;
        self.focus = (self.focus as isize + dir).rem_euclid(n) as usize;
    }

    pub fn hit(&self, x: i32, y: i32) -> Option<(Field, Op)> {
        self.hits
            .iter()
            .find(|(r, ..)| r.contains_point((x, y)))
            .map(|&(_, f, op)| (f, op))
    }

    /// Draw the page centred in the window, previews in `cell`-pixel
    /// characters.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        canvas: &mut Canvas<Window>,
        font: &mut UiFont,
        v: &View,
        (width, height): (u32, u32),
        pad: i32,
        cell: i32,
    ) {
        let t = &THEME;
        canvas.set_blend_mode(BlendMode::BLEND);
        canvas.set_draw_color(Color::RGBA(0, 0, 0, 90));
        let _ = canvas.fill_rect(None);
        canvas.set_blend_mode(BlendMode::NONE);

        let lh = font.line_height();
        let rows_per_set = 128usize.div_ceil(PER_ROW) as i32;
        let preview_w = PER_ROW as i32 * cell;
        let preview_h = rows_per_set * cell;
        let label_w = v
            .sets
            .iter()
            .map(|s| font.measure(s.name))
            .max()
            .unwrap_or(0);
        let button = |font: &mut UiFont, s: &str| font.measure(s) + pad * 4;
        let check_w = lh * 2 / 3 + pad * 2 + font.measure(BUILT_IN);
        let buttons_w = button(font, BROWSE) + check_w + pad * 4;
        let content_w = preview_w.max(font.measure("M") * 16 + buttons_w + pad * 4);
        let w = (label_w + pad * 6 + content_w + pad * 8).min(width as i32 - pad * 4);
        let title_h = lh + pad * 2;
        let set_h = lh + pad * 2 + preview_h + pad * 4;
        let button_h = lh + pad * 2;
        let h = title_h + pad * 3 + set_h * v.sets.len() as i32 + button_h + pad * 3;
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
        font.draw(canvas, x0 + pad * 4, y0 + pad, "Fonts", t.text);

        self.hits.clear();
        self.has_file = v.sets.iter().map(|s| s.file.is_some()).collect();
        let mut ui = Ui {
            canvas,
            font,
            pad,
            lh,
            focus: self.focused(),
            hits: &mut self.hits,
            rings: Vec::new(),
            in_row: |f| f != Field::Close,
        };
        let lx = x0 + pad * 4;
        let cx = lx + label_w + pad * 6;
        let mut y = y0 + title_h + pad * 3;
        for (i, set) in v.sets.iter().enumerate() {
            ui.font.draw(ui.canvas, lx, y, set.name, t.text);
            match &set.file {
                Some(name) => ui.font.draw(ui.canvas, cx, y, name, t.text),
                None => ui
                    .font
                    .draw(ui.canvas, cx, y, "No font file loaded yet", t.dim),
            };
            // Browse and Built-in at the right.
            let bx = x0 + w - pad * 4 - check_w;
            ui.checkbox_if(
                Field::BuiltIn(i),
                bx,
                y,
                BUILT_IN,
                set.builtin,
                set.file.is_some(),
            );
            let rw = button(ui.font, BROWSE);
            ui.button_w(Field::Browse(i), bx - pad * 4 - rw, y, rw, BROWSE);
            // The preview: 128 characters in the screen's colours.
            let py = y + lh + pad * 2;
            let area = Rect::new(cx, py, preview_w as u32, preview_h as u32);
            ui.canvas.set_draw_color(v.background);
            let _ = ui.canvas.fill_rect(area);
            let px = (cell / 8).max(1);
            let mut dots = Vec::new();
            for (n, glyph) in set.glyphs.iter().enumerate() {
                let gx = cx + (n % PER_ROW) as i32 * cell;
                let gy = py + (n / PER_ROW) as i32 * cell;
                for (row, &bits) in glyph.iter().enumerate() {
                    for col in 0..8 {
                        if bits & (0x80 >> col) != 0 {
                            dots.push(Rect::new(
                                gx + col * px,
                                gy + row as i32 * px,
                                px as u32,
                                px as u32,
                            ));
                        }
                    }
                }
            }
            ui.canvas.set_draw_color(v.text);
            let _ = ui.canvas.fill_rects(&dots);
            y += set_h;
        }

        let cw = button(ui.font, CLOSE) + pad * 4;
        ui.button_w(Field::Close, x0 + w - pad * 4 - cw, y, cw, CLOSE);
        ui.draw_focus();
    }
}
