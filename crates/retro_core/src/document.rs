//! A text document stored as lines of 8-bit machine codes.

/// A cursor position: line index and byte column within that line.
/// `col` may equal the line length (cursor after the last character).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone)]
pub struct Document {
    lines: Vec<Vec<u8>>,
    cursor: Pos,
    /// Whether the last line ends with a line terminator on disk.
    pub final_eol: bool,
    /// Overwrite (true) or insert (false) typing mode.
    pub overwrite: bool,
    modified: bool,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    pub fn new() -> Self {
        Self::from_lines(Vec::new(), true)
    }

    /// Build from decoded lines. An empty list becomes one empty line.
    pub fn from_lines(mut lines: Vec<Vec<u8>>, final_eol: bool) -> Self {
        if lines.is_empty() {
            lines.push(Vec::new());
        }
        Self {
            lines,
            cursor: Pos::default(),
            final_eol,
            overwrite: false,
            modified: false,
        }
    }

    pub fn lines(&self) -> &[Vec<u8>] {
        &self.lines
    }

    pub fn line(&self, i: usize) -> &[u8] {
        &self.lines[i]
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn cursor(&self) -> Pos {
        self.cursor
    }

    /// Move the cursor, clamping to valid positions.
    pub fn set_cursor(&mut self, pos: Pos) {
        let line = pos.line.min(self.lines.len() - 1);
        let col = pos.col.min(self.lines[line].len());
        self.cursor = Pos { line, col };
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn mark_modified(&mut self) {
        self.modified = true;
    }

    pub fn mark_saved(&mut self) {
        self.modified = false;
    }

    fn cur_line_mut(&mut self) -> &mut Vec<u8> {
        self.modified = true;
        &mut self.lines[self.cursor.line]
    }

    /// Type a code at the cursor, honouring insert/overwrite mode.
    pub fn type_code(&mut self, code: u8) {
        let Pos { col, .. } = self.cursor;
        let overwrite = self.overwrite;
        let line = self.cur_line_mut();
        if overwrite && col < line.len() {
            line[col] = code;
        } else {
            line.insert(col, code);
        }
        self.cursor.col += 1;
    }

    /// Split the line at the cursor.
    pub fn new_line(&mut self) {
        let Pos { line, col } = self.cursor;
        let rest = self.cur_line_mut().split_off(col);
        self.lines.insert(line + 1, rest);
        self.cursor = Pos {
            line: line + 1,
            col: 0,
        };
    }

    /// Delete the character left of the cursor, joining lines at column 0.
    pub fn backspace(&mut self) {
        let Pos { line, col } = self.cursor;
        if col > 0 {
            self.cur_line_mut().remove(col - 1);
            self.cursor.col -= 1;
        } else if line > 0 {
            let removed = self.lines.remove(line);
            self.modified = true;
            let prev = &mut self.lines[line - 1];
            let new_col = prev.len();
            prev.extend(removed);
            self.cursor = Pos {
                line: line - 1,
                col: new_col,
            };
        }
    }

    /// Delete the character under the cursor, joining the next line at the end.
    pub fn delete_forward(&mut self) {
        let Pos { line, col } = self.cursor;
        if col < self.lines[line].len() {
            self.cur_line_mut().remove(col);
        } else if line + 1 < self.lines.len() {
            let next = self.lines.remove(line + 1);
            self.cur_line_mut().extend(next);
        }
    }

    /// Insert a blank at the cursor without moving it (Atari "insert char").
    pub fn insert_space(&mut self, space: u8) {
        let col = self.cursor.col;
        self.cur_line_mut().insert(col, space);
    }

    /// Delete the whole current line (Atari SHIFT+DELETE).
    pub fn delete_line(&mut self) {
        self.modified = true;
        if self.lines.len() == 1 {
            self.lines[0].clear();
        } else {
            self.lines.remove(self.cursor.line);
        }
        let line = self.cursor.line.min(self.lines.len() - 1);
        self.cursor = Pos { line, col: 0 };
    }

    /// Insert an empty line above the current one (Atari SHIFT+INSERT).
    pub fn insert_line(&mut self) {
        self.modified = true;
        self.lines.insert(self.cursor.line, Vec::new());
        self.cursor.col = 0;
    }

    pub fn move_left(&mut self) {
        let Pos { line, col } = self.cursor;
        if col > 0 {
            self.cursor.col -= 1;
        } else if line > 0 {
            self.cursor = Pos {
                line: line - 1,
                col: self.lines[line - 1].len(),
            };
        }
    }

    pub fn move_right(&mut self) {
        let Pos { line, col } = self.cursor;
        if col < self.lines[line].len() {
            self.cursor.col += 1;
        } else if line + 1 < self.lines.len() {
            self.cursor = Pos {
                line: line + 1,
                col: 0,
            };
        }
    }

    pub fn line_start(&mut self) {
        self.cursor.col = 0;
    }

    pub fn line_end(&mut self) {
        self.cursor.col = self.lines[self.cursor.line].len();
    }

    pub fn doc_start(&mut self) {
        self.cursor = Pos::default();
    }

    pub fn doc_end(&mut self) {
        let line = self.lines.len() - 1;
        self.cursor = Pos {
            line,
            col: self.lines[line].len(),
        };
    }

    /// Move to the next multiple of `width` columns, padding with `space`
    /// if that is past the end of the line.
    pub fn next_tab_stop(&mut self, width: usize, space: u8) {
        let target = (self.cursor.col / width + 1) * width;
        let len = self.lines[self.cursor.line].len();
        if target > len {
            self.cur_line_mut().resize(target, space);
        }
        self.cursor.col = target;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(lines: &[&str]) -> Document {
        Document::from_lines(lines.iter().map(|l| l.as_bytes().to_vec()).collect(), true)
    }

    fn text(d: &Document) -> Vec<String> {
        d.lines()
            .iter()
            .map(|l| String::from_utf8_lossy(l).into_owned())
            .collect()
    }

    #[test]
    fn typing_inserts_and_overwrites() {
        let mut d = doc(&["AC"]);
        d.set_cursor(Pos { line: 0, col: 1 });
        d.type_code(b'B');
        assert_eq!(text(&d), ["ABC"]);
        d.overwrite = true;
        d.type_code(b'X');
        assert_eq!(text(&d), ["ABX"]);
        d.type_code(b'Y');
        assert_eq!(text(&d), ["ABXY"]);
        assert!(d.is_modified());
    }

    #[test]
    fn newline_and_backspace_round_trip() {
        let mut d = doc(&["HELLO"]);
        d.set_cursor(Pos { line: 0, col: 2 });
        d.new_line();
        assert_eq!(text(&d), ["HE", "LLO"]);
        assert_eq!(d.cursor(), Pos { line: 1, col: 0 });
        d.backspace();
        assert_eq!(text(&d), ["HELLO"]);
        assert_eq!(d.cursor(), Pos { line: 0, col: 2 });
    }

    #[test]
    fn delete_forward_joins_lines() {
        let mut d = doc(&["AB", "CD"]);
        d.set_cursor(Pos { line: 0, col: 2 });
        d.delete_forward();
        assert_eq!(text(&d), ["ABCD"]);
    }

    #[test]
    fn delete_and_insert_line() {
        let mut d = doc(&["ONE", "TWO", "THREE"]);
        d.set_cursor(Pos { line: 2, col: 3 });
        d.delete_line();
        assert_eq!(text(&d), ["ONE", "TWO"]);
        assert_eq!(d.cursor(), Pos { line: 1, col: 0 });
        d.insert_line();
        assert_eq!(text(&d), ["ONE", "", "TWO"]);
        let mut single = doc(&["X"]);
        single.delete_line();
        assert_eq!(text(&single), [""]);
    }

    #[test]
    fn cursor_wraps_across_lines() {
        let mut d = doc(&["AB", "C"]);
        d.set_cursor(Pos { line: 0, col: 2 });
        d.move_right();
        assert_eq!(d.cursor(), Pos { line: 1, col: 0 });
        d.move_left();
        assert_eq!(d.cursor(), Pos { line: 0, col: 2 });
    }

    #[test]
    fn tab_pads_with_spaces() {
        let mut d = doc(&["AB"]);
        d.set_cursor(Pos { line: 0, col: 2 });
        d.next_tab_stop(8, b' ');
        assert_eq!(d.cursor().col, 8);
        assert_eq!(d.line(0).len(), 8);
    }

    #[test]
    fn set_cursor_clamps() {
        let mut d = doc(&["AB"]);
        d.set_cursor(Pos { line: 9, col: 9 });
        assert_eq!(d.cursor(), Pos { line: 0, col: 2 });
    }
}
