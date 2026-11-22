//! Editor state: a document plus scrolling, applying [`EditAction`]s.

use crate::document::Document;
use crate::input::EditAction;
use crate::layout::{Layout, WrapMode};
use crate::machine::Machine;

/// Logical columns between tab stops.
pub const TAB_WIDTH: usize = 8;

#[derive(Debug, Clone)]
pub struct Editor {
    pub doc: Document,
    pub wrap: WrapMode,
    /// First visible screen row.
    pub top: usize,
    /// Column to aim for when moving up/down through shorter lines.
    want_x: Option<usize>,
}

impl Editor {
    pub fn new(doc: Document, wrap: WrapMode) -> Self {
        Self {
            doc,
            wrap,
            top: 0,
            want_x: None,
        }
    }

    /// Screen layout for a window of `win_cols` x `win_rows` text cells.
    pub fn layout(&self, machine: &dyn Machine, win_cols: usize, win_rows: usize) -> Layout {
        Layout::new(
            self.wrap,
            win_cols,
            win_rows,
            machine.screen_columns(),
            machine.left_margin(),
        )
    }

    /// Apply an action. Returns true if anything may have changed on screen.
    pub fn apply(&mut self, action: EditAction, layout: &Layout, machine: &dyn Machine) -> bool {
        use EditAction::*;
        let doc = &mut self.doc;
        let vertical = matches!(action, MoveUp | MoveDown | PageUp | PageDown);
        if vertical {
            let x = *self
                .want_x
                .get_or_insert_with(|| layout.locate(doc, doc.cursor()).1);
            let page = layout.rows.saturating_sub(1).max(1) as isize;
            let delta = match action {
                MoveUp => -1,
                MoveDown => 1,
                PageUp => -page,
                _ => page,
            };
            let pos = layout.move_rows(doc, doc.cursor(), delta, x);
            doc.set_cursor(pos);
            if matches!(action, PageUp | PageDown) {
                self.top = (self.top as isize + delta).max(0) as usize;
            }
        } else {
            self.want_x = Option::None;
            match action {
                Insert(code) => doc.type_code(code),
                NewLine => doc.new_line(),
                Backspace => doc.backspace(),
                DeleteForward => doc.delete_forward(),
                InsertSpace => doc.insert_space(machine.space()),
                DeleteLine => doc.delete_line(),
                InsertLine => doc.insert_line(),
                MoveLeft => doc.move_left(),
                MoveRight => doc.move_right(),
                LineStart => doc.line_start(),
                LineEnd => doc.line_end(),
                DocStart => doc.doc_start(),
                DocEnd => doc.doc_end(),
                NextTabStop => doc.next_tab_stop(TAB_WIDTH, machine.space()),
                ToggleOverwrite => doc.overwrite = !doc.overwrite,
                ToggleInverse | EscapePending => {}
                EditAction::None => return false,
                MoveUp | MoveDown | PageUp | PageDown => unreachable!(),
            }
        }
        self.scroll_to_cursor(layout);
        true
    }

    /// Keep the cursor on screen.
    pub fn scroll_to_cursor(&mut self, layout: &Layout) {
        let max_top = layout.total_rows(&self.doc).saturating_sub(1);
        self.top = layout.scroll_to_cursor(&self.doc, self.top.min(max_top));
    }

    /// Scroll by `delta` rows without moving the cursor (mouse wheel).
    pub fn scroll_by(&mut self, delta: isize, layout: &Layout) {
        let max_top = layout.total_rows(&self.doc).saturating_sub(layout.rows);
        self.top = (self.top as isize + delta).clamp(0, max_top as isize) as usize;
    }
}
