//! Editor state, input handling and commands.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use retro_core::{
    Document, EditAction, Editor, Key, KeyInput, Layout, Mods, Pos, WrapMode, clipboard,
};
use sdl2::clipboard::ClipboardUtil;
use sdl2::keyboard::{Keycode, Mod};

use crate::AppSpec;
use crate::render::rgb;
use sdl2::pixels::Color;

/// Attract mode changes colours every 256 frames, as the Atari OS does.
const ATTRACT_STEP: Duration = Duration::from_millis(4272);

/// The colours of the machine's screen for one frame.
pub struct ScreenColors {
    pub text: Color,
    pub background: Color,
    pub border: Color,
    pub warning: Color,
    pub selection: Color,
    pub selection_text: Color,
}
use crate::dialog::{After, Dialog, Purpose};
use std::sync::mpsc;

/// A file dialog running on another thread.
struct FileJob {
    kind: FileJobKind,
    rx: mpsc::Receiver<Option<PathBuf>>,
}

#[derive(Debug, Clone, Copy)]
enum FileJobKind {
    Open,
    /// Choose the default folder (Preferences).
    Folder,
    /// Save As, then optionally carry on with something.
    SaveAs {
        then: Option<After>,
    },
}
use crate::config::{self, Config, ControlKey, DirMode};
use crate::menu::{self, Command, Geometry, ItemState, Menu, MenuState};
use crate::prefs::{self, Field, Op, Prefs};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    None,
    Help,
    Licensing,
}

/// Pixel layout of the window for one frame.
pub struct Frame {
    pub width: u32,
    pub height: u32,
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
    /// The Preferences page, when open.
    pub prefs: Option<Prefs>,
    pub menus: Vec<Menu>,
    pub context_menu: Menu,
    pub menu: MenuState,
    /// Where the menus were last drawn.
    pub geom: Geometry,
    /// Physical pixels per logical pixel (HiDPI).
    pub ratio: f32,
    /// Interface spacing unit and bar height, in pixels.
    pub pad: i32,
    pub bar_height: i32,
    /// The dialog box being shown, if any.
    pub dialog: Option<Dialog>,
    /// A file dialog waiting for an answer.
    file_job: Option<FileJob>,
    /// The last key press was a command, so the text it would type (e.g.
    /// the `` ` `` of the inverse key) must be ignored.
    swallow_text: bool,
    /// Selecting text by dragging with the mouse.
    dragging: bool,
    /// The character set changed, so the glyph texture must be rebuilt.
    pub charset_changed: bool,
    /// When the user last pressed a key or used the mouse.
    last_activity: Instant,
    /// Attract mode's current colour shift, while it's running.
    pub attract: Option<u8>,
    next_attract: Instant,
    rng: u32,
}

impl App {
    pub fn new(mut spec: AppSpec) -> Self {
        let config = Config::load(spec.app_id);
        let charset = config.charset.min(spec.machine.charsets().len() - 1);
        spec.machine.set_charset(charset);
        spec.keyboard.set_charset(charset);
        // The Options menu toggles the second character set, if any.
        let alt_charset = spec
            .machine
            .charsets()
            .get(1)
            .map(|name| &*Box::leak(format!("&{name} Character Set").into_boxed_str()));
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
            prefs: None,
            menus: menu::menus(alt_charset),
            context_menu: menu::context_menu(),
            menu: MenuState::default(),
            geom: Geometry::default(),
            ratio: 1.0,
            pad: 6,
            bar_height: 24,
            dialog: None,
            file_job: None,
            swallow_text: false,
            dragging: false,
            charset_changed: false,
            last_activity: Instant::now(),
            attract: None,
            next_attract: Instant::now(),
            rng: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.subsec_nanos() | 1),
        }
    }

    // ----- colours and attract mode -------------------------------------------

    /// The text and background colours as (hue, lum), if the machine has a
    /// colour model.
    pub fn color_choice(&self) -> Option<((u8, u8), (u8, u8))> {
        let m = self.spec.colors?;
        let fix = |(h, l): (u8, u8)| (h.min(m.hues - 1), l.min(m.lums - 1));
        Some((
            fix(self.config.text_color.unwrap_or(m.default_text)),
            fix(self.config.background_color.unwrap_or(m.default_background)),
        ))
    }

    pub fn screen_colors(&self) -> ScreenColors {
        let pal = self.spec.palette;
        let mut c = ScreenColors {
            text: rgb(pal.foreground),
            background: rgb(pal.background),
            border: rgb(pal.border),
            warning: rgb(pal.warning),
            selection: rgb(pal.selection),
            selection_text: rgb(pal.selection_text),
        };
        if let (Some(m), Some((text, back))) = (self.spec.colors, self.color_choice()) {
            let look = |hl: (u8, u8)| {
                let (h, l) = match self.attract {
                    Some(shift) => (m.attract)(hl, shift),
                    None => hl,
                };
                rgb((m.rgb)(h, l))
            };
            c.text = look(text);
            c.background = look(back);
            c.border = look(m.border);
        }
        if self.attract.is_some() {
            let half = |c: Color| Color::RGB(c.r / 2, c.g / 2, c.b / 2);
            c.warning = half(c.warning);
            c.selection = half(c.selection);
            c.selection_text = half(c.selection_text);
        }
        c
    }

    fn attract_delay(&self) -> Option<Duration> {
        (self.spec.colors.is_some() && self.config.attract_on && self.config.attract_minutes > 0)
            .then(|| Duration::from_secs(60 * u64::from(self.config.attract_minutes)))
    }

    /// The user did something: stop attract mode and restart the timer.
    pub fn activity(&mut self) {
        self.last_activity = Instant::now();
        self.attract = None;
    }

    fn random_byte(&mut self) -> u8 {
        // xorshift32
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 24) as u8
    }

    /// Start attract mode or move it to its next colours when it's time.
    pub fn tick(&mut self) {
        let Some(delay) = self.attract_delay() else {
            self.attract = None;
            return;
        };
        let now = Instant::now();
        if now >= self.last_activity + delay && (self.attract.is_none() || now >= self.next_attract)
        {
            self.attract = Some(self.random_byte());
            self.next_attract = now + ATTRACT_STEP;
        }
    }

    /// How long the event loop may sleep before attract mode needs it.
    pub fn next_wakeup(&self) -> Option<Duration> {
        let delay = self.attract_delay()?;
        let now = Instant::now();
        let at = if self.attract.is_some() {
            self.next_attract
        } else {
            self.last_activity + delay
        };
        Some(at.saturating_duration_since(now))
    }

    /// Test hook: start attract mode now.
    pub fn test_attract(&mut self) {
        self.attract = Some(self.random_byte());
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
            height,
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

    fn file_dialog(&self, title: &str, for_save: bool) -> rfd::FileDialog {
        let (desc, exts) = self.spec.file_types;
        let mut d = rfd::FileDialog::new()
            .set_title(title)
            .add_filter(desc, exts)
            .add_filter("All files", &["*"]);
        // Save As for an existing file starts beside it; otherwise use the
        // default folder from Preferences.
        let beside = self
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .filter(|d| for_save && d.is_dir())
            .map(Path::to_path_buf);
        if let Some(dir) = beside.or_else(|| self.config.start_dir()) {
            d = d.set_directory(dir);
        }
        d
    }

    /// Remember the folder of a file just opened or saved.
    fn note_folder(&mut self, path: &Path) {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            self.config.last_dir = dir.to_string_lossy().into_owned();
        }
    }

    /// Show an Open or Save As dialog. It runs on its own thread so the
    /// window keeps responding; [`App::poll_file_job`] picks up the answer.
    /// (macOS needs dialogs on the main thread, and stays responsive there.)
    fn start_file_job(&mut self, kind: FileJobKind) {
        let d = match kind {
            FileJobKind::SaveAs { .. } => self
                .file_dialog("Save As", true)
                .set_file_name(self.file_name()),
            FileJobKind::Open => self.file_dialog("Open", false),
            FileJobKind::Folder => {
                let mut d = rfd::FileDialog::new().set_title("Default Folder");
                if let Some(dir) = self.config.start_dir() {
                    d = d.set_directory(dir);
                }
                d
            }
        };
        let (tx, rx) = mpsc::channel();
        let run = move || {
            let _ = tx.send(match kind {
                FileJobKind::SaveAs { .. } => d.save_file(),
                FileJobKind::Open => d.pick_file(),
                FileJobKind::Folder => d.pick_folder(),
            });
        };
        if cfg!(target_os = "macos") {
            run();
        } else {
            std::thread::spawn(run);
        }
        self.file_job = Some(FileJob { kind, rx });
        self.message = Some("Waiting for the file dialog...".into());
    }

    pub fn file_job_pending(&self) -> bool {
        self.file_job.is_some()
    }

    /// Act on the file dialog's answer once it arrives.
    pub fn poll_file_job(&mut self) {
        let Some(job) = &self.file_job else { return };
        let result = match job.rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => None,
        };
        let kind = job.kind;
        self.file_job = None;
        self.message = None;
        match (kind, result) {
            (FileJobKind::Open, Some(path)) => {
                self.note_folder(&path);
                self.open_path(&path);
            }
            (FileJobKind::Folder, Some(dir)) => {
                self.config.default_dir = dir.to_string_lossy().into_owned();
                self.config.dir_mode = DirMode::Fixed;
            }
            (FileJobKind::SaveAs { then }, Some(path)) => {
                if self.write_to(&path) {
                    self.note_folder(&path);
                    self.path = Some(path);
                    if let Some(after) = then {
                        self.perform(after);
                    }
                }
            }
            (_, None) => {} // cancelled
        }
    }

    /// Save to the current file (asking for a name if there isn't one),
    /// then do `then` if the save worked.
    fn save(&mut self, then: Option<After>) {
        match self.path.clone() {
            Some(path) => {
                if self.write_to(&path)
                    && let Some(after) = then
                {
                    self.perform(after);
                }
            }
            None => self.start_file_job(FileJobKind::SaveAs { then }),
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

    /// Do `after`, first asking to save unsaved changes.
    fn guard(&mut self, after: After) {
        if self.editor.doc.is_modified() {
            self.dialog = Some(Dialog::save_changes(&self.file_name(), after));
        } else {
            self.perform(after);
        }
    }

    fn perform(&mut self, after: After) {
        match after {
            After::New => {
                self.new_editor(Document::new());
                self.path = None;
                self.message = None;
            }
            After::Open => self.start_file_job(FileJobKind::Open),
            After::Quit => self.running = false,
        }
    }

    /// A dialog button was chosen.
    fn dialog_choose(&mut self, button: usize) {
        let Some(d) = self.dialog.take() else { return };
        match d.purpose {
            Purpose::About => {}
            Purpose::SaveChanges(after) => match button {
                0 => self.save(Some(after)),
                1 => self.perform(after),
                _ => {}
            },
        }
    }

    /// Show the About box (also used by tests).
    pub fn show_about(&mut self) {
        self.dialog = Some(Dialog::about(self.spec.title, &self.spec.about));
    }

    /// Write the settings file now (the "Save Preferences" button).
    fn save_prefs(&mut self) {
        self.message = Some(match self.config.save(self.spec.app_id) {
            Ok(()) => "Preferences saved".into(),
            Err(e) => format!("Preferences not saved: {e}"),
        });
    }

    /// At exit: save everything if "Save preferences on exit" is on.
    /// Otherwise only record that choice, keeping the saved settings.
    pub fn save_prefs_on_exit(&self) -> Result<(), String> {
        if self.config.save_on_exit {
            self.config.save(self.spec.app_id)
        } else {
            let mut saved = Config::load(self.spec.app_id);
            if !saved.save_on_exit {
                return Ok(());
            }
            saved.save_on_exit = false;
            saved.save(self.spec.app_id)
        }
    }

    // ----- settings ----------------------------------------------------------

    fn set_wrap(&mut self, native: bool) {
        self.config.wrap = if native {
            WrapMode::Native
        } else {
            WrapMode::Window
        };
        self.editor.wrap = self.config.wrap;
    }

    fn set_scale(&mut self, scale: i32) {
        self.config.scale = scale.clamp(config::MIN_SCALE as i32, config::MAX_SCALE as i32) as u32;
    }

    fn set_charset(&mut self, index: usize) {
        let index = index % self.spec.machine.charsets().len();
        self.spec.machine.set_charset(index);
        self.spec.keyboard.set_charset(index);
        self.config.charset = index;
        self.charset_changed = true;
    }

    // ----- preferences page ----------------------------------------------------

    pub fn open_prefs(&mut self, focus: Field) {
        self.close_menu();
        self.overlay = Overlay::None;
        let mut p = Prefs::new(self.spec.colors.is_some());
        p.focus_on(focus);
        self.prefs = Some(p);
    }

    fn close_prefs(&mut self) {
        if self.config.line_warning == 0 {
            self.config.line_warning = config::DEFAULT_LINE_WARNING;
        }
        if self.config.attract_minutes == 0 {
            self.config.attract_minutes = config::DEFAULT_ATTRACT_MINUTES;
        }
        self.prefs = None;
    }

    /// The values shown on the Preferences page.
    pub fn prefs_view(&self) -> prefs::View<'_> {
        prefs::View {
            native: self.config.wrap == WrapMode::Native,
            native_label: format!("{} columns", self.spec.machine.screen_columns()),
            scale: self.config.scale,
            charsets: self.spec.machine.charsets(),
            charset: self.spec.machine.charset(),
            key_click: self.config.key_click,
            left_alt: self.config.control_key == ControlKey::LeftAlt,
            warn_on: self.config.line_warning_on,
            warn_col: self.config.line_warning,
            dir_fixed: self.config.dir_mode == DirMode::Fixed,
            dir_path: &self.config.default_dir,
            save_on_exit: self.config.save_on_exit,
            colors: self
                .spec
                .colors
                .zip(self.color_choice())
                .map(|(m, (t, b))| {
                    let show =
                        |(h, l): (u8, u8)| (h, l * m.lum_step, rgb((m.rgb)(h, l)), (m.label)(h, l));
                    prefs::ColorView {
                        text: show(t),
                        background: show(b),
                    }
                }),
            attract_on: self.config.attract_on,
            attract_min: self.config.attract_minutes,
        }
    }

    /// The selected option of a choice field and how many options it has.
    fn choice_state(&self, f: Field) -> Option<(usize, usize)> {
        Some(match f {
            Field::Wrap => (usize::from(self.config.wrap == WrapMode::Window), 2),
            Field::Charset => (
                self.spec.machine.charset(),
                self.spec.machine.charsets().len(),
            ),
            Field::ControlKey => (
                usize::from(self.config.control_key == ControlKey::LeftAlt),
                2,
            ),
            Field::DirMode => (usize::from(self.config.dir_mode == DirMode::Fixed), 2),
            _ => return None,
        })
    }

    /// Change a setting from the Preferences page.
    fn prefs_apply(&mut self, f: Field, op: Op) {
        match (f, op) {
            (Field::Wrap, Op::Choose(i)) => self.set_wrap(i == 0),
            (Field::Zoom, Op::Dec) => self.set_scale(self.config.scale as i32 - 1),
            (Field::Zoom, Op::Inc) => self.set_scale(self.config.scale as i32 + 1),
            (Field::Charset, Op::Choose(i)) => self.set_charset(i),
            (Field::Click, Op::Toggle) => self.config.key_click = !self.config.key_click,
            (Field::ControlKey, Op::Choose(i)) => {
                self.config.control_key = if i == 0 {
                    ControlKey::RightCtrl
                } else {
                    ControlKey::LeftAlt
                }
            }
            (Field::WarnOn, Op::Toggle) => {
                self.config.line_warning_on = !self.config.line_warning_on
            }
            (Field::WarnCol, Op::Dec | Op::Inc) => {
                let d: isize = if op == Op::Inc { 1 } else { -1 };
                self.config.line_warning = (self.config.line_warning as isize + d)
                    .clamp(1, config::MAX_LINE_WARNING as isize)
                    as usize;
            }
            (Field::DirMode, Op::Choose(i)) => {
                self.config.dir_mode = if i == 0 {
                    DirMode::Last
                } else {
                    DirMode::Fixed
                }
            }
            (Field::FgHue | Field::FgLum | Field::BgHue | Field::BgLum, Op::Dec | Op::Inc) => {
                self.step_color(f, op == Op::Inc)
            }
            (Field::ResetColors, Op::Press) => {
                self.config.text_color = None;
                self.config.background_color = None;
            }
            (Field::AttractOn, Op::Toggle) => self.config.attract_on = !self.config.attract_on,
            (Field::AttractMin, Op::Dec | Op::Inc) => {
                let d: i64 = if op == Op::Inc { 1 } else { -1 };
                self.config.attract_minutes = (i64::from(self.config.attract_minutes) + d)
                    .clamp(1, i64::from(config::MAX_ATTRACT_MINUTES))
                    as u32;
            }
            (Field::Browse, Op::Press) => self.start_file_job(FileJobKind::Folder),
            (Field::SaveOnExit, Op::Toggle) => self.config.save_on_exit = !self.config.save_on_exit,
            (Field::SaveNow, Op::Press) => self.save_prefs(),
            (Field::Close, Op::Press) => self.close_prefs(),
            _ => {}
        }
    }

    /// Step a hue (wrapping round) or luminance (stopping at the ends).
    fn step_color(&mut self, f: Field, up: bool) {
        let (Some(m), Some((text, back))) = (self.spec.colors, self.color_choice()) else {
            return;
        };
        let text_field = matches!(f, Field::FgHue | Field::FgLum);
        let (mut h, mut l) = if text_field { text } else { back };
        if matches!(f, Field::FgHue | Field::BgHue) {
            h = if up {
                (h + 1) % m.hues
            } else {
                (h + m.hues - 1) % m.hues
            };
        } else {
            l = if up {
                (l + 1).min(m.lums - 1)
            } else {
                l.saturating_sub(1)
            };
        }
        if text_field {
            self.config.text_color = Some((h, l));
        } else {
            self.config.background_color = Some((h, l));
        }
    }

    /// A key pressed while Preferences is open. Returns true if the key was
    /// used up (so the text it types should be ignored).
    fn prefs_key(&mut self, k: Keycode, shift: bool) -> bool {
        let Some(p) = &mut self.prefs else {
            return false;
        };
        let f = p.focused();
        let text_field = matches!(f, Field::DirPath | Field::WarnCol | Field::AttractMin);
        match k {
            Keycode::TAB => p.move_focus(if shift { -1 } else { 1 }),
            Keycode::UP => p.move_focus(-1),
            Keycode::DOWN => p.move_focus(1),
            Keycode::ESCAPE => self.close_prefs(),
            Keycode::RETURN | Keycode::KP_ENTER => match f {
                Field::Browse | Field::SaveNow | Field::Close => self.prefs_apply(f, Op::Press),
                _ => self.close_prefs(),
            },
            Keycode::LEFT | Keycode::RIGHT => {
                let fwd = k == Keycode::RIGHT;
                if let Some((cur, n)) = self.choice_state(f) {
                    let next = if fwd {
                        (cur + 1) % n
                    } else {
                        (cur + n - 1) % n
                    };
                    self.prefs_apply(f, Op::Choose(next));
                } else if matches!(
                    f,
                    Field::Zoom
                        | Field::WarnCol
                        | Field::AttractMin
                        | Field::FgHue
                        | Field::FgLum
                        | Field::BgHue
                        | Field::BgLum
                ) {
                    self.prefs_apply(f, if fwd { Op::Inc } else { Op::Dec });
                }
            }
            Keycode::SPACE if !text_field => {
                if let Some((cur, n)) = self.choice_state(f) {
                    self.prefs_apply(f, Op::Choose((cur + 1) % n));
                } else {
                    self.prefs_apply(f, Op::Toggle);
                    self.prefs_apply(f, Op::Press);
                }
            }
            Keycode::BACKSPACE => match f {
                Field::DirPath => {
                    self.config.default_dir.pop();
                }
                Field::WarnCol => self.config.line_warning /= 10,
                Field::AttractMin => self.config.attract_minutes /= 10,
                _ => {}
            },
            _ => return !text_field,
        }
        true
    }

    /// Text typed while Preferences is open (folder path, column number).
    fn prefs_text(&mut self, text: &str) {
        let Some(p) = &self.prefs else { return };
        match p.focused() {
            Field::DirPath => {
                self.config.default_dir.push_str(text);
                self.config.dir_mode = DirMode::Fixed;
            }
            Field::AttractMin => {
                for d in text.chars().filter_map(|c| c.to_digit(10)) {
                    let n = self.config.attract_minutes * 10 + d;
                    if n <= config::MAX_ATTRACT_MINUTES {
                        self.config.attract_minutes = n;
                    }
                }
            }
            Field::WarnCol => {
                for d in text.chars().filter_map(|c| c.to_digit(10)) {
                    let n = self.config.line_warning * 10 + d as usize;
                    if n <= config::MAX_LINE_WARNING {
                        self.config.line_warning = n;
                    }
                }
            }
            _ => {}
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
            ToggleCharset => Some(self.spec.machine.charset() != 0),
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
        if self.dialog.is_some() || self.file_job.is_some() || self.prefs.is_some() {
            return;
        }
        match c {
            New => self.guard(After::New),
            Open => self.guard(After::Open),
            Save => self.save(None),
            SaveAs => self.start_file_job(FileJobKind::SaveAs { then: None }),
            Quit => self.guard(After::Quit),
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
            ToggleWrap => self.set_wrap(self.config.wrap != WrapMode::Native),
            ZoomIn => self.set_scale(self.config.scale as i32 + 1),
            ZoomOut => self.set_scale(self.config.scale as i32 - 1),
            ToggleClick => {
                self.config.key_click = !self.config.key_click;
                self.message = Some(format!(
                    "Key click {}",
                    if self.config.key_click { "on" } else { "off" }
                ));
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
            }
            ToggleCharset => {
                self.set_charset(self.spec.machine.charset() + 1);
                let m = &self.spec.machine;
                self.message = Some(format!("{} character set", m.charsets()[m.charset()]));
            }
            Preferences => self.open_prefs(Field::Wrap),
            KeyHelp => {
                self.overlay = Overlay::Help;
                self.overlay_top = 0;
            }
            Licensing => {
                self.overlay = Overlay::Licensing;
                self.overlay_top = 0;
            }
            About => self.show_about(),
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

    /// The menu currently showing its items (menu-bar or right-click).
    fn open_items(&self) -> Option<&Menu> {
        match (self.menu.open, self.menu.context) {
            (Some(i), _) => Some(&self.menus[i]),
            (None, Some(_)) => Some(&self.context_menu),
            (None, None) => None,
        }
    }

    /// The command of item `i` in the open menu.
    fn open_command(&self, i: usize) -> Option<Command> {
        self.open_items()?.items.get(i)?.command
    }

    fn menu_key(&mut self, k: Keycode, ctx: &Ctx) {
        let n = self.menus.len();
        match k {
            Keycode::ESCAPE | Keycode::F10 => self.close_menu(),
            Keycode::LEFT | Keycode::RIGHT => {
                if let Some(open) = self.menu.open {
                    let next = if k == Keycode::LEFT {
                        open + n - 1
                    } else {
                        open + 1
                    };
                    self.open_menu(next % n);
                }
            }
            Keycode::UP | Keycode::DOWN => {
                let dir = if k == Keycode::UP { -1 } else { 1 };
                if let Some(m) = self.open_items() {
                    self.menu.hover = menu::step(m, self.menu.hover, dir);
                }
            }
            Keycode::RETURN | Keycode::KP_ENTER | Keycode::SPACE => {
                let cmd = self.menu.hover.and_then(|h| self.open_command(h));
                self.close_menu();
                if let Some(c) = cmd {
                    self.command(c, ctx);
                }
            }
            other => {
                // The item's underlined hot key chooses it.
                let key = char::from_u32(other.into_i32() as u32).map(|c| c.to_ascii_lowercase());
                let cmd = self.open_items().and_then(|m| {
                    m.items
                        .iter()
                        .find(|it| it.command.is_some() && menu::hot_key(it.label) == key)
                        .and_then(|it| it.command)
                });
                if let Some(c) = cmd
                    && self.item_state(c, ctx.clipboard).enabled
                {
                    self.close_menu();
                    self.command(c, ctx);
                }
            }
        }
    }

    // ----- keyboard ------------------------------------------------------------

    fn atari_ctrl_mod(&self) -> Mod {
        match self.config.control_key {
            ControlKey::RightCtrl => Mod::RCTRLMOD,
            ControlKey::LeftAlt => Mod::LALTMOD,
        }
    }

    /// Typed text (from the OS keyboard layout). Returns true if it
    /// typed anything (and so should click).
    pub fn text_input(&mut self, text: &str, held: Mod, layout: &Layout) -> bool {
        if std::mem::take(&mut self.swallow_text)
            || self.menu.is_open()
            || self.dialog.is_some()
            || self.file_job.is_some()
        {
            return false;
        }
        if self.prefs.is_some() {
            self.prefs_text(text);
            return false;
        }
        let ctrl_held = held.intersects(Mod::LCTRLMOD | self.atari_ctrl_mod());
        let mut typed = false;
        if self.overlay == Overlay::None && !ctrl_held {
            for c in text.chars() {
                typed |= self.machine_key(KeyInput::plain(Key::Char(c)), layout);
            }
        }
        typed
    }

    /// Send a key to the machine's keyboard rules and apply the result.
    /// Returns false if the key means nothing to the machine.
    fn machine_key(&mut self, input: KeyInput, layout: &Layout) -> bool {
        let action = self.spec.keyboard.handle(input);
        if action != EditAction::None {
            self.message = None;
        }
        self.editor
            .apply(action, layout, self.spec.machine.as_ref(), input.mods.shift);
        if let Some(note) = self.editor.take_note() {
            self.message = Some(note);
        }
        action != EditAction::None
    }

    /// Handle a key press. `inverse_key` is set for the key left of 1.
    /// Returns true if it should click.
    pub fn key_down(&mut self, k: Keycode, m: Mod, inverse_key: bool, ctx: &Ctx) -> bool {
        self.swallow_text = inverse_key;
        // The number pad is a second set of cursor keys when NumLock is off,
        // or when Shift is held with NumLock on (the standard PC rule). In
        // the second case Shift is used up by the switch, so it moves the
        // cursor without selecting, and the digit the OS types is dropped.
        let shift_mods = Mod::LSHIFTMOD | Mod::RSHIFTMOD;
        let (k, m) = match numpad_navigation(k) {
            Some(nav) if !m.contains(Mod::NUMMOD) => (nav, m),
            Some(nav) if m.intersects(shift_mods) => {
                self.swallow_text = true;
                (nav, m & !shift_mods)
            }
            _ => (k, m),
        };
        let atari_ctrl = m.intersects(self.atari_ctrl_mod());
        let editor_ctrl = m.intersects(Mod::LCTRLMOD) && !atari_ctrl;
        let shift = m.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD);
        let alt = m.intersects(Mod::LALTMOD | Mod::RALTMOD) && !atari_ctrl;
        let layout = &ctx.frame.layout;

        if self.file_job.is_some() {
            return false;
        }
        if let Some(d) = &mut self.dialog {
            let n = d.buttons.len();
            match k {
                Keycode::LEFT => d.focus = (d.focus + n - 1) % n,
                Keycode::TAB if shift => d.focus = (d.focus + n - 1) % n,
                Keycode::RIGHT | Keycode::TAB => d.focus = (d.focus + 1) % n,
                Keycode::RETURN | Keycode::KP_ENTER | Keycode::SPACE => {
                    let f = d.focus;
                    self.dialog_choose(f);
                }
                Keycode::ESCAPE => {
                    let c = d.cancel;
                    self.dialog_choose(c);
                }
                _ => {}
            }
            self.swallow_text = true;
            return false;
        }
        if self.menu.is_open() {
            self.menu_key(k, ctx);
            self.swallow_text = true;
            return false;
        }

        if self.prefs.is_some() {
            self.swallow_text = self.prefs_key(k, shift);
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
            if let Some(i) = self
                .menus
                .iter()
                .position(|m| menu::hot_key(m.title) == Some(c))
            {
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
                Keycode::COMMA => Some(Preferences),
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

        // Function keys only act on their own: Shift+F4 is not F4.
        let plain = !shift && !alt && !atari_ctrl;
        let fkey = match k {
            _ if !plain => None,
            Keycode::F1 => Some(Command::KeyHelp),
            Keycode::F2 => Some(Command::Save),
            Keycode::F4 => Some(Command::ToggleWrap),
            Keycode::F6 => Some(Command::ToggleCharset),
            Keycode::F7 => Some(Command::ToggleClick),
            Keycode::F8 => Some(Command::ToggleControlKey),
            Keycode::F9 => {
                self.open_prefs(Field::WarnOn);
                return false;
            }
            _ => None,
        };
        if let Some(c) = fkey {
            self.command(c, ctx);
            return false;
        }
        if k == Keycode::F10 && plain {
            self.open_menu(0);
            return false;
        }

        let mods = Mods {
            shift,
            control: atari_ctrl,
        };
        let key = match k {
            _ if inverse_key => Key::Inverse,
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
            // Caps Lock is handled by the OS but clicks like an Atari key.
            Keycode::CAPSLOCK => return true,
            other => {
                let code = other.into_i32();
                let printable = (0x20..0x7F).contains(&code);
                if atari_ctrl && printable {
                    Key::Control(char::from(code as u8))
                } else {
                    // Printable keys arrive as TextInput, which decides
                    // whether to click; other keys (F-keys with Shift, etc.)
                    // do nothing.
                    return false;
                }
            }
        };
        self.machine_key(KeyInput { key, mods }, layout)
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
        if self.file_job.is_some() {
            return;
        }
        if let Some(d) = &self.dialog {
            if let Some(b) = d.button_at(x, y) {
                self.dialog_choose(b);
            }
            return;
        }
        if let Some(i) = self.geom.title_at(x, y) {
            if self.menu.open == Some(i) {
                self.close_menu();
            } else {
                self.menu = MenuState {
                    open: Some(i),
                    ..Default::default()
                };
            }
            return;
        }
        if self.menu.is_open() {
            if !self.geom.in_dropdown(x, y) {
                self.close_menu();
            }
            return;
        }
        if let Some(p) = &mut self.prefs {
            if let Some((f, op)) = p.hit(x, y) {
                p.focus_on(f);
                self.prefs_apply(f, op);
            }
            return;
        }
        if self.overlay != Overlay::None {
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

    /// Right button: move the cursor there (unless clicking inside the
    /// selection) and open the right-click menu.
    pub fn mouse_right(&mut self, x: i32, y: i32, f: &Frame) {
        self.close_menu();
        if self.dialog.is_some() || self.file_job.is_some() {
            return;
        }
        if self.overlay != Overlay::None
            || self.prefs.is_some()
            || y < f.text_top
            || y >= f.status_top
        {
            return;
        }
        let pos = self.pos_at(x, y, f);
        let in_selection = self
            .editor
            .selection()
            .is_some_and(|(a, b)| pos >= a && pos <= b);
        if !in_selection {
            self.editor.click(pos, false);
        }
        self.menu.context = Some((x, y));
    }

    pub fn mouse_move(&mut self, x: i32, y: i32, left_held: bool, f: &Frame) {
        if self.menu.is_open() {
            if self.menu.open.is_some()
                && let Some(i) = self.geom.title_at(x, y)
                && self.menu.open != Some(i)
            {
                self.menu = MenuState {
                    open: Some(i),
                    ..Default::default()
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
        if std::mem::take(&mut self.dragging) && !self.editor.has_selection() {
            // A click without a drag selects nothing.
            self.editor.clear_selection();
        }
        if self.menu.is_open() && self.geom.in_dropdown(x, y) {
            let cmd = self.geom.item_at(x, y).and_then(|i| self.open_command(i));
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

/// The navigation key a number-pad key gives with NumLock off.
fn numpad_navigation(k: Keycode) -> Option<Keycode> {
    Some(match k {
        Keycode::KP_8 => Keycode::UP,
        Keycode::KP_2 => Keycode::DOWN,
        Keycode::KP_4 => Keycode::LEFT,
        Keycode::KP_6 => Keycode::RIGHT,
        Keycode::KP_7 => Keycode::HOME,
        Keycode::KP_1 => Keycode::END,
        Keycode::KP_9 => Keycode::PAGEUP,
        Keycode::KP_3 => Keycode::PAGEDOWN,
        Keycode::KP_0 => Keycode::INSERT,
        Keycode::KP_PERIOD => Keycode::DELETE,
        _ => return None,
    })
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
