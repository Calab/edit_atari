//! Drawing the window: menu bar, character grid, status bar, overlays.

use retro_core::WrapMode;
use sdl2::clipboard::ClipboardUtil;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, Texture};
use sdl2::video::Window;

use crate::app::{App, Frame, Overlay};
use crate::config::ControlKey;
use crate::find_page;
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

        // The Find panel, under the menus.
        if let Some(mut p) = self.find.take() {
            // Atari characters in its fields at twice their size (more on
            // high-DPI displays), whatever the zoom.
            let cell = 8 * (2.0 * self.ratio).round().max(1.0) as i32;
            let found = self.found_rect(f);
            p.draw(
                canvas,
                small,
                &self.find_view(),
                f.width,
                (f.text_top, f.status_top),
                found,
                self.prefs_pad,
                cell,
            );
            let focused = p.focused();
            for &(field, r) in &p.fields {
                self.draw_field(canvas, atlas, field, r, cell, p.active && field == focused);
            }
            self.find = Some(p);
        }

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
        if let Some(mut p) = self.fonts.take() {
            let cell = 8 * (2.0 * self.ratio).round().max(1.0) as i32;
            p.draw(
                canvas,
                small,
                &self.fonts_view(),
                (f.width, f.height),
                self.prefs_pad,
                cell,
            );
            self.fonts = Some(p);
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

    /// Where the selection (the found text) is on screen, or the cursor
    /// if nothing is selected: the rows it spans, for the Find panel to
    /// keep clear of. `None` if it's scrolled out of view.
    fn found_rect(&self, f: &Frame) -> Option<Rect> {
        let doc = &self.editor.doc;
        let (a, b) = self
            .editor
            .selection()
            .unwrap_or((doc.cursor(), doc.cursor()));
        let layout = &f.layout;
        let (row_a, col_a) = layout.locate(doc, a);
        let (row_b, col_b) = layout.locate(doc, b);
        let top = self.editor.top;
        if row_b < top || row_a >= top + layout.rows {
            return None;
        }
        let cell = f.cell as i32;
        let x = |col: usize| f.origin_x + (layout.left_margin + col) as i32 * cell;
        let y = |row: usize| f.text_top + (row as i32 - top as i32) * cell;
        // One row: just the text; several: the full width of those rows.
        let (x0, x1) = if row_a == row_b {
            (x(col_a), x(col_b.max(col_a + 1)))
        } else {
            (x(0), x(layout.text_cols))
        };
        Some(Rect::new(
            x0,
            y(row_a.max(top)),
            (x1 - x0).max(1) as u32,
            ((row_b.min(top + layout.rows - 1) - row_a.max(top) + 1) as i32 * cell) as u32,
        ))
    }

    /// An Atari text field of the Find panel in `r`: its text in the
    /// screen's colours, scrolled to keep the cursor in view, with the
    /// selection, and the cursor when it has the focus.
    fn draw_field(
        &mut self,
        canvas: &mut Canvas<Window>,
        atlas: &mut Texture,
        field: find_page::Field,
        r: Rect,
        cell: i32,
        focused: bool,
    ) {
        let c = self.screen_colors();
        let i = usize::from(field == find_page::Field::ReplaceText);
        let e = if i == 0 {
            &self.find_field
        } else {
            &self.replace_field
        };
        let line = e.doc.line(0);
        let cursor = e.doc.cursor().col;
        let cols = (r.width() as i32 / cell).max(1) as usize;
        let scroll = &mut self.field_scroll[i];
        if cursor < *scroll {
            *scroll = cursor;
        } else if cursor >= *scroll + cols {
            *scroll = cursor + 1 - cols;
        }
        let scroll = *scroll;
        canvas.set_draw_color(c.background);
        let _ = canvas.fill_rect(r);
        let mut grid = Grid {
            canvas,
            atlas,
            cell: cell as u32,
            origin_x: r.x(),
            origin_y: r.y(),
        };
        let end = line.len().min(scroll + cols);
        if scroll < end {
            grid.codes(0, 0, &line[scroll..end], c.text);
        }
        if let Some((a, b)) = e.selection() {
            let (s0, s1) = (a.col.max(scroll), b.col.min(scroll + cols));
            for (x, &code) in (s0..s1).map(|c| c - scroll).zip(&line[s0..s1.max(s0)]) {
                grid.fill(x, 0, 1, c.selection);
                grid.codes(x, 0, &[code], c.selection_text);
            }
        }
        if focused && self.cursor_visible() && cursor >= scroll && cursor < scroll + cols {
            let code = line
                .get(cursor)
                .copied()
                .unwrap_or(self.spec.machine.space());
            grid.fill(cursor - scroll, 0, 1, c.text);
            grid.codes(cursor - scroll, 0, &[code], c.background);
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
        if self.cursor_visible() && crow >= self.editor.top && crow < self.editor.top + layout.rows
        {
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
                "Esc closes  \u{b7}  arrows, Page Up/Down, scroll".into()
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
        let (ctrl_key, alt_key) = (
            ControlKey::modifier_label(false),
            ControlKey::modifier_label(true),
        );
        // The editor's own shortcuts: Command on macOS, Ctrl elsewhere.
        let shortcuts = if cfg!(target_os = "macos") {
            "Menus  Option+F / E / O / H, or F10\n\
             \u{2318}N / O / S  New / Open / Save\n\
             \u{21E7}\u{2318}S  Save As\n\
             \u{2318}P  Print to PDF\n\
             \u{2318}Z / \u{21E7}\u{2318}Z  Undo / Redo\n\
             \u{2318}X / C / V  Cut / Copy / Paste\n\
             \u{2318}A  Select all\n\
             \u{2318}F  Find (and replace)\n\
             \u{2318}G / \u{21E7}\u{2318}G  Find next / previous\n\
             \u{2325}\u{2318}F  Replace\n\
             \u{2318}L  Go to line number\n\
             Shift+arrows  Select text (or drag with the mouse)\n\
             \u{2325}\u{2190} / \u{2325}\u{2192}  Previous / next word (Shift selects)\n\
             \u{2318}\u{2191} / \u{2318}\u{2193}  Start / end of file\n\
             \u{2318}\u{2190} / \u{2318}\u{2192}  Start / end of line\n\
             \u{2318}+ / -  Zoom in / out\n\
             Control-click  Right-click menu\n\
             \u{2318},  Preferences\n"
        } else {
            "Menus  Alt+F / Alt+E / Alt+O / Alt+H, or F10\n\
             Ctrl+N / O / S  New / Open / Save\n\
             Ctrl+Shift+S  Save As\n\
             Ctrl+P  Print to PDF\n\
             Ctrl+Z / Ctrl+Y  Undo / Redo\n\
             Ctrl+X / C / V  Cut / Copy / Paste\n\
             Ctrl+A  Select all\n\
             Ctrl+F  Find (and replace)\n\
             Ctrl+H  Replace\n\
             Ctrl+G  Go to line number\n\
             Shift+arrows  Select text (or drag with the mouse)\n\
             Ctrl+Left / Right  Previous / next word (Shift selects)\n\
             Ctrl+Home / End  Start / end of file\n\
             Ctrl+ + / -  Zoom in / out\n\
             Ctrl+,  Preferences\n"
        };
        let mut s = format!(
            "{} - KEYBOARD\n\
             \n\
             {shortcuts}\
             Drop files on the window to open them (one per window).\n\
             \n\
             F1  This keyboard help\n\
             F2  Save\n\
             F3 / Shift+F3  Find next / previous\n\
             F4  40 columns / window width\n\
             F5  Key click on/off\n\
             F6  Standard / international character set\n\
             F7  CONTROL key on left/right side toggle\n\
             F8  CONTROL on {ctrl_key}/{alt_key} toggle\n\
             F9  Preferences\n\
             F10  Menus\n\
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
