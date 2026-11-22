//! Small modal dialogs drawn inside the window (About, "Save changes?").
//!
//! These replace the OS message boxes, which block the event loop: while
//! blocked, the window can't answer the desktop's "are you alive?" checks
//! and GNOME reports the program as not responding.

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas};
use sdl2::video::Window;

use crate::menu::THEME;
use crate::ui_font::{BOLD, UiFont};

/// What to do after the unsaved changes have been dealt with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum After {
    New,
    Open,
    Import,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    About,
    /// "Save changes?" before doing something that discards the document.
    SaveChanges(After),
    /// Choose a format to read the current file in; the buttons are the
    /// formats' names, then Cancel.
    ReopenAs,
    /// Some lines would be saved as ERROR- lines: Save Anyway / Cancel.
    SaveProblemLines,
    /// Choose the format to save in; the buttons are the formats' names,
    /// then Cancel.
    SaveAsFormat,
    /// Export as text: 0 = Atari font, 1 = standard Unicode, 2 = ASCII,
    /// 3 = Cancel.
    Export,
    /// Characters won't export exactly: Export Anyway / Cancel.
    ExportLossy(crate::app::ExportKind),
    /// After Import: Convert to capitals / Leave As Is.
    ImportCase,
}

pub struct Dialog {
    pub purpose: Purpose,
    pub title: String,
    pub text: String,
    pub buttons: Vec<&'static str>,
    /// The highlighted button (Enter chooses it).
    pub focus: usize,
    /// The button Esc chooses.
    pub cancel: usize,
    /// Where the buttons were last drawn.
    pub button_rects: Vec<Rect>,
}

impl Dialog {
    pub fn about(title: &str, text: &str) -> Self {
        Self {
            purpose: Purpose::About,
            title: format!("About {title}"),
            text: text.into(),
            buttons: vec!["OK"],
            focus: 0,
            cancel: 0,
            button_rects: Vec::new(),
        }
    }

    /// Buttons: 0 = Save, 1 = Don't Save, 2 = Cancel.
    pub fn save_changes(file_name: &str, after: After) -> Self {
        Self {
            purpose: Purpose::SaveChanges(after),
            title: "Unsaved changes".into(),
            text: format!("Save changes to {file_name}?"),
            buttons: vec!["Save", "Don't Save", "Cancel"],
            focus: 0,
            cancel: 2,
            button_rects: Vec::new(),
        }
    }

    /// Buttons: one per format name, then Cancel.
    pub fn reopen_as(file_name: &str, formats: Vec<&'static str>, current: usize) -> Self {
        let cancel = formats.len();
        let mut buttons = formats;
        buttons.push("Cancel");
        Self {
            purpose: Purpose::ReopenAs,
            title: "Reopen As".into(),
            text: format!("Read {file_name} as:"),
            buttons,
            focus: current,
            cancel,
            button_rects: Vec::new(),
        }
    }

    /// Buttons: 0 = Save Anyway, 1 = Cancel.
    pub fn save_problem_lines(format: &str, lines: &[u32]) -> Self {
        let shown: Vec<String> = lines.iter().take(8).map(u32::to_string).collect();
        let more = if lines.len() > 8 { ", ..." } else { "" };
        let what = if lines.len() == 1 {
            "line has a syntax error"
        } else {
            "lines have syntax errors"
        };
        Self {
            purpose: Purpose::SaveProblemLines,
            title: "Syntax errors".into(),
            text: format!(
                "{} {what}: {}{more}\n\
                 They will be saved as ERROR- lines, as {format} would.",
                lines.len(),
                shown.join(", ")
            ),
            buttons: vec!["Save Anyway", "Cancel"],
            focus: 1,
            cancel: 1,
            button_rects: Vec::new(),
        }
    }

    /// Buttons: one per format name, then Cancel.
    pub fn save_as_format(file_name: &str, formats: Vec<&'static str>) -> Self {
        let cancel = formats.len();
        let mut buttons = formats;
        buttons.push("Cancel");
        Self {
            purpose: Purpose::SaveAsFormat,
            title: "Save As".into(),
            text: format!("Save {file_name} as:"),
            buttons,
            focus: 0,
            cancel,
            button_rects: Vec::new(),
        }
    }

    /// Buttons: 0 = Atari font, 1 = Standard, 2 = ASCII, 3 = Cancel.
    pub fn export(file_name: &str) -> Self {
        Self {
            purpose: Purpose::Export,
            title: "Export".into(),
            text: format!(
                "Export \"{file_name}\" as PC text:\n\
                 \n\
                 {BOLD}Atari Font{BOLD}: UTF-8 for the EightBit Atari font;\n\
                 every character exact.\n\
                 {BOLD}Standard{BOLD}: UTF-8 with ordinary Unicode symbols,\n\
                 for any font. Most inverse characters can't be\n\
                 shown and export as normal ones.\n\
                 {BOLD}ASCII{BOLD}: plain text, other characters as {{tags}}\n\
                 such as {{CLEAR}}, {{CTRL-A}} and {{INV}}...{{/INV}}.\n\
                 \n\
                 File > Import reads all three back."
            ),
            buttons: vec!["Atari Font", "Standard", "ASCII", "Cancel"],
            focus: 0,
            cancel: 3,
            button_rects: Vec::new(),
        }
    }

    /// Buttons: 0 = Export Anyway, 1 = Cancel.
    pub fn export_lossy(kind: crate::app::ExportKind, lost: usize) -> Self {
        let what = if lost == 1 {
            "1 character has".to_string()
        } else {
            format!("{lost} characters have")
        };
        Self {
            purpose: Purpose::ExportLossy(kind),
            title: "Export".into(),
            text: format!(
                "{what} no standard Unicode equivalent\n\
                 (inverse video) and will be exported as normal characters.\n\
                 Exporting in Atari font or ASCII keeps them."
            ),
            buttons: vec!["Export Anyway", "Cancel"],
            focus: 1,
            cancel: 1,
            button_rects: Vec::new(),
        }
    }

    /// Buttons: 0 = Convert, 1 = Leave As Is.
    pub fn import_case(file_name: &str, offer: &retro_core::CaseOffer) -> Self {
        let lines = if offer.changed == 1 {
            "1 line".to_string()
        } else {
            format!("{} lines", offer.changed)
        };
        Self {
            purpose: Purpose::ImportCase,
            title: "Import".into(),
            text: format!(
                "{file_name} looks like {} with lower case\n\
                 in {lines}. Convert it to capitals?\n\
                 {} are left as they are.",
                offer.what, offer.kept
            ),
            buttons: vec!["Convert", "Leave As Is"],
            focus: 0,
            cancel: 1,
            button_rects: Vec::new(),
        }
    }

    pub fn button_at(&self, x: i32, y: i32) -> Option<usize> {
        self.button_rects
            .iter()
            .position(|r| r.contains_point((x, y)))
    }

    /// Draw the dialog centred in a `width` x `height` window.
    pub fn draw(
        &mut self,
        canvas: &mut Canvas<Window>,
        font: &mut UiFont,
        (width, height): (u32, u32),
        pad: i32,
    ) {
        let t = &THEME;
        // Dim everything behind the dialog.
        canvas.set_blend_mode(BlendMode::BLEND);
        canvas.set_draw_color(Color::RGBA(0, 0, 0, 90));
        let _ = canvas.fill_rect(None);
        canvas.set_blend_mode(BlendMode::NONE);

        let lh = font.line_height();
        let lines: Vec<&str> = self.text.lines().collect();
        let button_w: Vec<i32> = self
            .buttons
            .iter()
            .map(|b| (font.measure(b) + pad * 4).max(pad * 14))
            .collect();
        let buttons_total: i32 =
            button_w.iter().sum::<i32>() + pad * 2 * (button_w.len() as i32 - 1);
        let title_w = font.measure(&self.title);
        let text_w = lines
            .iter()
            .map(|l| font.measure_marked(l))
            .max()
            .unwrap_or(0)
            .max(title_w);
        let title_h = lh + pad * 2;
        let button_h = lh + pad * 2;
        // Buttons in a row, or stacked in a column if a row won't fit.
        let stacked = buttons_total + pad * 8 > width as i32;
        let widest = button_w.iter().copied().max().unwrap_or(0);
        let n = button_w.len() as i32;
        let (buttons_w, buttons_h) = if stacked {
            (widest, n * button_h + (n - 1) * pad)
        } else {
            (buttons_total, button_h)
        };
        let w = text_w.max(buttons_w) + pad * 8;
        let h = title_h + pad * 3 + lines.len() as i32 * lh + pad * 4 + buttons_h + pad * 3;
        let x = (width as i32 - w) / 2;
        let y = (height as i32 - h) / 2;

        // Box with a title strip.
        let frame = Rect::new(x, y, w as u32, h as u32);
        canvas.set_draw_color(Color::RGB(0xFF, 0xFF, 0xFF));
        let _ = canvas.fill_rect(frame);
        canvas.set_draw_color(t.bar);
        let _ = canvas.fill_rect(Rect::new(x, y, w as u32, title_h as u32));
        canvas.set_draw_color(t.border);
        let _ = canvas.draw_line((x, y + title_h), (x + w - 1, y + title_h));
        let _ = canvas.draw_rect(frame);
        font.draw(canvas, x + pad * 4, y + pad, &self.title, t.text);

        let mut ty = y + title_h + pad * 3;
        for l in &lines {
            font.draw_marked(canvas, x + pad * 4, ty, l, t.text);
            ty += lh;
        }

        // Buttons, right-aligned (or a centred column).
        let mut by = y + h - pad * 3 - buttons_h;
        let mut bx = x + w - pad * 4 - buttons_total;
        self.button_rects.clear();
        for (i, label) in self.buttons.iter().enumerate() {
            let bw = if stacked { widest } else { button_w[i] };
            if stacked {
                bx = x + (w - widest) / 2;
            }
            let r = Rect::new(bx, by, bw as u32, button_h as u32);
            let focused = i == self.focus;
            canvas.set_draw_color(if focused { t.highlight } else { t.bar });
            let _ = canvas.fill_rect(r);
            canvas.set_draw_color(if focused { t.highlight } else { t.border });
            let _ = canvas.draw_rect(r);
            let lw = font.measure(label);
            let color = if focused { t.highlight_text } else { t.text };
            font.draw(canvas, bx + (bw - lw) / 2, by + pad, label, color);
            self.button_rects.push(r);
            if stacked {
                by += button_h + pad;
            } else {
                bx += bw + pad * 2;
            }
        }
    }
}
