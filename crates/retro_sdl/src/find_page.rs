//! The Find / Replace panel and the Go to Line panel: small forms at the
//! top right of the window, leaving the text visible (and not dimmed) so
//! matches can be seen as they're found.
//!
//! The Find and Replace fields are Atari text, typed with the same keys
//! as the document (inverse, CONTROL graphics, Esc) and shown in the Atari
//! character set. The panel leaves their boxes empty and reports where
//! they are ([`FindPage::fields`]); the view draws the text in them.

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::video::Window;

use crate::menu::THEME;
use crate::ui_font::UiFont;
use crate::widgets::{Op, Ui};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Find,
    GoTo,
}

/// The controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    FindText,
    ReplaceText,
    MatchCase,
    MatchInverse,
    FindNext,
    ReplaceOne,
    ReplaceAll,
    LineText,
    EditorLine,
    Go,
    Close,
}

impl Field {
    /// Fields that take typed text.
    pub fn is_text(self) -> bool {
        matches!(self, Field::FindText | Field::ReplaceText | Field::LineText)
    }

    /// The Atari text fields.
    pub fn is_atari(self) -> bool {
        matches!(self, Field::FindText | Field::ReplaceText)
    }
}

fn order(mode: Mode) -> &'static [Field] {
    use Field::*;
    match mode {
        Mode::Find => &[
            FindText,
            MatchCase,
            ReplaceText,
            MatchInverse,
            FindNext,
            ReplaceOne,
            ReplaceAll,
            Close,
        ],
        Mode::GoTo => &[LineText, EditorLine, Go, Close],
    }
}

/// How many characters an Atari field shows.
pub const FIELD_CHARS: i32 = 22;

/// The values the panel shows.
pub struct View<'a> {
    pub match_case: bool,
    pub match_inverse: bool,
    pub line: &'a str,
    pub editor_line: bool,
}

pub struct FindPage {
    pub mode: Mode,
    focus: usize,
    hits: Vec<(Rect, Field, Op)>,
    /// Where the panel was last drawn.
    area: Rect,
    /// Where the Atari fields' text goes (inside their frames), and the
    /// character cell size, from the last draw.
    pub fields: Vec<(Field, Rect)>,
    pub cell: i32,
    /// Whether keys go to the panel. A click in the text gives them back
    /// to the document (the panel stays open); a click on the panel, or
    /// Find again, takes them.
    pub active: bool,
}

const FIND: &str = "Find";
const REPLACE: &str = "Replace with";
const MATCH_CASE: &str = "Match case";
const MATCH_INVERSE: &str = "Match inverse";
const FIND_NEXT: &str = "Find Next";
const REPLACE_ONE: &str = "Replace";
const REPLACE_ALL: &str = "Replace All";
const LINE: &str = "Go to line";
const EDITOR_LINE: &str = "Editor line";
const GO: &str = "Go";
const CLOSE: &str = "Close";

impl FindPage {
    pub fn new(mode: Mode, focus: Field) -> Self {
        let mut p = Self {
            mode,
            focus: 0,
            hits: Vec::new(),
            area: Rect::new(0, 0, 1, 1),
            fields: Vec::new(),
            cell: 16,
            active: true,
        };
        p.focus_on(focus);
        p
    }

    pub fn focused(&self) -> Field {
        order(self.mode)[self.focus]
    }

    pub fn focus_on(&mut self, f: Field) {
        self.focus = order(self.mode).iter().position(|&o| o == f).unwrap_or(0);
    }

    pub fn move_focus(&mut self, dir: isize) {
        let n = order(self.mode).len() as isize;
        self.focus = (self.focus as isize + dir).rem_euclid(n) as usize;
    }

    pub fn hit(&self, x: i32, y: i32) -> Option<(Field, Op)> {
        self.hits
            .iter()
            .find(|(r, ..)| r.contains_point((x, y)))
            .map(|&(_, f, op)| (f, op))
    }

    /// Whether a point is on the panel.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.area.contains_point((x, y))
    }

    /// Draw the panel at the top right of the text area (`top` to
    /// `bottom`), or at the bottom left if that would cover `avoid` (the
    /// found text), with Atari character cells of `cell` pixels in its
    /// fields.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        canvas: &mut Canvas<Window>,
        font: &mut UiFont,
        v: &View,
        width: u32,
        (top, bottom): (i32, i32),
        avoid: Option<Rect>,
        pad: i32,
        cell: i32,
    ) {
        let t = &THEME;
        let lh = font.line_height();
        self.cell = cell;
        // Rows are as tall as an Atari field (a character and a border).
        let field_h = cell + 4;
        let row = lh.max(field_h) + pad * 2;
        let button = |font: &mut UiFont, s: &str| font.measure(s) + pad * 4;
        let check = |font: &mut UiFont, s: &str| lh * 2 / 3 + pad * 2 + font.measure(s);
        let field_w = cell * FIELD_CHARS + 4;
        let label_w = match self.mode {
            Mode::Find => font.measure(REPLACE),
            Mode::GoTo => font.measure(LINE),
        };
        let w = match self.mode {
            Mode::Find => {
                let fields = label_w
                    + field_w
                    + check(font, MATCH_CASE).max(check(font, MATCH_INVERSE))
                    + pad * 10;
                let buttons = button(font, FIND_NEXT)
                    + button(font, REPLACE_ONE)
                    + button(font, REPLACE_ALL)
                    + button(font, CLOSE)
                    + pad * 6;
                fields.max(buttons)
            }
            Mode::GoTo => {
                label_w
                    + font.measure("0") * 8
                    + check(font, EDITOR_LINE)
                    + button(font, GO)
                    + button(font, CLOSE)
                    + pad * 14
            }
        } + pad * 6;
        let rows = if self.mode == Mode::Find { 3 } else { 1 };
        let h = row * rows + pad * 2;
        let top_right = Rect::new(
            (width as i32 - w - pad * 2).max(0),
            top + pad,
            w as u32,
            h as u32,
        );
        let bottom_left = Rect::new(pad * 2, (bottom - h - pad).max(top), w as u32, h as u32);
        self.area = match avoid {
            Some(a) if a.has_intersection(top_right) => bottom_left,
            _ => top_right,
        };
        let (x0, y0) = (self.area.x(), self.area.y());

        canvas.set_draw_color(Color::RGB(0xFF, 0xFF, 0xFF));
        let _ = canvas.fill_rect(self.area);
        canvas.set_draw_color(t.border);
        let _ = canvas.draw_rect(self.area);

        self.hits.clear();
        self.fields.clear();
        let mut ui = Ui {
            canvas,
            font,
            pad,
            lh,
            focus: self.focused(),
            hits: &mut self.hits,
            rings: Vec::new(),
            in_row: |_| true,
        };
        let lx = x0 + pad * 3;
        let fx = lx + label_w + pad * 3;
        // Text and controls centred in each row.
        let mut y = y0 + pad * 2;
        let text_y = |y: i32| y + (row - pad * 2 - lh) / 2;
        let field_y = |y: i32| y + (row - pad * 2 - field_h) / 2;
        match self.mode {
            Mode::Find => {
                for (label, f, check_f, check_label, on) in [
                    (
                        FIND,
                        Field::FindText,
                        Field::MatchCase,
                        MATCH_CASE,
                        v.match_case,
                    ),
                    (
                        REPLACE,
                        Field::ReplaceText,
                        Field::MatchInverse,
                        MATCH_INVERSE,
                        v.match_inverse,
                    ),
                ] {
                    ui.font.draw(ui.canvas, lx, text_y(y), label, t.text);
                    let r = Rect::new(fx, field_y(y), field_w as u32, field_h as u32);
                    ui.field_box(f, r);
                    self.fields.push((
                        f,
                        Rect::new(r.x() + 2, r.y() + 2, (field_w - 4) as u32, cell as u32),
                    ));
                    ui.checkbox(check_f, fx + field_w + pad * 4, text_y(y), check_label, on);
                    y += row;
                }
                // Buttons along the bottom, right-aligned.
                let mut bx = x0 + w - pad * 3;
                for (f, label) in [
                    (Field::Close, CLOSE),
                    (Field::ReplaceAll, REPLACE_ALL),
                    (Field::ReplaceOne, REPLACE_ONE),
                    (Field::FindNext, FIND_NEXT),
                ] {
                    let bw = button(ui.font, label);
                    bx -= bw;
                    ui.button_w(f, bx, text_y(y), bw, label);
                    bx -= pad * 2;
                }
            }
            Mode::GoTo => {
                let digits_w = ui.font.measure("0") * 8;
                ui.font.draw(ui.canvas, lx, text_y(y), LINE, t.text);
                ui.text_field(Field::LineText, fx, text_y(y), digits_w, v.line);
                let cx = fx + digits_w + pad * 4;
                let after =
                    ui.checkbox(Field::EditorLine, cx, text_y(y), EDITOR_LINE, v.editor_line);
                let gx = after + pad * 4;
                let gw = button(ui.font, GO);
                ui.button_w(Field::Go, gx, text_y(y), gw, GO);
                let cw = button(ui.font, CLOSE);
                ui.button_w(Field::Close, gx + gw + pad * 2, text_y(y), cw, CLOSE);
            }
        }
        if self.active {
            ui.draw_focus();
        }
    }
}
