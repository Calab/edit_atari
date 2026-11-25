//! The Open from Image and Save to Image pages (File menu): the image's
//! directories on the left, the chosen directory's files on the right,
//! names in the machine's own characters. Save to Image adds a File name
//! box.
//!
//! The page holds a copy of the image, read when the image is chosen, for
//! its listing and to check names. It never writes it: opening or saving
//! a file reads the image file again.

use std::path::PathBuf;

use retro_core::disk::{DateTime, DiskEntry, DiskVolume};
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas, Texture};
use sdl2::video::Window;

use crate::menu::THEME;
use crate::new_image::{self, NewImagePopup};
use crate::render::Grid;
use crate::ui_font::UiFont;
use crate::widgets::{Op, Ui};

/// Which page this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Open,
    Save,
}

/// The controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Tree,
    Files,
    /// The File name box (Save to Image).
    Name,
    Choose,
    NewDir,
    NewImage,
    /// Open, or Save.
    Go,
    Cancel,
}

const CHOOSE: &str = "Choose Image...";
const NEW_DIR: &str = "Create Directory...";
const NEW_IMAGE: &str = "New Image...";
const CANCEL: &str = "Cancel";

/// The longest name: 8 characters, a dot and 3 more.
const NAME_MAX: usize = 12;

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
    /// Save the document as the name in the File name box.
    Save,
    /// Make the directory named in the Create Directory pop-up, inside
    /// the directory shown.
    MakeDir,
    /// Open the New Image pop-up (the app knows the choices).
    OpenNewImage,
    /// Make the image chosen in the New Image pop-up.
    CreateImage,
    Close,
}

/// The Create Directory pop-up: the name typed, the cursor, and why the
/// last try failed.
pub struct NewDirPopup {
    pub name: Vec<u8>,
    cursor: usize,
    pub error: Option<String>,
    /// Create (true) and Cancel (false), from the last draw.
    hits: Vec<(Rect, bool)>,
}

pub struct ImagePage {
    pub mode: Mode,
    /// The image shown, if one has been chosen.
    pub image: Option<PathBuf>,
    /// The image as read, for checking names.
    vol: Option<Box<dyn DiskVolume>>,
    /// "SpartaDOS, 720 sectors of 256 bytes, 312 free".
    pub info: String,
    /// Why the image can't be shown, instead of `info`.
    pub error: Option<String>,
    write_protected: bool,
    has_directories: bool,
    /// The Create Directory pop-up, while it's open.
    pub new_dir: Option<NewDirPopup>,
    /// The New Image pop-up, while it's open.
    pub new_image: Option<NewImagePopup>,
    /// The machine can make new images (New Image isn't greyed out).
    pub can_new_image: bool,
    pub tree: Vec<TreeRow>,
    pub has_dates: bool,
    /// The directory shown (an index into `tree`).
    pub dir: usize,
    /// The selected entry of that directory.
    pub selected: usize,
    /// The File name box: the name in machine codes, and where the
    /// cursor is in it.
    pub name: Vec<u8>,
    cursor: usize,
    focus: Field,
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
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            image: None,
            vol: None,
            info: String::new(),
            error: None,
            write_protected: false,
            has_directories: false,
            new_dir: None,
            new_image: None,
            can_new_image: false,
            tree: Vec::new(),
            has_dates: false,
            dir: 0,
            selected: 0,
            name: Vec::new(),
            cursor: 0,
            focus: Self::first_focus(mode),
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

    fn first_focus(mode: Mode) -> Field {
        match mode {
            Mode::Open => Field::Files,
            Mode::Save => Field::Name,
        }
    }

    /// Show `vol`, read from `image`. `top` is how the top directory is
    /// shown, in machine codes.
    pub fn load(&mut self, image: PathBuf, vol: Box<dyn DiskVolume>, top: Vec<u8>) {
        self.image = Some(image);
        self.error = None;
        self.info = vol.info();
        self.write_protected = vol.write_protected();
        if self.write_protected {
            self.info.push_str(", write-protected");
        }
        self.has_dates = vol.has_dates();
        self.has_directories = vol.has_directories();
        self.new_dir = None;
        self.tree.clear();
        self.add_dir(vol.as_ref(), Vec::new(), top, 0);
        self.vol = Some(vol);
        self.dir = 0;
        self.selected = 0;
        self.tree_top = 0;
        self.files_top = 0;
        self.focus = Self::first_focus(self.mode);
        self.follow = true;
        // The name, as this disk's names allow.
        let name = std::mem::take(&mut self.name);
        self.set_name(&name);
    }

    /// The image couldn't be read or opened.
    pub fn failed(&mut self, image: PathBuf, why: String) {
        self.image = Some(image);
        self.vol = None;
        self.new_dir = None;
        self.error = Some(why);
        self.info.clear();
        self.tree.clear();
        self.dir = 0;
        self.selected = 0;
        self.focus = Field::Choose;
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
        Some(self.path_of(&e.name))
    }

    /// The path of `name` in the directory shown.
    pub fn path_of(&self, name: &[u8]) -> Vec<Vec<u8>> {
        let mut path = self
            .tree
            .get(self.dir)
            .map_or(Vec::new(), |t| t.path.clone());
        path.push(name.to_vec());
        path
    }

    // ----- the File name box ---------------------------------------------------

    /// Put `name` in the File name box: as it is if a file in the
    /// directory shown has exactly that name (so an odd name made by
    /// another program can be replaced), otherwise only the characters
    /// this disk's names allow (lower case as capitals), cut to 8.3.
    pub fn set_name(&mut self, name: &[u8]) {
        let existing = self.entries().iter().any(|e| !e.is_dir && e.name == name);
        self.name = if existing {
            name.to_vec()
        } else {
            let dot = name.iter().rposition(|&c| c == b'.');
            let (base, ext) = match dot {
                Some(d) => (&name[..d], &name[d + 1..]),
                None => (name, &[][..]),
            };
            let keep = |part: &[u8], n: usize| -> Vec<u8> {
                part.iter()
                    .filter(|&&c| c != b'.')
                    .filter_map(|&c| self.name_char(c))
                    .take(n)
                    .collect()
            };
            let mut out = keep(base, 8);
            let ext = keep(ext, 3);
            if !ext.is_empty() {
                out.push(b'.');
                out.extend(ext);
            }
            out
        };
        self.cursor = self.name.len();
    }

    /// A typed character as it goes in a name, or `None`.
    fn name_char(&self, code: u8) -> Option<u8> {
        match &self.vol {
            Some(v) => v.name_char(code),
            None => match code {
                b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' => Some(code),
                b'a'..=b'z' => Some(code.to_ascii_uppercase()),
                _ => None,
            },
        }
    }

    /// Text typed while the page is open, in machine codes: into the
    /// File name box if it has the focus.
    pub fn type_codes(&mut self, codes: &[u8]) {
        if let Some(p) = &mut self.new_image {
            p.type_codes(codes);
            return;
        }
        let codes: Vec<u8> = codes.iter().filter_map(|&c| self.name_char(c)).collect();
        let (name, cursor) = match &mut self.new_dir {
            Some(p) => (&mut p.name, &mut p.cursor),
            None if self.focus == Field::Name => (&mut self.name, &mut self.cursor),
            None => return,
        };
        for c in codes {
            if name.len() >= NAME_MAX {
                break;
            }
            name.insert(*cursor, c);
            *cursor += 1;
        }
    }

    /// Whether typed text is wanted (a name box has the focus).
    pub fn wants_text(&self) -> bool {
        self.new_image.is_some() || self.new_dir.is_some() || self.focus == Field::Name
    }

    /// Why the name in the box can't be saved as, if it can't: `Some("")`
    /// when there's nothing to say (an empty box).
    pub fn name_problem(&self) -> Option<String> {
        self.problem_with(&self.name, false)
    }

    /// Why `name` can't be used for a new file (`dir` false; an existing
    /// file may be replaced) or directory, if it can't.
    fn problem_with(&self, name: &[u8], dir: bool) -> Option<String> {
        if name.is_empty() {
            return Some(String::new());
        }
        if let Some(e) = self.entries().iter().find(|e| e.name == name) {
            return match (dir, e.is_dir) {
                (false, false) => None,
                (false, true) => Some("That's the name of a directory".into()),
                (true, _) => Some("That name is already in this directory".into()),
            };
        }
        let vol = self.vol.as_ref()?;
        vol.check_name(name).err().map(|e| {
            // "bad name: X: why" -> "Why".
            let why = e.message.rsplit(": ").next().unwrap_or(&e.message);
            let mut c = why.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect())
                .unwrap_or_default()
        })
    }

    /// Whether Create Directory can be pressed.
    fn can_make_dir(&self) -> bool {
        self.vol.is_some() && self.has_directories && !self.write_protected
    }

    /// Open the Create Directory pop-up.
    fn open_new_dir(&mut self) {
        if self.can_make_dir() {
            self.new_dir = Some(NewDirPopup {
                name: Vec::new(),
                cursor: 0,
                error: None,
                hits: Vec::new(),
            });
        }
    }

    /// Why the pop-up's name can't be used, if it can't.
    fn new_dir_problem(&self) -> Option<String> {
        let p = self.new_dir.as_ref()?;
        self.problem_with(&p.name, true)
    }

    /// The directory was made: close the pop-up and show it (after the
    /// app has loaded the image again).
    pub fn show_path(&mut self, path: &[Vec<u8>]) {
        if let Some(d) = self.tree.iter().position(|t| t.path == path) {
            self.show_dir(d);
        }
    }

    /// Whether Save can be pressed.
    fn can_save(&self) -> bool {
        self.vol.is_some() && !self.write_protected && self.name_problem().is_none()
    }

    // ----- focus and keys ------------------------------------------------------

    /// The controls in Tab order, leaving out greyed-out ones.
    fn order(&self) -> Vec<Field> {
        match self.mode {
            Mode::Open => vec![
                Field::Tree,
                Field::Files,
                Field::Choose,
                Field::Go,
                Field::Cancel,
            ],
            Mode::Save => {
                let mut v = vec![Field::Tree, Field::Files, Field::Name, Field::Choose];
                if self.can_make_dir() {
                    v.push(Field::NewDir);
                }
                if self.can_new_image {
                    v.push(Field::NewImage);
                }
                if self.can_save() {
                    v.push(Field::Go);
                }
                v.push(Field::Cancel);
                v
            }
        }
    }

    pub fn focused(&self) -> Field {
        self.focus
    }

    fn move_focus(&mut self, step: isize) {
        let order = self.order();
        let i = order.iter().position(|&f| f == self.focus).unwrap_or(0) as isize;
        let n = order.len() as isize;
        self.focus = order[(i + step).rem_euclid(n) as usize];
    }

    fn show_dir(&mut self, d: usize) {
        self.follow = true;
        if d < self.tree.len() && d != self.dir {
            self.dir = d;
            self.selected = 0;
            self.files_top = 0;
        }
    }

    /// Enter the selected directory; on a file, open it or save as it.
    fn activate(&mut self) -> Action {
        let Some(e) = self.entries().get(self.selected) else {
            return Action::None;
        };
        if !e.is_dir {
            return match self.mode {
                Mode::Open => Action::Open,
                Mode::Save => {
                    let name = e.name.clone();
                    self.set_name(&name);
                    self.save()
                }
            };
        }
        let path = self.path_of(&e.name.clone());
        if let Some(d) = self.tree.iter().position(|t| t.path == path) {
            self.show_dir(d);
        }
        Action::None
    }

    /// Save, if the name allows it.
    fn save(&self) -> Action {
        if self.can_save() {
            Action::Save
        } else {
            Action::None
        }
    }

    /// The Open or Save button.
    fn go(&mut self) -> Action {
        match self.mode {
            Mode::Open => self.activate(),
            Mode::Save => self.save(),
        }
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

    /// A key while the page is open; every key is used up.
    pub fn key(&mut self, k: sdl2::keyboard::Keycode, shift: bool) -> Action {
        use sdl2::keyboard::Keycode as K;
        // A pop-up takes every key while it's open.
        if let Some(p) = &mut self.new_image {
            return match p.key(k, shift) {
                new_image::Action::Create => Action::CreateImage,
                new_image::Action::Cancel => {
                    self.new_image = None;
                    Action::None
                }
                new_image::Action::None => Action::None,
            };
        }
        if self.new_dir.is_some() {
            let ok = self.new_dir_problem().is_none();
            let p = self.new_dir.as_mut().unwrap();
            match k {
                K::ESCAPE => self.new_dir = None,
                K::RETURN | K::KP_ENTER if ok => return Action::MakeDir,
                _ => edit_name(&mut p.name, &mut p.cursor, k),
            }
            return Action::None;
        }
        if k == K::ESCAPE {
            return Action::Close;
        }
        if k == K::TAB {
            self.move_focus(if shift { -1 } else { 1 });
            return Action::None;
        }
        let enter = matches!(k, K::RETURN | K::KP_ENTER);
        let press = enter || k == K::SPACE;
        self.follow = true;
        match self.focus {
            Field::Choose if press => return Action::Choose,
            Field::NewDir if press => self.open_new_dir(),
            Field::NewImage if press => return Action::OpenNewImage,
            Field::Go if press => return self.go(),
            Field::Cancel if press => return Action::Close,
            Field::Name if enter => return self.save(),
            Field::Name => edit_name(&mut self.name, &mut self.cursor, k),
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
                        self.focus = Field::Files;
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
                    _ if enter => return self.activate(),
                    K::BACKSPACE | K::LEFT => {
                        self.parent();
                        None
                    }
                    _ => None,
                };
                if let Some(i) = to.filter(|_| n > 0) {
                    self.selected = i.min(n - 1);
                    self.pick_name();
                }
            }
            _ => {}
        }
        Action::None
    }

    /// Saving: a file chosen in the list puts its name in the box.
    fn pick_name(&mut self) {
        if self.mode != Mode::Save {
            return;
        }
        if let Some(e) = self.entries().get(self.selected).filter(|e| !e.is_dir) {
            let name = e.name.clone();
            self.set_name(&name);
        }
    }

    /// A left click; `double` for a double click.
    pub fn click(&mut self, x: i32, y: i32, double: bool) -> Action {
        // A pop-up: only it works while it's open.
        if let Some(p) = &mut self.new_image {
            return match p.click(x, y) {
                new_image::Action::Create => Action::CreateImage,
                new_image::Action::Cancel => {
                    self.new_image = None;
                    Action::None
                }
                new_image::Action::None => Action::None,
            };
        }
        if let Some(p) = &self.new_dir {
            match p.hits.iter().find(|(r, _)| r.contains_point((x, y))) {
                Some(&(_, true)) => return Action::MakeDir,
                Some(&(_, false)) => self.new_dir = None,
                None => {}
            }
            return Action::None;
        }
        if let Some(&(_, list, step)) = self.arrows.iter().find(|(r, ..)| r.contains_point((x, y)))
        {
            self.scroll(list, step);
            return Action::None;
        }
        if let Some(&(_, list, row)) = self.rows.iter().find(|(r, ..)| r.contains_point((x, y))) {
            self.focus = list;
            match list {
                Field::Tree => self.show_dir(row),
                _ => {
                    self.selected = row;
                    self.pick_name();
                    if double {
                        return self.activate();
                    }
                }
            }
            return Action::None;
        }
        if let Some(&(_, f, _)) = self.hits.iter().find(|(r, ..)| r.contains_point((x, y))) {
            self.focus = f;
            return match f {
                Field::Choose => Action::Choose,
                Field::NewDir => {
                    self.open_new_dir();
                    Action::None
                }
                Field::NewImage => Action::OpenNewImage,
                Field::Go => self.go(),
                Field::Cancel => Action::Close,
                _ => Action::None,
            };
        }
        Action::None
    }

    /// The mouse wheel: scroll the list under the pointer (`dy` notches,
    /// up positive).
    pub fn wheel(&mut self, x: i32, y: i32, dy: i32) {
        if self.new_dir.is_some() || self.new_image.is_some() {
            return;
        }
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

    // ----- drawing -------------------------------------------------------------

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

        let saving = self.mode == Mode::Save;
        let (title, go_label) = match self.mode {
            Mode::Open => ("Open from Image", "Open"),
            Mode::Save => ("Save to Image", "Save"),
        };
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
        let h = (height as i32 - pad * 8).max(row_h * 10);
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
        font.draw(canvas, x0 + pad * 4, y0 + pad, title, t.text);

        let mut hits = std::mem::take(&mut self.hits);
        hits.clear();
        self.rows.clear();
        self.arrows.clear();
        let focus = self.focused();
        let can_save = self.can_save();
        let problem = if saving { self.name_problem() } else { None };
        let lx = x0 + pad * 4;
        let rx = x0 + w - pad * 4;
        let mut y = y0 + title_h + pad * 3;

        // Names to draw in the machine's characters: where, what, colour,
        // and the box they're clipped to.
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
            // Image: path ......... [Choose Image...]
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

            // The two lists, above the File name row (saving) and the
            // buttons.
            let button_h = lh + pad * 2;
            let name_row_h = if saving { row_h + pad * 3 } else { 0 };
            let list_top = y + lh + pad;
            let list_h = (y0 + h - pad * 3 - button_h - pad * 3 - name_row_h - list_top).max(row_h);
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
                let (base, ext) = split_name(&e.name);
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

            // File name: [SAMPLE.LST  ]  why it can't be used
            if saving {
                let ny = list_top + list_h + pad * 3;
                let label = "File name:";
                ui.font
                    .draw(ui.canvas, lx, ny + (row_h - lh) / 2, label, t.text);
                let bx = lx + ui.font.measure(label) + pad * 3;
                let field_w = cell * NAME_MAX as i32 + cell + pad * 2;
                let r = Rect::new(bx, ny, field_w as u32, row_h as u32);
                ui.canvas.set_draw_color(Color::RGB(0xFF, 0xFF, 0xFF));
                let _ = ui.canvas.fill_rect(r);
                ui.field_box(Field::Name, r);
                let gx = bx + pad;
                let gy = ny + (row_h - cell) / 2;
                g.push((gx, gy, self.name.clone(), t.text, r));
                if focus == Field::Name {
                    // A block cursor, the character under it reversed.
                    let cx = gx + self.cursor as i32 * cell;
                    ui.canvas.set_draw_color(t.text);
                    let _ = ui
                        .canvas
                        .fill_rect(Rect::new(cx, gy, cell as u32, cell as u32));
                    if let Some(&c) = self.name.get(self.cursor) {
                        g.push((cx, gy, vec![c], Color::RGB(0xFF, 0xFF, 0xFF), r));
                    }
                }
                let note = if self.write_protected {
                    Some("The image is write-protected: it can't be saved to".to_string())
                } else {
                    problem.filter(|p| !p.is_empty())
                };
                if let Some(note) = note {
                    let tx = bx + field_w + pad * 4;
                    ui.font
                        .draw(ui.canvas, tx, ny + (row_h - lh) / 2, &note, warning);
                }
            }

            // Buttons along the bottom: New Directory and New Image at the
            // left (saving), Open or Save and Cancel at the right.
            let by = y0 + h - pad * 3 - button_h;
            if saving {
                let bw = button(ui.font, NEW_DIR);
                if self.can_make_dir() {
                    ui.button_w(Field::NewDir, lx, by, bw, NEW_DIR);
                } else {
                    ui.button_off(lx, by, bw, NEW_DIR);
                }
                let iw = button(ui.font, NEW_IMAGE);
                if self.can_new_image {
                    ui.button_w(Field::NewImage, lx + bw + pad * 2, by, iw, NEW_IMAGE);
                } else {
                    ui.button_off(lx + bw + pad * 2, by, iw, NEW_IMAGE);
                }
            }
            let mut bx = rx;
            for (f, label) in [(Field::Cancel, CANCEL), (Field::Go, go_label)] {
                let bw = button(ui.font, label).max(digit * 8);
                bx -= bw;
                if f == Field::Go && saving && !can_save {
                    ui.button_off(bx, by, bw, label);
                } else {
                    ui.button_w(f, bx, by, bw, label);
                }
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

        if self.new_dir.is_some() {
            self.draw_new_dir(canvas, font, atlas, (width, height), pad, cell, warning);
        }
        if let Some(p) = &mut self.new_image {
            p.draw(canvas, font, atlas, (width, height), pad, cell, warning);
        }
    }

    /// The Create Directory pop-up, in the middle of the window.
    #[allow(clippy::too_many_arguments)]
    fn draw_new_dir(
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
        let problem = self.new_dir_problem();
        let inside = self
            .tree
            .get(self.dir)
            .map_or(Vec::new(), |t| t.label.clone());
        let Some(p) = &mut self.new_dir else { return };
        canvas.set_blend_mode(BlendMode::BLEND);
        canvas.set_draw_color(Color::RGBA(0, 0, 0, 60));
        let _ = canvas.fill_rect(None);
        canvas.set_blend_mode(BlendMode::NONE);

        let lh = font.line_height();
        let row_h = lh.max(cell) + pad;
        let label_w = font.measure("Inside:").max(font.measure("Name:"));
        let field_w = cell * NAME_MAX as i32 + cell + pad * 2;
        let w = (pad * 8 + label_w + pad * 3 + field_w).max(font.measure("M") * 26);
        let title_h = lh + pad * 2;
        let button_h = lh + pad * 2;
        let h = title_h + pad * 3 + row_h * 2 + pad * 2 + lh + pad * 3 + button_h + pad * 3;
        let x0 = (width as i32 - w) / 2;
        let y0 = (height as i32 - h) / 2;
        let frame = Rect::new(x0, y0, w as u32, h as u32);
        canvas.set_draw_color(Color::RGB(0xFF, 0xFF, 0xFF));
        let _ = canvas.fill_rect(frame);
        canvas.set_draw_color(t.bar);
        let _ = canvas.fill_rect(Rect::new(x0, y0, w as u32, title_h as u32));
        canvas.set_draw_color(t.border);
        let _ = canvas.draw_line((x0, y0 + title_h), (x0 + w - 1, y0 + title_h));
        let _ = canvas.draw_rect(frame);
        font.draw(canvas, x0 + pad * 4, y0 + pad, "Create Directory", t.text);

        let lx = x0 + pad * 4;
        let fx = lx + label_w + pad * 3;
        let mut y = y0 + title_h + pad * 3;
        let mut glyphs = Vec::new();
        // Inside: GAMES
        font.draw(canvas, lx, y + (row_h - lh) / 2, "Inside:", t.text);
        glyphs.push((fx + pad, y + (row_h - cell) / 2, inside, t.text));
        y += row_h + pad;
        // Name: [        ]
        font.draw(canvas, lx, y + (row_h - lh) / 2, "Name:", t.text);
        let r = Rect::new(fx, y, field_w as u32, row_h as u32);
        canvas.set_draw_color(t.highlight);
        let _ = canvas.draw_rect(r);
        let _ = canvas.draw_rect(Rect::new(
            r.x() + 1,
            r.y() + 1,
            r.width() - 2,
            r.height() - 2,
        ));
        let gx = fx + pad;
        let gy = y + (row_h - cell) / 2;
        glyphs.push((gx, gy, p.name.clone(), t.text));
        let cx = gx + p.cursor as i32 * cell;
        canvas.set_draw_color(t.text);
        let _ = canvas.fill_rect(Rect::new(cx, gy, cell as u32, cell as u32));
        if let Some(&c) = p.name.get(p.cursor) {
            glyphs.push((cx, gy, vec![c], Color::RGB(0xFF, 0xFF, 0xFF)));
        }
        y += row_h + pad * 2;
        // Why it can't be made.
        let note = p
            .error
            .clone()
            .or(problem.clone().filter(|s| !s.is_empty()));
        if let Some(note) = note {
            font.draw(canvas, lx, y, &note, warning);
        }

        // [Create] [Cancel]
        p.hits.clear();
        let by = y0 + h - pad * 3 - button_h;
        let bw = font.measure("Create").max(font.measure("Cancel")) + pad * 6;
        let ok = problem.is_none();
        let mut bx = x0 + w - pad * 4;
        for (label, create) in [("Cancel", false), ("Create", true)] {
            bx -= bw;
            let r = Rect::new(bx, by, bw as u32, button_h as u32);
            let (fill, text) = match (create, ok) {
                (true, true) => (t.highlight, t.highlight_text),
                (true, false) => (t.bar, t.dim),
                _ => (t.bar, t.text),
            };
            canvas.set_draw_color(fill);
            let _ = canvas.fill_rect(r);
            canvas.set_draw_color(if fill == t.highlight { fill } else { t.border });
            let _ = canvas.draw_rect(r);
            let lw = font.measure(label);
            font.draw(
                canvas,
                bx + (bw - lw) / 2,
                by + (button_h - lh) / 2,
                label,
                text,
            );
            if !create || ok {
                p.hits.push((r, create));
            }
            bx -= pad * 2;
        }

        for (x, y, codes, fg) in glyphs {
            let mut grid = Grid {
                canvas: &mut *canvas,
                atlas: &mut *atlas,
                cell: cell as u32,
                origin_x: x,
                origin_y: y,
            };
            grid.codes(0, 0, &codes, fg);
        }
    }
}

/// An editing key in a name box: arrows, Home, End, Backspace, Delete.
fn edit_name(name: &mut Vec<u8>, cursor: &mut usize, k: sdl2::keyboard::Keycode) {
    use sdl2::keyboard::Keycode as K;
    match k {
        K::LEFT => *cursor = cursor.saturating_sub(1),
        K::RIGHT => *cursor = (*cursor + 1).min(name.len()),
        K::HOME => *cursor = 0,
        K::END => *cursor = name.len(),
        K::BACKSPACE if *cursor > 0 => {
            *cursor -= 1;
            name.remove(*cursor);
        }
        K::DELETE if *cursor < name.len() => {
            name.remove(*cursor);
        }
        _ => {}
    }
}

/// `NAME.EXT` as its name and extension (without the dot).
fn split_name(name: &[u8]) -> (&[u8], &[u8]) {
    match name.iter().rposition(|&c| c == b'.') {
        Some(d) => (&name[..d], &name[d + 1..]),
        None => (name, &[]),
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
