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
use crate::ui_font::UiFont;

/// What to do after the unsaved changes have been dealt with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum After {
    New,
    Open,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    About,
    /// "Save changes?" before doing something that discards the document.
    SaveChanges(After),
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
            .map(|l| font.measure(l))
            .max()
            .unwrap_or(0)
            .max(title_w);
        let w = text_w.max(buttons_total) + pad * 8;
        let title_h = lh + pad * 2;
        let button_h = lh + pad * 2;
        let h = title_h + pad * 3 + lines.len() as i32 * lh + pad * 4 + button_h + pad * 3;
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
            font.draw(canvas, x + pad * 4, ty, l, t.text);
            ty += lh;
        }

        // Buttons, right-aligned.
        let by = y + h - pad * 3 - button_h;
        let mut bx = x + w - pad * 4 - buttons_total;
        self.button_rects.clear();
        for (i, label) in self.buttons.iter().enumerate() {
            let r = Rect::new(bx, by, button_w[i] as u32, button_h as u32);
            let focused = i == self.focus;
            canvas.set_draw_color(if focused { t.highlight } else { t.bar });
            let _ = canvas.fill_rect(r);
            canvas.set_draw_color(if focused { t.highlight } else { t.border });
            let _ = canvas.draw_rect(r);
            let lw = font.measure(label);
            let color = if focused { t.highlight_text } else { t.text };
            font.draw(canvas, bx + (button_w[i] - lw) / 2, by + pad, label, color);
            self.button_rects.push(r);
            bx += button_w[i] + pad * 2;
        }
    }
}
