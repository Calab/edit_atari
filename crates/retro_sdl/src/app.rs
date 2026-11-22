//! Editor state, input handling and commands.

use std::path::{Path, PathBuf};

use retro_core::{
    Document, EditAction, Editor, Key, KeyInput, Layout, Mods, Pos, WrapMode, clipboard,
};
use sdl2::clipboard::ClipboardUtil;
use sdl2::keyboard::{Keycode, Mod};
use sdl2::video::Window;

use crate::AppSpec;
use crate::config::{self, Config, ControlKey};
use crate::menu::{self, Command, Geometry, ItemState, Menu, MenuState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    None,
    Help,
    About,
}

/// Pixel layout of the window for one frame.
pub struct Frame {
    pub width: u32,
    /// Pixels per character cell.
    pub cell: u32,
    /// Top of the text area (bottom of the menu bar).
    pub text_top: i32,
    /// Top of the status bar.
    pub status_top: i32,
    pub layout: Layout,
    /// X pixel of character column 0.
    pub origin_x: i32,
}

/// What commands need from the window system.
pub struct Ctx<'a> {
    pub window: &'a Window,
    pub clipboard: &'a ClipboardUtil,
    pub frame: &'a Frame,
}

pub struct App {
    pub spec: AppSpec,
    pub config: Config,
    pub editor: Editor,
    pub path: Option<PathBuf>,
    pub message: Option<String>,
    pub running: bool,
    pub overlay: Overlay,
    pub overlay_top: usize,
    /// A number being typed into the status bar (line length warning).
    pub prompt: Option<String>,
    pub menus: Vec<Menu>,
    pub menu: MenuState,
    /// Where the menus were last drawn.
    pub geom: Geometry,
    /// Physical pixels per logical pixel (HiDPI).
    pub ratio: f32,
    /// Interface spacing unit and bar height, in pixels.
    pub pad: i32,
    pub bar_height: i32,
    /// Quit was requested but a confirmation box couldn't be shown; a second
    /// request quits.
    quit_armed: bool,
    /// The last key press was a command, so the text it would type (e.g.
    /// the `` ` `` of the inverse key) must be ignored.
    swallow_text: bool,
    /// Selecting text by dragging with the mouse.
    dragging: bool,
}

impl App {
    pub fn new(spec: AppSpec) -> Self {
        let config = Config::load(spec.app_id);
        let editor = Editor::new(
            Document::new(),
            config.wrap,
            spec.machine.default_tab_stops(),
        );
        Self {
            spec,
            config,
            editor,
            path: None,
            message: None,
            running: true,
            overlay: Overlay::None,
            overlay_top: 0,
            prompt: None,
            menus: menu::menus(),
            menu: MenuState::default(),
            geom: Geometry::default(),
            ratio: 1.0,
            pad: 6,
            bar_height: 24,
            quit_armed: false,
            swallow_text: false,
            dragging: false,
        }
    }

    pub fn set_ui_metrics(&mut self, line_height: i32) {
        self.pad = (6.0 * self.ratio).round() as i32;
        self.bar_height = line_height + self.pad;
    }

    /// Lay out the window: menu bar, text area, status bar.
    pub fn frame(&self, (width, height): (u32, u32)) -> Frame {
        let cell = self.config.scale * 8;
        let text_top = self.bar_height;
        let status_top = height as i32 - self.bar_height;
        let rows = ((status_top - text_top).max(0) as u32 / cell) as usize;
        let cols = (width / cell) as usize;
        let layout = self.editor.layout(self.spec.machine.as_ref(), cols, rows);
        let origin_x = match self.editor.wrap {
            WrapMode::Native => {
                let w = (layout.left_margin + layout.text_cols) as u32 * cell;
                (width.saturating_sub(w) / 2) as i32
            }
            WrapMode::Window => 0,
        };
        Frame {
            width,
            cell,
            text_top,
            status_top,
            layout,
            origin_x,
        }
    }

    pub fn file_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into())
    }

    pub fn window_title(&self) -> String {
        format!(
            "{}{} - {}",
            self.file_name(),
            if self.editor.doc.is_modified() {
                "*"
            } else {
                ""
            },
            self.spec.title
        )
    }

    fn new_editor(&mut self, doc: Document) {
        self.editor = Editor::new(doc, self.config.wrap, self.editor.tabs.clone());
    }

    // ----- files -------------------------------------------------------------

    pub fn open_path(&mut self, path: &Path) {
        match std::fs::read(path) {
            Ok(bytes) => {
                let d = self.spec.machine.decode(&bytes);
                let mut doc = Document::from_lines(d.lines, d.final_eol);
                self.message = None;
                if let Some(from) = d.converted_from {
                    doc.mark_modified();
                    self.message = Some(format!("Converted from {from}; will save as native"));
                }
                self.new_editor(doc);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.new_editor(Document::new());
                self.message = Some("New file".into());
            }
            Err(e) => {
                self.message = Some(format!("Can't open {}: {e}", path.display()));
                return;
            }
        }
        self.path = Some(path.to_path_buf());
    }

    fn dialog(&self, title: &str) -> rfd::FileDialog {
        let (desc, exts) = self.spec.file_types;
        let mut d = rfd::FileDialog::new()
            .set_title(title)
            .add_filter(desc, exts)
            .add_filter("All files", &["*"]);
        if let Some(dir) = self.path.as_ref().and_then(|p| p.parent())
            && dir.is_dir()
        {
            d = d.set_directory(dir);
        }
        d
    }

    /// Save to the current file, asking for a name if there isn't one.
    /// Returns true if the file was saved.
    fn save(&mut self) -> bool {
        match self.path.clone() {
            Some(path) => self.write_to(&path),
            None => self.save_as(),
        }
    }

    fn save_as(&mut self) -> bool {
        let Some(path) = self
            .dialog("Save As")
            .set_file_name(self.file_name())
            .save_file()
        else {
            return false;
        };
        if self.write_to(&path) {
            self.path = Some(path);
            true
        } else {
            false
        }
    }

    fn write_to(&mut self, path: &Path) -> bool {
        let bytes = self.spec.machine.encode(&self.editor.doc);
        match write_atomically(path, &bytes) {
            Ok(()) => {
                self.editor.doc.mark_saved();
                self.message = Some(format!("Saved {} bytes", bytes.len()));
                true
            }
            Err(e) => {
                self.message = Some(format!("Save failed: {e}"));
                false
            }
        }
    }

    /// If there are unsaved changes, ask whether to save them. Returns true
    /// if it's fine to throw the document away.
    fn confirm_discard(&mut self, window: &Window) -> bool {
        use sdl2::messagebox::*;
        if !self.editor.doc.is_modified() {
            return true;
        }
        let buttons = [
            ButtonData {
                flags: MessageBoxButtonFlag::RETURNKEY_DEFAULT,
                button_id: 0,
                text: "Save",
            },
            ButtonData {
                flags: MessageBoxButtonFlag::NOTHING,
                button_id: 1,
                text: "Don't Save",
            },
            ButtonData {
                flags: MessageBoxButtonFlag::ESCAPEKEY_DEFAULT,
                button_id: 2,
                text: "Cancel",
            },
        ];
        let text = format!("Save changes to {}?", self.file_name());
        match show_message_box(
            MessageBoxFlag::WARNING,
            &buttons,
            self.spec.title,
            &text,
            window,
            None,
        ) {
            Ok(ClickedButton::CustomButton(b)) => match b.button_id {
                0 => self.save(),
                1 => true,
                _ => false,
            },
            Ok(ClickedButton::CloseButton) => false,
            Err(_) => {
                // No message box available: ask for the command twice.
                if self.quit_armed {
                    true
                } else {
                    self.quit_armed = true;
                    self.message = Some("Unsaved changes! Repeat to discard them".into());
                    false
                }
            }
        }
    }

    fn save_config(&mut self) {
        if let Err(e) = self.config.save(self.spec.app_id) {
            self.message = Some(format!("Settings not saved: {e}"));
        }
    }

    // ----- commands ------------------------------------------------------------

    pub fn item_state(&self, c: Command, clipboard: &ClipboardUtil) -> ItemState {
        use Command::*;
        let enabled = match c {
            Undo => self.editor.can_undo(),
            Redo => self.editor.can_redo(),
            Cut | Copy => self.editor.has_selection(),
            Paste => clipboard.has_clipboard_text(),
            _ => true,
        };
        let checked = match c {
            ToggleWrap => Some(self.config.wrap == WrapMode::Native),
            ToggleClick => Some(self.config.key_click),
            ToggleControlKey => Some(self.config.control_key == ControlKey::LeftAlt),
            _ => None,
        };
        ItemState { enabled, checked }
    }

    pub fn command(&mut self, c: Command, ctx: &Ctx) {
        use Command::*;
        if !self.item_state(c, ctx.clipboard).enabled {
            return;
        }
        let layout = &ctx.frame.layout;
        if !matches!(c, Quit | New | Open) {
            self.quit_armed = false;
        }
        match c {
            New => {
                if self.confirm_discard(ctx.window) {
                    self.new_editor(Document::new());
                    self.path = None;
                    self.message = None;
                }
            }
            Open => {
                if self.confirm_discard(ctx.window)
                    && let Some(path) = self.dialog("Open").pick_file()
                {
                    self.open_path(&path);
                }
            }
            Save => {
                self.save();
            }
            SaveAs => {
                self.save_as();
            }
            Quit => {
                if self.confirm_discard(ctx.window) {
                    self.running = false;
                }
            }
            Undo => {
                self.editor.undo(layout);
            }
            Redo => {
                self.editor.redo(layout);
            }
            Cut | Copy => {
                let text = if c == Cut {
                    self.editor.cut(layout)
                } else {
                    self.editor.copy()
                };
                if let Some(lines) = text {
                    let s = clipboard::to_text(&lines, self.spec.machine.as_ref());
                    if let Err(e) = ctx.clipboard.set_clipboard_text(&s) {
                        self.message = Some(format!("Clipboard error: {e}"));
                    }
                }
            }
            Paste => match ctx.clipboard.clipboard_text() {
                Ok(s) => {
                    let (lines, skipped) = clipboard::from_text(&s, self.spec.machine.as_ref());
                    self.editor.paste(&lines, layout);
                    self.message = (skipped > 0).then(|| {
                        format!("{skipped} character(s) have no Atari equivalent and were left out")
                    });
                }
                Err(e) => self.message = Some(format!("Clipboard error: {e}")),
            },
            SelectAll => self.editor.select_all(),
            ToggleWrap => {
                self.config.wrap = match self.config.wrap {
                    WrapMode::Native => WrapMode::Window,
                    WrapMode::Window => WrapMode::Native,
                };
                self.editor.wrap = self.config.wrap;
                self.save_config();
            }
            ZoomIn | ZoomOut => {
                let delta = if c == ZoomIn { 1 } else { -1 };
                let s = (self.config.scale as i32 + delta)
                    .clamp(config::MIN_SCALE as i32, config::MAX_SCALE as i32)
                    as u32;
                if s != self.config.scale {
                    self.config.scale = s;
                    self.save_config();
                }
            }
            ToggleClick => {
                self.config.key_click = !self.config.key_click;
                self.message = Some(format!(
                    "Key click {}",
                    if self.config.key_click { "on" } else { "off" }
                ));
                self.save_config();
            }
            ToggleControlKey => {
                self.config.control_key = match self.config.control_key {
                    ControlKey::RightCtrl => ControlKey::LeftAlt,
                    ControlKey::LeftAlt => ControlKey::RightCtrl,
                };
                self.message = Some(format!(
                    "CONTROL key is now {}",
                    self.config.control_key.label()
                ));
                self.save_config();
            }
            LineWarning => {
                self.prompt = Some(match self.config.line_warning {
                    0 => String::new(),
                    n => n.to_string(),
                });
            }
            KeyHelp => {
                self.overlay = Overlay::Help;
                self.overlay_top = 0;
            }
            About => {
                self.overlay = Overlay::About;
                self.overlay_top = 0;
            }
        }
    }

    // ----- menus -------------------------------------------------------------

    pub fn open_menu(&mut self, i: usize) {
        if i < self.menus.len() {
            self.menu.open = Some(i);
            self.menu.hover = menu::step(&self.menus[i], None, 1);
        }
    }

    fn close_menu(&mut self) {
        self.menu = MenuState::default();
    }

    fn menu_key(&mut self, k: Keycode, ctx: &Ctx) {
        let Some(open) = self.menu.open else { return };
        let n = self.menus.len();
        match k {
            Keycode::ESCAPE | Keycode::F10 => self.close_menu(),
            Keycode::LEFT => self.open_menu((open + n - 1) % n),
            Keycode::RIGHT => self.open_menu((open + 1) % n),
            Keycode::UP | Keycode::DOWN => {
                let dir = if k == Keycode::UP { -1 } else { 1 };
                self.menu.hover = menu::step(&self.menus[open], self.menu.hover, dir);
            }
            Keycode::RETURN | Keycode::KP_ENTER | Keycode::SPACE => {
                let cmd = self
                    .menu
                    .hover
                    .and_then(|h| self.menus[open].items[h].command);
                self.close_menu();
                if let Some(c) = cmd {
                    self.command(c, ctx);
                }
            }
            _ => {}
        }
    }

    // ----- keyboard ------------------------------------------------------------

    fn atari_ctrl_mod(&self) -> Mod {
        match self.config.control_key {
            ControlKey::RightCtrl => Mod::RCTRLMOD,
            ControlKey::LeftAlt => Mod::LALTMOD,
        }
    }

    /// Typed text (from the OS keyboard layout).
    pub fn text_input(&mut self, text: &str, held: Mod, layout: &Layout) {
        if std::mem::take(&mut self.swallow_text) || self.menu.open.is_some() {
            return;
        }
        if let Some(input) = &mut self.prompt {
            for c in text.chars().filter(char::is_ascii_digit) {
                if input.len() < 4 {
                    input.push(c);
                }
            }
            return;
        }
        let ctrl_held = held.intersects(Mod::LCTRLMOD | self.atari_ctrl_mod());
        if self.overlay == Overlay::None && !ctrl_held {
            for c in text.chars() {
                self.machine_key(KeyInput::plain(Key::Char(c)), layout);
            }
        }
    }

    /// Send a key to the machine's keyboard rules and apply the result.
    fn machine_key(&mut self, input: KeyInput, layout: &Layout) {
        let action = self.spec.keyboard.handle(input);
        if action != EditAction::None {
            self.message = None;
            self.quit_armed = false;
        }
        self.editor
            .apply(action, layout, self.spec.machine.as_ref(), input.mods.shift);
        if let Some(note) = self.editor.take_note() {
            self.message = Some(note);
        }
    }

    /// Handle a key press. `inverse_key` is set for the key left of 1.
    /// Returns true if it should click.
    pub fn key_down(&mut self, k: Keycode, m: Mod, inverse_key: bool, ctx: &Ctx) -> bool {
        self.swallow_text = inverse_key;
        let atari_ctrl = m.intersects(self.atari_ctrl_mod());
        let editor_ctrl = m.intersects(Mod::LCTRLMOD) && !atari_ctrl;
        let shift = m.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD);
        let alt = m.intersects(Mod::LALTMOD | Mod::RALTMOD) && !atari_ctrl;
        let layout = &ctx.frame.layout;

        if self.menu.open.is_some() {
            self.menu_key(k, ctx);
            self.swallow_text = true;
            return false;
        }

        if let Some(input) = &mut self.prompt {
            match k {
                Keycode::RETURN | Keycode::KP_ENTER => {
                    let n = input.parse().unwrap_or(0);
                    self.prompt = None;
                    self.config.line_warning = n.min(config::MAX_LINE_WARNING);
                    self.message = Some(if n == 0 {
                        "Line length warning off".into()
                    } else {
                        format!("Line length warning at {n} characters")
                    });
                    self.save_config();
                }
                Keycode::ESCAPE => self.prompt = None,
                Keycode::BACKSPACE => {
                    input.pop();
                }
                _ => {}
            }
            return false;
        }

        if self.overlay != Overlay::None {
            match k {
                Keycode::ESCAPE | Keycode::F1 | Keycode::RETURN => self.overlay = Overlay::None,
                Keycode::UP => self.overlay_top = self.overlay_top.saturating_sub(1),
                Keycode::DOWN => self.overlay_top += 1,
                Keycode::PAGEUP => self.overlay_top = self.overlay_top.saturating_sub(20),
                Keycode::PAGEDOWN => self.overlay_top += 20,
                _ => {}
            }
            return false;
        }

        // Alt+letter opens a menu (unless Alt is the Atari CONTROL key).
        if alt {
            let c = char::from_u32(k.into_i32() as u32).unwrap_or('\0');
            if let Some(i) = self.menus.iter().position(|m| m.mnemonic == c) {
                self.open_menu(i);
                self.swallow_text = true;
            }
            return false;
        }

        // Editor commands (left Ctrl and function keys) never click.
        if editor_ctrl {
            use Command::*;
            let cmd = match k {
                Keycode::N => Some(New),
                Keycode::O => Some(Open),
                Keycode::S if shift => Some(SaveAs),
                Keycode::S => Some(Save),
                Keycode::Q => Some(Quit),
                Keycode::Z if shift => Some(Redo),
                Keycode::Z => Some(Undo),
                Keycode::Y => Some(Redo),
                Keycode::X => Some(Cut),
                Keycode::C => Some(Copy),
                Keycode::V => Some(Paste),
                Keycode::A => Some(SelectAll),
                Keycode::EQUALS | Keycode::PLUS | Keycode::KP_PLUS => Some(ZoomIn),
                Keycode::MINUS | Keycode::KP_MINUS => Some(ZoomOut),
                Keycode::HOME | Keycode::END => {
                    let a = if k == Keycode::HOME {
                        EditAction::DocStart
                    } else {
                        EditAction::DocEnd
                    };
                    self.editor
                        .apply(a, layout, self.spec.machine.as_ref(), shift);
                    None
                }
                _ => None,
            };
            if let Some(c) = cmd {
                self.command(c, ctx);
            }
            return false;
        }

        let fkey = match k {
            Keycode::F1 => Some(Command::KeyHelp),
            Keycode::F2 => Some(Command::Save),
            Keycode::F4 => Some(Command::ToggleWrap),
            Keycode::F7 => Some(Command::ToggleClick),
            Keycode::F8 => Some(Command::ToggleControlKey),
            Keycode::F9 => Some(Command::LineWarning),
            _ => None,
        };
        if let Some(c) = fkey {
            self.command(c, ctx);
            return false;
        }
        if k == Keycode::F10 {
            self.open_menu(0);
            return false;
        }

        let mods = Mods {
            shift,
            control: atari_ctrl,
        };
        let key = match k {
            Keycode::UP => Key::Up,
            Keycode::DOWN => Key::Down,
            Keycode::LEFT => Key::Left,
            Keycode::RIGHT => Key::Right,
            Keycode::HOME => Key::Home,
            Keycode::END => Key::End,
            Keycode::PAGEUP => Key::PageUp,
            Keycode::PAGEDOWN => Key::PageDown,
            Keycode::RETURN | Keycode::KP_ENTER => Key::Enter,
            Keycode::BACKSPACE => Key::Backspace,
            Keycode::DELETE => Key::Delete,
            Keycode::INSERT => Key::Insert,
            Keycode::TAB => Key::Tab,
            Keycode::ESCAPE => Key::Escape,
            Keycode::F6 => Key::Inverse,
            other => {
                let code = other.into_i32();
                let printable = (0x20..0x7F).contains(&code);
                if atari_ctrl && printable {
                    Key::Control(char::from(code as u8))
                } else {
                    // Printable keys arrive as TextInput; still click for them.
                    return printable;
                }
            }
        };
        self.machine_key(KeyInput { key, mods }, layout);
        true
    }

    // ----- mouse -------------------------------------------------------------

    /// Document position under pixel (x, y), clamped into the text area.
    fn pos_at(&self, x: i32, y: i32, f: &Frame) -> Pos {
        let cell = f.cell as i32;
        let col = (x - f.origin_x).div_euclid(cell) - f.layout.left_margin as i32;
        let row = (y - f.text_top).div_euclid(cell);
        let abs = (self.editor.top as i32 + row).max(0) as usize;
        f.layout
            .position_at(&self.editor.doc, abs, col.max(0) as usize)
    }

    pub fn mouse_down(&mut self, x: i32, y: i32, shift: bool, f: &Frame) {
        if let Some(i) = self.geom.title_at(x, y) {
            if self.menu.open == Some(i) {
                self.close_menu();
            } else {
                self.menu = MenuState {
                    open: Some(i),
                    hover: None,
                };
            }
            return;
        }
        if self.menu.open.is_some() {
            if !self.geom.in_dropdown(x, y) {
                self.close_menu();
            }
            return;
        }
        if self.overlay != Overlay::None || self.prompt.is_some() {
            return;
        }
        if y >= f.text_top && y < f.status_top {
            let pos = self.pos_at(x, y, f);
            self.editor.click(pos, shift);
            if !shift {
                self.editor.begin_drag();
            }
            self.dragging = true;
        }
    }

    pub fn mouse_move(&mut self, x: i32, y: i32, left_held: bool, f: &Frame) {
        if self.menu.open.is_some() {
            if let Some(i) = self.geom.title_at(x, y)
                && self.menu.open != Some(i)
            {
                self.menu = MenuState {
                    open: Some(i),
                    hover: None,
                };
            } else if self.geom.in_dropdown(x, y) {
                self.menu.hover = self.geom.item_at(x, y);
            }
            return;
        }
        if self.dragging && left_held {
            let pos = self.pos_at(x, y, f);
            self.editor.click(pos, true);
            self.editor.scroll_to_cursor(&f.layout);
        }
    }

    pub fn mouse_up(&mut self, x: i32, y: i32, ctx: &Ctx) {
        self.dragging = false;
        if let Some(open) = self.menu.open
            && self.geom.in_dropdown(x, y)
        {
            let cmd = self
                .geom
                .item_at(x, y)
                .and_then(|i| self.menus[open].items[i].command);
            if let Some(c) = cmd
                && self.item_state(c, ctx.clipboard).enabled
            {
                self.close_menu();
                self.command(c, ctx);
            }
        }
    }

    pub fn wheel(&mut self, dy: i32, f: &Frame) {
        if self.overlay == Overlay::None {
            self.editor.scroll_by(-dy as isize * 3, &f.layout);
        } else {
            self.overlay_top = (self.overlay_top as isize - dy as isize * 3).max(0) as usize;
        }
    }

    /// Test hook: select the first `n` lines.
    pub fn test_select_lines(&mut self, n: usize) {
        self.editor.click(Pos::default(), false);
        self.editor.click(Pos { line: n, col: 0 }, true);
    }
}

/// Write to a temporary file next to `path`, then rename over it, so a
/// failed write never leaves a half-written file.
fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp~");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}
