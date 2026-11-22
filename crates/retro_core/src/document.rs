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
    /// Identifies the current contents; every edit gets a new number, so
    /// undoing back to the saved contents makes the document unmodified.
    version: u64,
    saved_version: u64,
    last_version: u64,
}

/// A copy of a document's contents, for undo/redo.
#[derive(Debug, Clone)]
pub struct Snapshot {
    lines: Vec<Vec<u8>>,
    cursor: Pos,
    version: u64,
}

impl Snapshot {
    /// The document version this snapshot holds.
    pub fn version(&self) -> u64 {
        self.version
    }
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
            version: 0,
            saved_version: 0,
            last_version: 0,
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

    /// Identifies the current contents (changes on every edit).
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn is_modified(&self) -> bool {
        self.version != self.saved_version
    }

    pub fn mark_modified(&mut self) {
        self.touch();
    }

    pub fn mark_saved(&mut self) {
        self.saved_version = self.version;
    }

    /// Record that the contents changed.
    fn touch(&mut self) {
        self.last_version += 1;
        self.version = self.last_version;
    }

    fn cur_line_mut(&mut self) -> &mut Vec<u8> {
        self.touch();
        &mut self.lines[self.cursor.line]
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            lines: self.lines.clone(),
            cursor: self.cursor,
            version: self.version,
        }
    }

    pub fn restore(&mut self, s: Snapshot) {
        self.lines = s.lines;
        self.cursor = s.cursor;
        self.version = s.version;
    }

    /// The text between two positions (`a` before `b`), as lines.
    pub fn text_range(&self, a: Pos, b: Pos) -> Vec<Vec<u8>> {
        if a.line == b.line {
            return vec![self.lines[a.line][a.col..b.col].to_vec()];
        }
        let mut out = vec![self.lines[a.line][a.col..].to_vec()];
        out.extend(self.lines[a.line + 1..b.line].iter().cloned());
        out.push(self.lines[b.line][..b.col].to_vec());
        out
    }

    /// Delete the text between two positions (`a` before `b`), leaving the
    /// cursor at `a`.
    pub fn delete_range(&mut self, a: Pos, b: Pos) {
        self.touch();
        let tail = self.lines[b.line][b.col..].to_vec();
        self.lines.drain(a.line + 1..=b.line);
        let line = &mut self.lines[a.line];
        line.truncate(a.col);
        line.extend(tail);
        self.cursor = a;
    }

    /// Insert text (one or more lines) at the cursor, leaving the cursor
    /// after it. Always inserts, even in overwrite mode.
    pub fn insert_text(&mut self, text: &[Vec<u8>]) {
        let Some((last, first_lines)) = text.split_last() else {
            return;
        };
        let Pos { line, col } = self.cursor;
        self.touch();
        if first_lines.is_empty() {
            self.lines[line].splice(col..col, last.iter().copied());
            self.cursor.col += last.len();
            return;
        }
        let tail = self.lines[line].split_off(col);
        self.lines[line].extend_from_slice(&first_lines[0]);
        let mut new_lines: Vec<Vec<u8>> = first_lines[1..].to_vec();
        let mut end = last.clone();
        let end_col = end.len();
        end.extend(tail);
        new_lines.push(end);
        let n = new_lines.len();
        self.lines.splice(line + 1..line + 1, new_lines);
        self.cursor = Pos {
            line: line + n,
            col: end_col,
        };
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
            self.touch();
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
        self.touch();
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
        self.touch();
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

    /// Move to column `col` of the current line, padding with `space` if
    /// that is past the end of the line.
    pub fn move_to_col_padded(&mut self, col: usize, space: u8) {
        if col > self.lines[self.cursor.line].len() {
            self.cur_line_mut().resize(col, space);
        }
        self.cursor.col = col;
    }

    /// Move to the start of the next line, if there is one.
    pub fn next_line_start(&mut self) {
        if self.cursor.line + 1 < self.lines.len() {
            self.cursor = Pos {
                line: self.cursor.line + 1,
                col: 0,
            };
        }
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
    fn moving_past_line_end_pads_with_spaces() {
        let mut d = doc(&["AB", "C"]);
        d.set_cursor(Pos { line: 0, col: 2 });
        d.move_to_col_padded(8, b' ');
        assert_eq!(d.cursor().col, 8);
        assert_eq!(d.line(0), b"AB      ");
        d.move_to_col_padded(3, b' ');
        assert_eq!(d.line(0).len(), 8);
        d.next_line_start();
        assert_eq!(d.cursor(), Pos { line: 1, col: 0 });
        d.next_line_start();
        assert_eq!(d.cursor(), Pos { line: 1, col: 0 });
    }

    #[test]
    fn ranges_copy_delete_and_insert() {
        let mut d = doc(&["ONE", "TWO", "THREE"]);
        let (a, b) = (Pos { line: 0, col: 1 }, Pos { line: 2, col: 2 });
        let t = d.text_range(a, b);
        assert_eq!(t, vec![b"NE".to_vec(), b"TWO".to_vec(), b"TH".to_vec()]);
        d.delete_range(a, b);
        assert_eq!(text(&d), ["OREE"]);
        assert_eq!(d.cursor(), a);
        d.insert_text(&t);
        assert_eq!(text(&d), ["ONE", "TWO", "THREE"]);
        assert_eq!(d.cursor(), b);
        d.set_cursor(Pos { line: 1, col: 1 });
        d.insert_text(&[b"XY".to_vec()]);
        assert_eq!(text(&d), ["ONE", "TXYWO", "THREE"]);
        assert_eq!(d.cursor(), Pos { line: 1, col: 3 });
    }

    #[test]
    fn modified_tracks_saved_version() {
        let mut d = doc(&["A"]);
        assert!(!d.is_modified());
        let before = d.snapshot();
        d.type_code(b'B');
        assert!(d.is_modified());
        d.restore(before);
        assert!(!d.is_modified());
        d.type_code(b'C');
        d.mark_saved();
        assert!(!d.is_modified());
    }

    #[test]
    fn set_cursor_clamps() {
        let mut d = doc(&["AB"]);
        d.set_cursor(Pos { line: 9, col: 9 });
        assert_eq!(d.cursor(), Pos { line: 0, col: 2 });
    }
}
