//! Wrapping document lines into screen rows.
//!
//! A line of length `n` wrapped at `w` columns occupies `n / w + 1` rows, so
//! the cursor always has a cell to sit in after the last character -- the same
//! rule the Atari screen editor uses for logical lines.

use crate::document::{Document, Pos};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapMode {
    /// The machine's native screen width (40 columns, with its left margin).
    Native,
    /// Wrap at the width of the window, like a PC notepad.
    Window,
}

/// Screen geometry for one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// Columns available for text on each row.
    pub text_cols: usize,
    /// Blank columns before the text.
    pub left_margin: usize,
    /// Text rows visible on screen.
    pub rows: usize,
}

impl Layout {
    /// Compute the layout for a window `win_cols` x `win_rows` cells.
    pub fn new(
        mode: WrapMode,
        win_cols: usize,
        win_rows: usize,
        native_cols: usize,
        native_margin: usize,
    ) -> Self {
        let (text_cols, left_margin) = match mode {
            WrapMode::Native => (native_cols - native_margin, native_margin),
            WrapMode::Window => (win_cols.max(2) - 1, 1),
        };
        Self {
            text_cols: text_cols.max(1),
            left_margin,
            rows: win_rows.max(1),
        }
    }

    /// Screen rows a line of `len` characters occupies.
    pub fn rows_for_len(&self, len: usize) -> usize {
        len / self.text_cols + 1
    }

    /// Total screen rows for the whole document.
    pub fn total_rows(&self, doc: &Document) -> usize {
        doc.lines().iter().map(|l| self.rows_for_len(l.len())).sum()
    }

    /// Absolute screen row (from the top of the document) and x of a position.
    pub fn locate(&self, doc: &Document, pos: Pos) -> (usize, usize) {
        let before: usize = doc.lines()[..pos.line]
            .iter()
            .map(|l| self.rows_for_len(l.len()))
            .sum();
        (before + pos.col / self.text_cols, pos.col % self.text_cols)
    }

    /// Document position shown at absolute screen row `row`, column `x`.
    /// Clamps to the end of the line, or of the document.
    pub fn position_at(&self, doc: &Document, row: usize, x: usize) -> Pos {
        let mut remaining = row;
        for (i, l) in doc.lines().iter().enumerate() {
            let rows = self.rows_for_len(l.len());
            if remaining < rows {
                let col = (remaining * self.text_cols + x.min(self.text_cols - 1)).min(l.len());
                return Pos { line: i, col };
            }
            remaining -= rows;
        }
        let last = doc.line_count() - 1;
        Pos {
            line: last,
            col: doc.line(last).len(),
        }
    }

    /// Cursor position `delta` screen rows away, aiming for column `want_x`.
    pub fn move_rows(&self, doc: &Document, pos: Pos, delta: isize, want_x: usize) -> Pos {
        let (row, _) = self.locate(doc, pos);
        let last = self.total_rows(doc) - 1;
        let target = (row as isize + delta).clamp(0, last as isize) as usize;
        self.position_at(doc, target, want_x)
    }

    /// Adjust `top` (first visible row) so that the cursor row is on screen.
    pub fn scroll_to_cursor(&self, doc: &Document, top: usize) -> usize {
        let (row, _) = self.locate(doc, doc.cursor());
        if row < top {
            row
        } else if row >= top + self.rows {
            row + 1 - self.rows
        } else {
            top
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc_with_lens(lens: &[usize]) -> Document {
        Document::from_lines(lens.iter().map(|&n| vec![b'A'; n]).collect(), true)
    }

    fn native() -> Layout {
        Layout::new(WrapMode::Native, 80, 24, 40, 2)
    }

    #[test]
    fn native_mode_has_38_text_columns() {
        let l = native();
        assert_eq!((l.text_cols, l.left_margin), (38, 2));
        assert_eq!(l.rows_for_len(0), 1);
        assert_eq!(l.rows_for_len(37), 1);
        assert_eq!(l.rows_for_len(38), 2); // cursor wraps to the next row
        assert_eq!(l.rows_for_len(114), 4);
    }

    #[test]
    fn locate_and_position_are_inverse() {
        let l = native();
        let d = doc_with_lens(&[10, 50, 0]);
        let p = Pos { line: 1, col: 45 };
        let (row, x) = l.locate(&d, p);
        assert_eq!((row, x), (2, 7));
        assert_eq!(l.position_at(&d, row, x), p);
    }

    #[test]
    fn moving_down_keeps_column_and_clamps() {
        let l = native();
        let d = doc_with_lens(&[30, 5]);
        let p = l.move_rows(&d, Pos { line: 0, col: 20 }, 1, 20);
        assert_eq!(p, Pos { line: 1, col: 5 });
        let p = l.move_rows(&d, p, 5, 20);
        assert_eq!(p, Pos { line: 1, col: 5 });
    }

    #[test]
    fn window_mode_uses_window_width() {
        let l = Layout::new(WrapMode::Window, 100, 30, 40, 2);
        assert_eq!((l.text_cols, l.left_margin), (99, 1));
    }

    #[test]
    fn scrolling_follows_cursor() {
        let l = Layout::new(WrapMode::Native, 40, 3, 40, 2);
        let mut d = doc_with_lens(&[1, 1, 1, 1, 1]);
        d.set_cursor(Pos { line: 4, col: 0 });
        assert_eq!(l.scroll_to_cursor(&d, 0), 2);
        d.set_cursor(Pos { line: 1, col: 0 });
        assert_eq!(l.scroll_to_cursor(&d, 2), 1);
    }
}
