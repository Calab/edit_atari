//! Editor state: a document plus scrolling, selection and undo history,
//! applying [`EditAction`]s.

use crate::document::{Document, Pos, Snapshot};
use crate::input::EditAction;
use crate::layout::{Layout, WrapMode};
use crate::machine::Machine;
use crate::tabs::TabStops;

/// Undo steps kept; older ones are dropped.
const MAX_UNDO: usize = 1000;

/// Consecutive edits of the same kind are undone together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    Typing,
    Deleting,
}

#[derive(Debug, Clone)]
pub struct Editor {
    pub doc: Document,
    pub wrap: WrapMode,
    /// First visible screen row.
    pub top: usize,
    pub tabs: TabStops,
    /// The other end of the selection; the cursor is one end.
    anchor: Option<Pos>,
    /// Column to aim for when moving up/down through shorter lines.
    want_x: Option<usize>,
    /// A short message for the status bar, taken by the front end.
    note: Option<String>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// The kind of the edit group in progress, if any.
    group: Option<Group>,
}

impl Editor {
    pub fn new(doc: Document, wrap: WrapMode, tabs: TabStops) -> Self {
        Self {
            doc,
            wrap,
            top: 0,
            tabs,
            anchor: None,
            want_x: None,
            note: None,
            undo: Vec::new(),
            redo: Vec::new(),
            group: None,
        }
    }

    /// Take the pending status-bar message, if any.
    pub fn take_note(&mut self) -> Option<String> {
        self.note.take()
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

    // ----- selection -------------------------------------------------------

    /// The selected range, start before end, if anything is selected.
    pub fn selection(&self) -> Option<(Pos, Pos)> {
        let a = self.anchor?;
        let c = self.doc.cursor();
        match a.cmp(&c) {
            std::cmp::Ordering::Less => Some((a, c)),
            std::cmp::Ordering::Greater => Some((c, a)),
            std::cmp::Ordering::Equal => None,
        }
    }

    pub fn has_selection(&self) -> bool {
        self.selection().is_some()
    }

    pub fn select_all(&mut self) {
        self.anchor = Some(Pos::default());
        self.doc.doc_end();
        self.group = None;
    }

    pub fn clear_selection(&mut self) {
        self.anchor = None;
    }

    /// Put the cursor at `pos` (a mouse click). With `extend`, the selection
    /// grows from its anchor (or the old cursor) to `pos` (shift-click or drag).
    pub fn click(&mut self, pos: Pos, extend: bool) {
        if extend {
            self.anchor.get_or_insert(self.doc.cursor());
        } else {
            self.anchor = None;
        }
        self.doc.set_cursor(pos);
        self.want_x = None;
        self.group = None;
    }

    /// Start a selection at the cursor if there isn't one (mouse press).
    pub fn begin_drag(&mut self) {
        self.anchor = Some(self.doc.cursor());
    }

    fn delete_selection(&mut self) -> bool {
        match self.selection() {
            Some((a, b)) => {
                self.doc.delete_range(a, b);
                self.anchor = None;
                true
            }
            None => false,
        }
    }

    // ----- clipboard -------------------------------------------------------

    /// The selected text as lines, if anything is selected.
    pub fn copy(&self) -> Option<Vec<Vec<u8>>> {
        self.selection().map(|(a, b)| self.doc.text_range(a, b))
    }

    /// Remove the selected text and return it.
    pub fn cut(&mut self, layout: &Layout) -> Option<Vec<Vec<u8>>> {
        let text = self.copy()?;
        self.checkpoint(None);
        self.delete_selection();
        self.scroll_to_cursor(layout);
        Some(text)
    }

    /// Replace the selection (if any) with `text`.
    pub fn paste(&mut self, text: &[Vec<u8>], layout: &Layout) {
        self.checkpoint(None);
        self.delete_selection();
        self.doc.insert_text(text);
        self.anchor = None;
        self.scroll_to_cursor(layout);
    }

    /// Select from `a` to `b` (the cursor ends at `b`) and show it.
    pub fn select_range(&mut self, a: Pos, b: Pos, layout: &Layout) {
        self.anchor = Some(a);
        self.doc.set_cursor(b);
        self.want_x = None;
        self.group = None;
        self.scroll_to_cursor(layout);
    }

    /// Replace `len` codes at each of `at` (non-overlapping, in document
    /// order, each within one line) with `with`, as one undo step. The
    /// cursor ends after the last replacement.
    pub fn replace_matches(&mut self, at: &[Pos], len: usize, with: &[u8], layout: &Layout) {
        let Some(&last) = at.last() else { return };
        self.checkpoint(None);
        self.anchor = None;
        // From the end, so earlier positions stay valid.
        for &p in at.iter().rev() {
            self.doc.delete_range(
                p,
                Pos {
                    line: p.line,
                    col: p.col + len,
                },
            );
            self.doc.set_cursor(p);
            self.doc.insert_text(&[with.to_vec()]);
        }
        let shift: usize = at.iter().filter(|p| p.line == last.line).count();
        let col = last.col + shift * with.len() - (shift - 1) * len;
        self.doc.set_cursor(Pos {
            line: last.line,
            col,
        });
        self.scroll_to_cursor(layout);
    }

    // ----- undo ------------------------------------------------------------

    /// Save the state before an edit. Edits in the same `group` as the
    /// previous one join its undo step; `None` always starts a new step.
    fn checkpoint(&mut self, group: Option<Group>) {
        if group.is_none() || group != self.group {
            self.undo.push(self.doc.snapshot());
            if self.undo.len() > MAX_UNDO {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
        self.group = group;
    }

    pub fn can_undo(&self) -> bool {
        self.undo.iter().any(|s| s.version() != self.doc.version())
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self, layout: &Layout) -> bool {
        // Skip steps recorded before actions that turned out not to change
        // anything (e.g. a Tab that only moved the cursor).
        while let Some(s) = self.undo.pop() {
            if s.version() != self.doc.version() {
                self.redo.push(self.doc.snapshot());
                self.doc.restore(s);
                self.after_history(layout);
                return true;
            }
        }
        false
    }

    pub fn redo(&mut self, layout: &Layout) -> bool {
        let Some(s) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.doc.snapshot());
        self.doc.restore(s);
        self.after_history(layout);
        true
    }

    fn after_history(&mut self, layout: &Layout) {
        self.anchor = None;
        self.group = None;
        self.want_x = None;
        self.scroll_to_cursor(layout);
    }

    // ----- actions ---------------------------------------------------------

    /// Apply an action. With `extend`, cursor movements extend the selection
    /// (Shift+arrows). Returns true if anything may have changed on screen.
    pub fn apply(
        &mut self,
        action: EditAction,
        layout: &Layout,
        machine: &dyn Machine,
        extend: bool,
    ) -> bool {
        use EditAction::*;
        let movement = matches!(
            action,
            MoveUp
                | MoveDown
                | PageUp
                | PageDown
                | MoveLeft
                | MoveRight
                | LineStart
                | LineEnd
                | DocStart
                | DocEnd
        );
        if movement {
            self.group = Option::None;
            if extend {
                self.anchor.get_or_insert(self.doc.cursor());
            } else if let Some((a, b)) = self.selection() {
                // Left/right without Shift collapse the selection to its edge.
                self.anchor = Option::None;
                match action {
                    MoveLeft => return self.set_and_scroll(a, layout),
                    MoveRight => return self.set_and_scroll(b, layout),
                    _ => {}
                }
            } else {
                self.anchor = Option::None;
            }
        }

        let vertical = matches!(action, MoveUp | MoveDown | PageUp | PageDown);
        if vertical {
            let doc = &mut self.doc;
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
            self.scroll_to_cursor(layout);
            return true;
        }

        self.want_x = Option::None;
        // Any edit ends the selection (even an empty one left by a click).
        if matches!(
            action,
            Insert(_) | NewLine | Backspace | DeleteForward | InsertSpace
        ) && !self.has_selection()
        {
            self.anchor = Option::None;
        }
        match action {
            Insert(code) => {
                self.checkpoint(Some(Group::Typing));
                self.delete_selection();
                self.doc.type_code(code);
            }
            NewLine => {
                self.checkpoint(Some(Group::Typing));
                self.delete_selection();
                self.doc.new_line();
            }
            Backspace | DeleteForward => {
                self.checkpoint(Some(Group::Deleting));
                if !self.delete_selection() {
                    if action == Backspace {
                        self.doc.backspace();
                    } else {
                        self.doc.delete_forward();
                    }
                }
            }
            InsertSpace => {
                self.checkpoint(Some(Group::Typing));
                self.doc.insert_space(machine.space());
            }
            DeleteLine => {
                self.checkpoint(Option::None);
                self.anchor = Option::None;
                self.doc.delete_line();
            }
            InsertLine => {
                self.checkpoint(Option::None);
                self.anchor = Option::None;
                self.doc.insert_line();
            }
            NextTabStop => {
                self.checkpoint(Some(Group::Typing));
                self.anchor = Option::None;
                match self.tabs.next(self.doc.cursor().col) {
                    Some(col) => self.doc.move_to_col_padded(col, machine.space()),
                    // No more stops in the logical line: go to the next line.
                    Option::None => self.doc.next_line_start(),
                }
            }
            SetTabStop | ClearTabStop => {
                let col = self.doc.cursor().col;
                let set = action == SetTabStop;
                let ok = if set {
                    self.tabs.set(col)
                } else {
                    self.tabs.clear(col)
                };
                self.note = Some(if !ok {
                    "No tab stops past the logical line".into()
                } else if set {
                    format!("Tab stop set at column {}", col + 1)
                } else {
                    format!("Tab stop cleared at column {}", col + 1)
                });
            }
            MoveLeft => self.doc.move_left(),
            MoveRight => self.doc.move_right(),
            LineStart => self.doc.line_start(),
            LineEnd => self.doc.line_end(),
            DocStart => self.doc.doc_start(),
            DocEnd => self.doc.doc_end(),
            ToggleOverwrite => self.doc.overwrite = !self.doc.overwrite,
            ToggleInverse | EscapePending => {}
            EditAction::None => return false,
            MoveUp | MoveDown | PageUp | PageDown => unreachable!(),
        }
        self.scroll_to_cursor(layout);
        true
    }

    fn set_and_scroll(&mut self, pos: Pos, layout: &Layout) -> bool {
        self.doc.set_cursor(pos);
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
