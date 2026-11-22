//! Drawing the window: menu bar, character grid, status bar, overlays.

use retro_core::WrapMode;
use sdl2::clipboard::ClipboardUtil;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, Texture};
use sdl2::video::Window;

use crate::app::{App, Frame, Overlay};
use crate::menu::{self, THEME};
use crate::render::Grid;
use crate::ui_font::UiFont;

impl App {
    pub fn draw<'t>(
        &mut self,
        canvas: &mut Canvas<Window>,
        atlas: &mut Texture,
        (font, mono, small): (&mut UiFont<'t>, &mut UiFont<'t>, &mut UiFont<'t>),
        f: &Frame,
        clipboard: &ClipboardUtil,
    ) {
        let colors = self.screen_colors();
        canvas.set_draw_color(colors.border);
        canvas.clear();

        if self.overlay == Overlay::None {
            // The machine's screen: text area background, then the text.
            let screen_cols = f.layout.left_margin + f.layout.text_cols;
            canvas.set_draw_color(colors.background);
            let _ = canvas.fill_rect(Rect::new(
                f.origin_x,
                f.text_top,
                screen_cols as u32 * f.cell,
                f.layout.rows as u32 * f.cell,
            ));
            let mut grid = Grid {
                canvas,
                atlas,
                cell: f.cell,
                origin_x: f.origin_x,
                origin_y: f.text_top,
            };
            self.draw_text(&mut grid, f);
        } else {
            // Pages of plain text (licences) read best in a monospace font.
            let page_font = if self.overlay == Overlay::Licensing {
                &mut *mono
            } else {
                &mut *font
            };
            self.draw_overlay(canvas, page_font, f);
        }

        self.draw_status(canvas, font, f);

        // Menu bar last, so an open menu covers the text.
        let menus = std::mem::take(&mut self.menus);
        let state = self.menu;
        self.geom = menu::draw(
            canvas,
            font,
            &menus,
            &self.context_menu,
            &state,
            (f.width, f.height),
            self.pad,
            &|c| self.item_state(c, clipboard),
        );
        self.menus = menus;

        if let Some(d) = &mut self.dialog {
            d.draw(canvas, font, (f.width, f.height), self.pad);
        }
        if let Some(mut p) = self.prefs.take() {
            p.draw(
                canvas,
                small,
                &self.prefs_view(),
                (f.width, f.height),
                self.prefs_pad,
            );
            self.prefs = Some(p);
        }
        if let Some(mut p) = self.print.take() {
            p.draw(
                canvas,
                small,
                &self.print_view(),
                (f.width, f.height),
                self.prefs_pad,
            );
            self.print = Some(p);
        }
    }

    fn draw_text(&self, grid: &mut Grid, f: &Frame) {
        let c = self.screen_colors();
        let (fg, bg, warn) = (c.text, c.background, c.warning);
        let (sel_bg, sel_fg) = (c.selection, c.selection_text);
        let layout = &f.layout;
        let doc = &self.editor.doc;
        let w = layout.text_cols;
        let warn_col = self.config.warning_column().unwrap_or(usize::MAX);
        let sel = self.editor.selection();
        let mut row = 0usize; // absolute screen row of the current line's first row
        for (i, line) in doc.lines().iter().enumerate() {
            let rows = layout.rows_for_len(line.len());
            // Selected columns of this line; `len + 1` includes the line end.
            let sel_cols = sel.and_then(|(a, b)| {
                if i < a.line || i > b.line {
                    return None;
                }
                let s0 = if i == a.line { a.col } else { 0 };
                let s1 = if i == b.line { b.col } else { line.len() + 1 };
                (s0 < s1).then_some((s0, s1))
            });
            for r in 0..rows {
                let abs = row + r;
                if abs < self.editor.top {
                    continue;
                }
                let screen = abs - self.editor.top;
                if screen >= layout.rows {
                    break;
                }
                let start = (r * w).min(line.len());
                let end = ((r + 1) * w).min(line.len());
                // Characters past the warning column use the warning colour.
                let split = warn_col.clamp(start, end);
                grid.codes(layout.left_margin, screen, &line[start..split], fg);
                grid.codes(
                    layout.left_margin + split - start,
                    screen,
                    &line[split..end],
                    warn,
                );
                // Selection: its own colours, so inverse characters
                // still look inverse inside it.
                if let Some((s0, s1)) = sel_cols {
                    let (c0, c1) = (s0.max(r * w), s1.min((r + 1) * w));
                    for c in c0..c1 {
                        let x = layout.left_margin + c - r * w;
                        grid.fill(x, screen, 1, sel_bg);
                        if let Some(&code) = line.get(c) {
                            grid.codes(x, screen, &[code], sel_fg);
                        }
                    }
                }
            }
            row += rows;
            if row >= self.editor.top + layout.rows {
                break;
            }
        }

        // Cursor: the character under it in inverse colours -- inverse of
        // the selection's colours when it's on a selected character, so
        // the selection still shows there.
        let cur = doc.cursor();
        let (fg, bg) = if sel.is_some_and(|(a, b)| a <= cur && cur < b) {
            (sel_fg, sel_bg)
        } else {
            (fg, bg)
        };
        let (crow, cx) = layout.locate(doc, cur);
        if crow >= self.editor.top && crow < self.editor.top + layout.rows {
            let screen = crow - self.editor.top;
            let code = doc
                .line(cur.line)
                .get(cur.col)
                .copied()
                .unwrap_or(self.spec.machine.space());
            grid.fill(layout.left_margin + cx, screen, 1, fg);
            grid.codes(layout.left_margin + cx, screen, &[code], bg);
        }
    }

    fn draw_status(&self, canvas: &mut Canvas<Window>, font: &mut UiFont, f: &Frame) {
        let t = &THEME;
        let (pad, h) = (self.pad, self.bar_height);
        let y = f.status_top;
        canvas.set_draw_color(t.bar);
        let _ = canvas.fill_rect(Rect::new(0, y, f.width, h as u32));
        canvas.set_draw_color(t.border);
        let _ = canvas.draw_line((0, y), (f.width as i32, y));
        let ty = y + pad / 2;

        // Right-hand indicators, separated by thin lines.
        let doc = &self.editor.doc;
        let cur = doc.cursor();
        let mut parts: Vec<(String, Color)> = Vec::new();
        let len = doc.line(cur.line).len();
        if self.config.warning_column().is_some_and(|w| len > w) {
            parts.push((
                format!("Line too long: {len}"),
                Color::RGB(0xC0, 0x40, 0x00),
            ));
        }
        if let Some((a, b)) = self.editor.selection() {
            let n: usize = doc
                .text_range(a, b)
                .iter()
                .map(|l| l.len() + 1)
                .sum::<usize>()
                - 1;
            parts.push((format!("{n} selected"), t.text));
        }
        parts.push((format!("Ln {}, Col {}", cur.line + 1, cur.col + 1), t.text));
        for ind in self.spec.keyboard.indicators() {
            let label = match ind {
                "INV" => "Inverse",
                "ESC" => "Esc",
                other => other,
            };
            parts.push((label.into(), t.highlight));
        }
        let m = &self.spec.machine;
        // The file's type: "Atari BASIC", "ATASCII"...
        parts.push((self.format.status_name().into(), t.text));
        if m.charset() != 0 {
            parts.push((m.charsets()[m.charset()].into(), t.text));
        }
        parts.push((
            if doc.overwrite { "Overwrite" } else { "Insert" }.into(),
            t.text,
        ));
        parts.push((
            match self.editor.wrap {
                WrapMode::Native => format!("{} columns", self.spec.machine.screen_columns()),
                WrapMode::Window => "Window width".into(),
            },
            t.text,
        ));

        let gap = pad * 3;
        let mut x = f.width as i32 - pad;
        let mut right_start = x;
        for (s, color) in parts.iter().rev() {
            let w = font.measure(s);
            x -= w;
            if x < pad * 20 {
                break;
            }
            font.draw(canvas, x, ty, s, *color);
            right_start = x;
            x -= gap / 2;
            canvas.set_draw_color(t.border);
            let _ = canvas.draw_line((x, y + pad / 2), (x, y + h - pad / 2));
            x -= gap / 2;
        }

        let left = match &self.message {
            _ if self.overlay != Overlay::None => {
                "Esc closes  \u{b7}  arrows, Page Up/Down or the mouse wheel scroll".into()
            }
            Some(m) => m.clone(),
            None => format!(
                "{}{}",
                self.file_name(),
                if doc.is_modified() { " (modified)" } else { "" }
            ),
        };
        // Clip the left text so it doesn't run into the indicators.
        let mut left = left;
        while !left.is_empty() && pad + font.measure(&left) > right_start - gap {
            left.pop();
        }
        font.draw(canvas, pad, ty, &left, t.text);
    }

    fn draw_overlay(&mut self, canvas: &mut Canvas<Window>, font: &mut UiFont, f: &Frame) {
        let t = &THEME;
        let pad = self.pad;
        let top = f.text_top;
        let height = f.status_top - top;
        canvas.set_draw_color(Color::RGB(0xFF, 0xFF, 0xFF));
        let _ = canvas.fill_rect(Rect::new(0, top, f.width, height.max(0) as u32));
        let x = pad * 3;
        let max_w = f.width as i32 - x * 2;

        // Each display line: an optional key column and the text.
        let text = match self.overlay {
            Overlay::Help => self.help_text(),
            Overlay::FileTypes => self.spec.file_types_help.join("\n"),
            _ => String::new(),
        };
        let lines: Vec<(Option<String>, String)> = match self.overlay {
            Overlay::Help | Overlay::FileTypes => text
                .lines()
                .map(|l| match l.find("  ") {
                    Some(i) => (Some(l[..i].trim().into()), l[i..].trim().into()),
                    None => (None, l.into()),
                })
                .collect(),
            _ => wrap_text(font, &self.spec.licensing, max_w),
        };
        let key_w = lines
            .iter()
            .filter_map(|(k, _)| k.as_deref())
            .map(|k| font.measure(k))
            .max()
            .unwrap_or(0);

        let lh = font.line_height();
        let visible = ((height - pad * 2) / lh).max(1) as usize;
        self.overlay_top = self.overlay_top.min(lines.len().saturating_sub(visible));
        let mut y = top + pad;
        for (key, text) in lines.iter().skip(self.overlay_top).take(visible) {
            match key {
                Some(k) => {
                    font.draw(canvas, x, y, k, t.text);
                    font.draw(canvas, x + key_w + pad * 3, y, text, t.text);
                }
                None => {
                    font.draw(canvas, x, y, text, t.text);
                }
            }
            y += lh;
        }
    }

    fn help_text(&self) -> String {
        let ctrl = self.config.control_key.label();
        let mut s = format!(
            "{} - KEYBOARD\n\
             \n\
             Menus  Alt+F / Alt+E / Alt+O / Alt+H, or F10\n\
             Ctrl+N / O / S  New / Open / Save\n\
             Ctrl+Shift+S  Save As\n\
             Ctrl+Z / Ctrl+Y  Undo / Redo\n\
             Ctrl+X / C / V  Cut / Copy / Paste\n\
             Ctrl+A  Select all\n\
             Shift+arrows  Select text (or drag with the mouse)\n\
             Ctrl+Home / End  Start / end of file\n\
             Ctrl+ + / -  Zoom in / out\n\
             F4  40 columns / window width\n\
             `  Inverse video on/off (the Atari key; left of 1)\n\
             F6  Standard / international character set\n\
             F7  Key click on/off\n\
             F8  CONTROL key ({ctrl})\n\
             F9  Line length warning (Preferences)\n\
             Ctrl+,  Preferences\n\
             \n\
             Insert  Insert / overwrite mode\n\
             Shift+Insert  Insert line\n\
             Shift+Delete  Delete line\n\
             Tab  Next tab stop\n\
             Shift+Tab  Set tab stop at cursor\n\
             CONTROL+Tab  Clear tab stop at cursor\n\
             \n",
            self.spec.title.to_uppercase()
        );
        for l in self.spec.help {
            s.push_str(l);
            s.push('\n');
        }
        s
    }
}

/// Word-wrap text to `max_w` pixels, keeping each line's indentation.
fn wrap_text(font: &mut UiFont, text: &str, max_w: i32) -> Vec<(Option<String>, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let indent: String = line.chars().take_while(|c| *c == ' ').collect();
        let mut cur = indent.clone();
        for word in line.split_whitespace() {
            let candidate = if cur.trim().is_empty() {
                format!("{indent}{word}")
            } else {
                format!("{cur} {word}")
            };
            if font.measure(&candidate) > max_w && !cur.trim().is_empty() {
                out.push((None, std::mem::replace(&mut cur, format!("{indent}{word}"))));
            } else {
                cur = candidate;
            }
        }
        out.push((None, cur));
    }
    out
}
