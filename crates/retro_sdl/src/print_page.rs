//! The Print page: paper, line width, character size and header, then
//! Print (a PDF opened in the system's viewer, which prints it) or Save as
//! PDF.

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas};
use sdl2::video::Window;

use crate::menu::THEME;
use crate::ui_font::UiFont;
use crate::widgets::{Op, Ui};

/// The controls, in Tab order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Paper,
    Width,
    Size,
    Header,
    SavePdf,
    Print,
    Cancel,
}

const ORDER: [Field; 7] = [
    Field::Paper,
    Field::Width,
    Field::Size,
    Field::Header,
    Field::SavePdf,
    Field::Print,
    Field::Cancel,
];

/// The values the page shows.
pub struct View {
    /// 0 = Letter, 1 = A4.
    pub paper: usize,
    /// 0 = 40 characters, 1 = as wide as the page.
    pub width: usize,
    pub size: u32,
    pub header: bool,
    /// What the settings give, e.g. "54 characters a line, 69 lines a page,
    /// 7 pages".
    pub summary: String,
}

const PAPERS: [&str; 2] = ["Letter", "A4"];
const WIDTHS: [&str; 2] = ["40 characters", "As wide as the page"];
const HEADER: &str = "File name and page numbers";
const SAVE_PDF: &str = "Save as PDF...";
const PRINT: &str = "Print...";
const CANCEL: &str = "Cancel";

const ROWS: [&str; 4] = ["Paper", "Line width", "Character size", "Header"];

pub struct PrintPage {
    focus: usize,
    hits: Vec<(Rect, Field, Op)>,
}

impl PrintPage {
    pub fn new() -> Self {
        Self {
            // Enter prints straight away.
            focus: ORDER.iter().position(|&f| f == Field::Print).unwrap_or(0),
            hits: Vec::new(),
        }
    }

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
        let row = lh + pad + pad / 2;
        let label_w = ROWS.iter().map(|l| font.measure(l)).max().unwrap_or(0);
        let r = lh / 3;
        let choice_w = |font: &mut UiFont, options: &[&str]| {
            options
                .iter()
                .map(|o| r * 2 + pad + font.measure(o))
                .sum::<i32>()
                + pad * 5 * (options.len() as i32 - 1)
        };
        let controls_w = choice_w(font, &PAPERS)
            .max(choice_w(font, &WIDTHS))
            .max(lh * 2 / 3 + pad * 2 + font.measure(HEADER))
            .max(font.measure(&v.summary));
        let buttons_w =
            font.measure(SAVE_PDF) + font.measure(PRINT) + font.measure(CANCEL) + pad * 30;
        let w =
            ((label_w + pad * 6 + controls_w).max(buttons_w) + pad * 8).min(width as i32 - pad * 4);
        let title_h = lh + pad * 2;
        let button_h = lh + pad * 2;
        // Title, four rows, the summary, then the buttons.
        let h = title_h + pad * 3 + row * 5 + pad * 2 + button_h + pad * 3;
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
        font.draw(canvas, x0 + pad * 4, y0 + pad, "Print", t.text);

        self.hits.clear();
        let mut ui = Ui {
            canvas,
            font,
            pad,
            lh,
            focus: self.focused(),
            hits: &mut self.hits,
            rings: Vec::new(),
            in_row: |_| false,
        };
        let lx = x0 + pad * 4;
        let cx = lx + label_w + pad * 6;
        let mut y = y0 + title_h + pad * 3;
        for label in ROWS {
            ui.font.draw(ui.canvas, lx, y, label, t.text);
            match label {
                "Paper" => {
                    ui.choice(Field::Paper, cx, y, &PAPERS, v.paper);
                }
                "Line width" => {
                    ui.choice(Field::Width, cx, y, &WIDTHS, v.width);
                }
                "Character size" => {
                    let end = ui.spinner(Field::Size, cx, y, &v.size.to_string(), 2);
                    ui.font.draw(ui.canvas, end + pad * 2, y, "points", t.text);
                }
                _ => {
                    ui.checkbox(Field::Header, cx, y, HEADER, v.header);
                }
            }
            y += row;
        }
        ui.font.draw(ui.canvas, cx, y, &v.summary, t.dim);

        // Save as PDF on the left; Print and Cancel on the right.
        let by = y0 + h - pad * 3 - button_h;
        let cancel_w = ui.font.measure(CANCEL) + pad * 8;
        let print_w = ui.font.measure(PRINT) + pad * 8;
        let bx = x0 + w - pad * 4 - cancel_w;
        ui.button_w(Field::Cancel, bx, by, cancel_w, CANCEL);
        ui.button_w(Field::Print, bx - pad * 2 - print_w, by, print_w, PRINT);
        let save_w = ui.font.measure(SAVE_PDF) + pad * 4;
        ui.button_w(Field::SavePdf, lx, by, save_w, SAVE_PDF);
        ui.draw_focus();
    }
}
