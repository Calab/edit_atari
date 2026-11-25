//! The Open from Image page (File > Open from Image): the image's
//! directories on the left, the chosen directory's files on the right,
//! names in the machine's own characters.
//!
//! The page holds a listing of the whole image, made when the image is
//! chosen; it never keeps the image open. Opening a file reads the image
//! file again.

use std::path::PathBuf;

use retro_core::disk::{DateTime, DiskEntry, DiskVolume};
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas, Texture};
use sdl2::video::Window;

use crate::menu::THEME;
use crate::render::Grid;
use crate::ui_font::UiFont;
use crate::widgets::{Op, Ui};

/// The controls, in Tab order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Tree,
    Files,
    Choose,
    Open,
    Cancel,
}

const ORDER: [Field; 5] = [
    Field::Tree,
    Field::Files,
    Field::Choose,
    Field::Open,
    Field::Cancel,
];

const TITLE: &str = "Open from Image";
const CHOOSE: &str = "Choose Image...";
const OPEN: &str = "Open";
const CANCEL: &str = "Cancel";

/// Deepest directory listed (a damaged disk could loop).
const MAX_DEPTH: usize = 16;

/// One directory in the tree.
pub struct TreeRow {
    /// Its path in the image (empty for the top directory).
    pub path: Vec<Vec<u8>>,
    /// Its name in machine codes (the top: the machine's sign for it).
    pub label: Vec<u8>,
    pub depth: usize,
    /// Its entries, or why they can't be listed.
    pub entries: Result<Vec<DiskEntry>, String>,
}

/// What a click or key asks the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    /// Choose another image (the OS file dialog).
    Choose,
    /// Open the selected file.
    Open,
    Close,
}

pub struct ImagePage {
    /// The image shown, if one has been chosen.
    pub image: Option<PathBuf>,
    /// "SpartaDOS, 720 sectors of 256 bytes, 312 free".
    pub info: String,
    /// Why the image can't be shown, instead of `info`.
    pub error: Option<String>,
    pub tree: Vec<TreeRow>,
    pub has_dates: bool,
    /// The directory shown (an index into `tree`).
    pub dir: usize,
    /// The selected entry of that directory.
    pub selected: usize,
    focus: usize,
    tree_top: usize,
    files_top: usize,
    /// Rows each list shows, from the last draw (for Page Up/Down).
    tree_rows: usize,
    files_rows: usize,
    hits: Vec<(Rect, Field, Op)>,
    /// Each visible list row: where, which list, which row.
    rows: Vec<(Rect, Field, usize)>,
    /// The directory list's box, for the mouse wheel.
    tree_area: Rect,
    /// Each list's scroll arrows that can be clicked: where, which list,
    /// which way (-1 up, 1 down).
    arrows: Vec<(Rect, Field, isize)>,
    /// Scroll the lists to show the selections at the next draw (after a
    /// key, not after the wheel or the arrows).
    follow: bool,
}

impl ImagePage {
    pub fn new() -> Self {
        Self {
            image: None,
            info: String::new(),
            error: None,
            tree: Vec::new(),
            has_dates: false,
            dir: 0,
            selected: 0,
            focus: 1,
            tree_top: 0,
            files_top: 0,
            tree_rows: 1,
            files_rows: 1,
            hits: Vec::new(),
            rows: Vec::new(),
            tree_area: Rect::new(0, 0, 1, 1),
            arrows: Vec::new(),
            follow: true,
        }
    }

    /// Show `vol`, read from `image`. `top` is how the top directory is
    /// shown, in machine codes.
    pub fn load(&mut self, image: PathBuf, vol: &dyn DiskVolume, top: Vec<u8>) {
        self.image = Some(image);
        self.error = None;
        self.info = vol.info();
        if vol.write_protected() {
            self.info.push_str(", write-protected");
        }
        self.has_dates = vol.has_dates();
        self.tree.clear();
        self.add_dir(vol, Vec::new(), top, 0);
        self.dir = 0;
        self.selected = 0;
        self.tree_top = 0;
        self.files_top = 0;
        self.focus = 1;
        self.follow = true;
    }

    /// The image couldn't be read or opened.
    pub fn failed(&mut self, image: PathBuf, why: String) {
        self.image = Some(image);
        self.error = Some(why);
        self.info.clear();
        self.tree.clear();
        self.dir = 0;
        self.selected = 0;
        self.focus = ORDER.iter().position(|&f| f == Field::Choose).unwrap();
    }

    fn add_dir(&mut self, vol: &dyn DiskVolume, path: Vec<Vec<u8>>, label: Vec<u8>, depth: usize) {
        let entries = vol.list(&path).map_err(|e| e.message);
        let subdirs: Vec<Vec<u8>> = match &entries {
            Ok(list) if depth < MAX_DEPTH => list
                .iter()
                .filter(|e| e.is_dir)
                .map(|e| e.name.clone())
                .collect(),
            _ => Vec::new(),
        };
        self.tree.push(TreeRow {
            path: path.clone(),
            label,
            depth,
            entries,
        });
        for name in subdirs {
            let mut sub = path.clone();
            sub.push(name.clone());
            self.add_dir(vol, sub, name, depth + 1);
        }
    }

    /// Show the directory at `path` with `name` selected, if they're there
    /// (the document's own place, when it came from this image).
    pub fn select_path(&mut self, dir: &[Vec<u8>], name: &[u8]) {
        if let Some(d) = self.tree.iter().position(|t| t.path == dir) {
            self.show_dir(d);
            if let Some(i) = self.entries().iter().position(|e| e.name == name) {
                self.selected = i;
            }
        }
        self.follow = true;
    }

    /// The shown directory's entries (none if it can't be listed).
    pub fn entries(&self) -> &[DiskEntry] {
        match self.tree.get(self.dir).map(|t| &t.entries) {
            Some(Ok(list)) => list,
            _ => &[],
        }
    }

    /// The selected entry's path in the image, if it's a file.
    pub fn selected_file(&self) -> Option<Vec<Vec<u8>>> {
        let e = self.entries().get(self.selected)?;
        if e.is_dir {
            return None;
        }
        let mut path = self.tree[self.dir].path.clone();
        path.push(e.name.clone());
        Some(path)
    }

    fn show_dir(&mut self, d: usize) {
        self.follow = true;
        if d < self.tree.len() && d != self.dir {
            self.dir = d;
            self.selected = 0;
            self.files_top = 0;
        }
    }

    /// Enter the selected directory, or ask to open the selected file.
    fn activate(&mut self) -> Action {
        let Some(e) = self.entries().get(self.selected) else {
            return Action::None;
        };
        if !e.is_dir {
            return Action::Open;
        }
        let mut path = self.tree[self.dir].path.clone();
        path.push(e.name.clone());
        if let Some(d) = self.tree.iter().position(|t| t.path == path) {
            self.show_dir(d);
        }
        Action::None
    }

    /// Up to the parent directory, selecting the one just left.
    fn parent(&mut self) {
        let path = &self.tree[self.dir].path;
        let Some((name, up)) = path.split_last() else {
            return;
        };
        let (name, up) = (name.clone(), up.to_vec());
        self.select_path(&up, &name);
    }

    pub fn focused(&self) -> Field {
        ORDER[self.focus]
    }

    fn focus_on(&mut self, f: Field) {
        self.focus = ORDER.iter().position(|&o| o == f).unwrap_or(0);
    }

    /// A key while the page is open; every key is used up.
    pub fn key(&mut self, k: sdl2::keyboard::Keycode, shift: bool) -> Action {
        use sdl2::keyboard::Keycode as K;
        if k == K::ESCAPE {
            return Action::Close;
        }
        if k == K::TAB {
            let n = ORDER.len() as isize;
            let step = if shift { -1 } else { 1 };
            self.focus = (self.focus as isize + step).rem_euclid(n) as usize;
            return Action::None;
        }
        let f = self.focused();
        let press = matches!(k, K::RETURN | K::KP_ENTER | K::SPACE);
        self.follow = true;
        match f {
            Field::Choose if press => return Action::Choose,
            Field::Open if press => return self.activate(),
            Field::Cancel if press => return Action::Close,
            Field::Tree => {
                let n = self.tree.len();
                let to = match k {
                    K::UP => self.dir.checked_sub(1),
                    K::DOWN => Some(self.dir + 1),
                    K::HOME => Some(0),
                    K::END => n.checked_sub(1),
                    K::PAGEUP => Some(self.dir.saturating_sub(self.tree_rows)),
                    K::PAGEDOWN => Some(self.dir + self.tree_rows),
                    K::RIGHT | K::RETURN | K::KP_ENTER => {
                        self.focus_on(Field::Files);
                        None
                    }
                    _ => None,
                };
                if let Some(d) = to.filter(|_| n > 0) {
                    self.show_dir(d.min(n - 1));
                }
            }
            Field::Files => {
                let n = self.entries().len();
                let to = match k {
                    K::UP => self.selected.checked_sub(1),
                    K::DOWN => Some(self.selected + 1),
                    K::HOME => Some(0),
                    K::END => n.checked_sub(1),
                    K::PAGEUP => Some(self.selected.saturating_sub(self.files_rows)),
                    K::PAGEDOWN => Some(self.selected + self.files_rows),
                    K::RETURN | K::KP_ENTER => return self.activate(),
                    K::BACKSPACE | K::LEFT => {
                        self.parent();
                        None
                    }
                    _ => None,
                };
                if let Some(i) = to.filter(|_| n > 0) {
                    self.selected = i.min(n - 1);
                }
            }
            _ => {}
        }
        Action::None
    }

    /// A left click; `double` for a double click.
    pub fn click(&mut self, x: i32, y: i32, double: bool) -> Action {
        if let Some(&(_, list, step)) = self.arrows.iter().find(|(r, ..)| r.contains_point((x, y)))
        {
            self.scroll(list, step);
            return Action::None;
        }
        if let Some(&(_, list, row)) = self.rows.iter().find(|(r, ..)| r.contains_point((x, y))) {
            self.focus_on(list);
            match list {
                Field::Tree => self.show_dir(row),
                _ => {
                    self.selected = row;
                    if double {
                        return self.activate();
                    }
                }
            }
            return Action::None;
        }
        if let Some(&(_, f, _)) = self.hits.iter().find(|(r, ..)| r.contains_point((x, y))) {
            self.focus_on(f);
            return match f {
                Field::Choose => Action::Choose,
                Field::Open => self.activate(),
                Field::Cancel => Action::Close,
                _ => Action::None,
            };
        }
        Action::None
    }

    /// The mouse wheel: scroll the list under the pointer (`dy` notches,
    /// up positive).
    pub fn wheel(&mut self, x: i32, y: i32, dy: i32) {
        // Over the directories: those. Anywhere else: the files.
        let list = if self.tree_area.contains_point((x, y)) {
            Field::Tree
        } else {
            Field::Files
        };
        self.scroll(list, -dy as isize * 3);
    }

    /// Scroll a list by `step` rows (down positive), leaving the
    /// selection where it is.
    fn scroll(&mut self, list: Field, step: isize) {
        let (top, len, rows) = match list {
            Field::Tree => (&mut self.tree_top, self.tree.len(), self.tree_rows),
            _ => {
                let n = match self.tree.get(self.dir).map(|t| &t.entries) {
                    Some(Ok(l)) => l.len(),
                    _ => 0,
                };
                (&mut self.files_top, n, self.files_rows)
            }
        };
        let max = len.saturating_sub(rows) as isize;
        *top = (*top as isize + step).clamp(0, max) as usize;
    }

    /// Draw the page over the window, names in `cell`-pixel characters
    /// from `atlas`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        canvas: &mut Canvas<Window>,
        font: &mut UiFont,
        atlas: &mut Texture,
        (width, height): (u32, u32),
        pad: i32,
        cell: i32,
        warning: Color,
    ) {
        let t = &THEME;
        canvas.set_blend_mode(BlendMode::BLEND);
        canvas.set_draw_color(Color::RGBA(0, 0, 0, 90));
        let _ = canvas.fill_rect(None);
        canvas.set_blend_mode(BlendMode::NONE);

        let lh = font.line_height();
        let row_h = lh.max(cell) + pad;
        let button = |font: &mut UiFont, s: &str| font.measure(s) + pad * 4;
        let digit = font.measure("0");
        // Files: lock mark, NAME EXT, size, date.
        let mark_w = font.measure("*") + pad * 2;
        let name_w = cell * 12;
        let size_w = font.measure("000000") + pad * 3;
        let date_w = if self.has_dates {
            font.measure("0000-00-00 00:00") + pad * 3
        } else {
            0
        };
        // Each list has a column of scroll arrows at its right.
        let arrow_w = lh;
        let files_w = pad * 2 + mark_w + name_w + size_w + date_w + pad * 2 + arrow_w;
        let want_w = cell * 16 + pad * 4 + files_w + pad * 12;
        let w = want_w
            .max(font.measure("M") * 36)
            .min(width as i32 - pad * 4);
        let h = (height as i32 - pad * 8).max(row_h * 8);
        let x0 = (width as i32 - w) / 2;
        let y0 = ((height as i32 - h) / 2).max(0);
        // The directories give up room first when the window is narrow.
        let tree_w = (w - pad * 12 - files_w).clamp(cell * 8 + pad * 4, cell * 16 + pad * 4);

        let frame = Rect::new(x0, y0, w as u32, h as u32);
        let title_h = lh + pad * 2;
        canvas.set_draw_color(Color::RGB(0xFF, 0xFF, 0xFF));
        let _ = canvas.fill_rect(frame);
        canvas.set_draw_color(t.bar);
        let _ = canvas.fill_rect(Rect::new(x0, y0, w as u32, title_h as u32));
        canvas.set_draw_color(t.border);
        let _ = canvas.draw_line((x0, y0 + title_h), (x0 + w - 1, y0 + title_h));
        let _ = canvas.draw_rect(frame);
        font.draw(canvas, x0 + pad * 4, y0 + pad, TITLE, t.text);

        let mut hits = std::mem::take(&mut self.hits);
        hits.clear();
        self.rows.clear();
        self.arrows.clear();
        let focus = self.focused();
        let lx = x0 + pad * 4;
        let rx = x0 + w - pad * 4;
        let mut y = y0 + title_h + pad * 3;

        // Image: path ......... [Choose Image...]
        // Names to draw in the machine's characters: where, what, colour,
        // and the list box they're clipped to.
        let glyphs: Vec<(i32, i32, Vec<u8>, Color, Rect)>;
        {
            let mut ui = Ui {
                canvas: &mut *canvas,
                font: &mut *font,
                pad,
                lh,
                focus,
                hits: &mut hits,
                rings: Vec::new(),
                in_row: |f| f == Field::Choose,
            };
            let cw = button(ui.font, CHOOSE);
            ui.button_w(Field::Choose, rx - cw, y, cw, CHOOSE);
            let label = "Image:";
            let px = lx + ui.font.measure(label) + pad * 2;
            ui.font.draw(ui.canvas, lx, y, label, t.text);
            let mut path = match &self.image {
                Some(p) => p.to_string_lossy().into_owned(),
                None => "No image chosen".into(),
            };
            // Show the end of a long path, which is the informative part.
            let room = rx - cw - pad * 4 - px;
            while path.chars().count() > 1 && ui.font.measure(&path) > room {
                path.remove(0);
            }
            let color = if self.image.is_some() { t.text } else { t.dim };
            ui.font.draw(ui.canvas, px, y, &path, color);
            y += lh + pad * 2;
            match &self.error {
                Some(e) => ui
                    .font
                    .draw(ui.canvas, lx, y, &format!("Can't read it: {e}"), warning),
                None => ui.font.draw(ui.canvas, lx, y, &self.info, t.dim),
            };
            y += lh + pad * 3;

            // The two lists.
            let button_h = lh + pad * 2;
            let list_top = y + lh + pad;
            let list_h = (y0 + h - pad * 3 - button_h - pad * 3 - list_top).max(row_h);
            let tree_box = Rect::new(lx, list_top, tree_w as u32, list_h as u32);
            let files_x = lx + tree_w + pad * 4;
            let files_box = Rect::new(
                files_x,
                list_top,
                (rx - files_x).max(10) as u32,
                list_h as u32,
            );
            ui.font.draw(ui.canvas, lx, y, "Directories", t.text);
            // Column headings over the files.
            let name_x = files_x + pad * 2 + mark_w;
            let size_r = name_x + name_w + size_w;
            ui.font.draw(ui.canvas, files_x, y, "Files", t.text);
            if !self.tree.is_empty() {
                let sw = ui.font.measure("Bytes");
                ui.font.draw(ui.canvas, size_r - sw, y, "Bytes", t.dim);
                if self.has_dates {
                    ui.font.draw(ui.canvas, size_r + pad * 3, y, "Date", t.dim);
                }
            }
            let mut g = Vec::new();

            // The tree.
            let rows = ((list_h - pad) / row_h).max(1) as usize;
            self.tree_rows = rows;
            if self.follow && self.dir < self.tree_top {
                self.tree_top = self.dir;
            } else if self.follow && self.dir >= self.tree_top + rows {
                self.tree_top = self.dir + 1 - rows;
            }
            self.tree_top = self.tree_top.min(self.tree.len().saturating_sub(rows));
            list_frame(ui.canvas, tree_box);
            self.tree_area = tree_box;
            ui.canvas.set_clip_rect(tree_box);
            for (i, row) in self.tree.iter().enumerate().skip(self.tree_top).take(rows) {
                let ry = list_top + pad / 2 + (i - self.tree_top) as i32 * row_h;
                let r = Rect::new(lx + 1, ry, (tree_w - 2 - arrow_w) as u32, row_h as u32);
                let on = i == self.dir;
                let fg = row_colors(ui.canvas, r, on, focus == Field::Tree);
                let gx = lx + pad * 2 + row.depth as i32 * cell;
                g.push((gx, ry + (row_h - cell) / 2, row.label.clone(), fg, tree_box));
                self.rows.push((r, Field::Tree, i));
            }

            ui.canvas.set_clip_rect(None);
            let more = (self.tree_top > 0, self.tree_top + rows < self.tree.len());
            self.arrows.extend(scroll_arrows(
                ui.canvas,
                tree_box,
                arrow_w,
                Field::Tree,
                more,
            ));

            // The files.
            let n = self.entries().len();
            self.files_rows = rows;
            if self.follow && self.selected < self.files_top {
                self.files_top = self.selected;
            } else if self.follow && self.selected >= self.files_top + rows {
                self.files_top = self.selected + 1 - rows;
            }
            self.follow = false;
            self.files_top = self.files_top.min(n.saturating_sub(rows));
            list_frame(ui.canvas, files_box);
            ui.canvas.set_clip_rect(files_box);
            let empty = match self.tree.get(self.dir).map(|t| &t.entries) {
                Some(Err(e)) => Some(format!("Can't list it: {e}")),
                Some(Ok(l)) if l.is_empty() => Some("No files".into()),
                _ => None,
            };
            if let Some(text) = empty {
                ui.font
                    .draw(ui.canvas, files_x + pad * 2, list_top + pad, &text, t.dim);
            }
            let entries: Vec<DiskEntry> = self
                .entries()
                .iter()
                .skip(self.files_top)
                .take(rows)
                .cloned()
                .collect();
            for (k, e) in entries.iter().enumerate() {
                let i = self.files_top + k;
                let ry = list_top + pad / 2 + k as i32 * row_h;
                let r = Rect::new(
                    files_x + 1,
                    ry,
                    files_box.width() - 2 - arrow_w as u32,
                    row_h as u32,
                );
                let on = i == self.selected;
                let fg = row_colors(ui.canvas, r, on, focus == Field::Files);
                let ty = ry + (row_h - lh) / 2;
                if e.locked {
                    ui.font.draw(ui.canvas, files_x + pad * 2, ty, "*", fg);
                }
                // NAME EXT, as DOS lists them.
                let (base, ext) = match e.name.iter().rposition(|&c| c == b'.') {
                    Some(d) => (&e.name[..d], &e.name[d + 1..]),
                    None => (&e.name[..], &[][..]),
                };
                let gy = ry + (row_h - cell) / 2;
                g.push((name_x, gy, base.to_vec(), fg, files_box));
                g.push((name_x + cell * 9, gy, ext.to_vec(), fg, files_box));
                let size = if e.is_dir {
                    "<DIR>".to_string()
                } else if e.damaged {
                    "damaged".to_string()
                } else {
                    e.size.to_string()
                };
                let sw = ui.font.measure(&size);
                ui.font.draw(ui.canvas, size_r - sw, ty, &size, fg);
                if let Some(d) = e.modified.filter(|_| self.has_dates) {
                    ui.font
                        .draw(ui.canvas, size_r + pad * 3, ty, &date_text(d), fg);
                }
                self.rows.push((r, Field::Files, i));
            }
            ui.canvas.set_clip_rect(None);
            let more = (self.files_top > 0, self.files_top + rows < n);
            self.arrows.extend(scroll_arrows(
                ui.canvas,
                files_box,
                arrow_w,
                Field::Files,
                more,
            ));
            // Focus rings around the lists.
            for (f, b) in [(Field::Tree, tree_box), (Field::Files, files_box)] {
                ui.rings.push((
                    f,
                    Rect::new(b.x() - 2, b.y() - 2, b.width() + 4, b.height() + 4),
                ));
            }

            // Buttons along the bottom.
            let by = y0 + h - pad * 3 - button_h;
            let mut bx = rx;
            for (f, label) in [(Field::Cancel, CANCEL), (Field::Open, OPEN)] {
                let bw = button(ui.font, label).max(digit * 8);
                bx -= bw;
                ui.button_w(f, bx, by, bw, label);
                bx -= pad * 2;
            }
            ui.draw_focus();
            glyphs = g;
        }
        self.hits = hits;

        // Names in the machine's characters, over their rows.
        for (x, y, codes, fg, clip) in glyphs {
            canvas.set_clip_rect(clip);
            let mut grid = Grid {
                canvas: &mut *canvas,
                atlas: &mut *atlas,
                cell: cell as u32,
                origin_x: x,
                origin_y: y,
            };
            grid.codes(0, 0, &codes, fg);
        }
        canvas.set_clip_rect(None);
    }
}

impl Default for ImagePage {
    fn default() -> Self {
        Self::new()
    }
}

/// The scroll arrows at the right of a list `area`, `w` pixels wide:
/// up at the top, down at the bottom, each dark when the list runs on
/// that way (`more`: above, below) and grey otherwise. Gives the ones
/// that can be clicked.
fn scroll_arrows(
    canvas: &mut Canvas<Window>,
    area: Rect,
    w: i32,
    list: Field,
    (above, below): (bool, bool),
) -> Vec<(Rect, Field, isize)> {
    let t = &THEME;
    let x = area.right() - 1 - w;
    let column = Rect::new(x, area.y() + 1, w as u32, area.height() - 2);
    canvas.set_draw_color(t.bar);
    let _ = canvas.fill_rect(column);
    canvas.set_draw_color(t.border);
    let _ = canvas.draw_line((x, column.y()), (x, column.bottom() - 1));
    let up = Rect::new(x, area.y() + 1, w as u32, w as u32);
    let down = Rect::new(x, area.bottom() - 1 - w, w as u32, w as u32);
    let mut hits = Vec::new();
    for (r, step, on) in [(up, -1, above), (down, 1, below)] {
        canvas.set_draw_color(t.border);
        let edge = if step < 0 { r.bottom() - 1 } else { r.y() };
        let _ = canvas.draw_line((r.x(), edge), (r.right() - 1, edge));
        // A triangle, pointing up or down.
        canvas.set_draw_color(if on { t.text } else { t.dim });
        let half = (w / 4).max(2);
        let (cx, cy) = (r.x() + w / 2, r.y() + w / 2);
        for i in 0..=half {
            let y = if step < 0 {
                cy - half / 2 + i
            } else {
                cy + half / 2 - i
            };
            let _ = canvas.draw_line((cx - i, y), (cx + i, y));
        }
        if on {
            hits.push((r, list, step));
        }
    }
    hits
}

/// A list's white box and border.
fn list_frame(canvas: &mut Canvas<Window>, r: Rect) {
    canvas.set_draw_color(Color::RGB(0xFF, 0xFF, 0xFF));
    let _ = canvas.fill_rect(r);
    canvas.set_draw_color(THEME.border);
    let _ = canvas.draw_rect(r);
}

/// Fill a list row's background (the selected row highlighted; grey
/// when its list hasn't the focus) and give its text colour.
fn row_colors(canvas: &mut Canvas<Window>, r: Rect, selected: bool, focused: bool) -> Color {
    let t = &THEME;
    if !selected {
        return t.text;
    }
    if focused {
        canvas.set_draw_color(t.highlight);
        let _ = canvas.fill_rect(r);
        t.highlight_text
    } else {
        canvas.set_draw_color(Color::RGB(0xDC, 0xDC, 0xDC));
        let _ = canvas.fill_rect(r);
        t.text
    }
}

/// "2026-11-24 15:30".
fn date_text(d: DateTime) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        d.year, d.month, d.day, d.hour, d.minute
    )
}
