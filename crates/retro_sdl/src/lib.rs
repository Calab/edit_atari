//! SDL2 front end shared by the retro text editors.
//!
//! A machine-specific program (e.g. `atari_editor`) fills in an [`AppSpec`]
//! and calls [`run`]. Everything here is machine-independent.

pub mod config;
pub mod render;

use std::path::{Path, PathBuf};

use retro_core::{
    Document, EditAction, Editor, Key, KeyInput, KeyboardRules, Layout, Machine, Mods, WrapMode,
};
use sdl2::audio::{AudioQueue, AudioSpecDesired};
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::{Keycode, Mod, Scancode};
use sdl2::mouse::MouseButton;
use sdl2::pixels::Color;

pub use config::{Config, ControlKey};
use render::{Grid, Palette, rgb};

/// Everything that makes an editor specific to one machine.
pub struct AppSpec {
    /// Window title, e.g. "Atari Editor".
    pub title: &'static str,
    /// Folder name for settings, e.g. "atari_editor".
    pub app_id: &'static str,
    pub machine: Box<dyn Machine>,
    pub keyboard: Box<dyn KeyboardRules>,
    /// Generates the key-click sound: (sample rate, volume) -> mono samples.
    pub click: fn(u32, f32) -> Vec<f32>,
    pub palette: Palette,
    /// Machine-specific lines appended to the help screen.
    pub help: &'static [&'static str],
    /// Licence/credits text shown by the About screen.
    pub about: &'static str,
}

/// Rows reserved below the text for the status bar.
const STATUS_ROWS: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Overlay {
    None,
    Help,
    About,
}

struct App {
    spec: AppSpec,
    config: Config,
    editor: Editor,
    path: Option<PathBuf>,
    message: Option<String>,
    /// Quit was requested with unsaved changes; a second request quits.
    quit_armed: bool,
    overlay: Overlay,
    overlay_top: usize,
    running: bool,
    /// The last key press was handled as a command, so the text it would
    /// type (e.g. the `` ` `` of the inverse key) must be ignored.
    swallow_text: bool,
    /// A number being typed into the status bar (F9), if any.
    prompt: Option<String>,
}

/// Open `file` (if given) and run the editor until the user quits.
pub fn run(spec: AppSpec, file: Option<PathBuf>) -> Result<(), String> {
    let config = Config::load(spec.app_id);
    let mut app = App {
        editor: Editor::new(
            Document::new(),
            config.wrap,
            spec.machine.default_tab_stops(),
        ),
        config,
        spec,
        path: None,
        message: None,
        quit_armed: false,
        overlay: Overlay::None,
        swallow_text: false,
        prompt: None,
        overlay_top: 0,
        running: true,
    };
    if let Some(p) = file {
        app.open(&p);
    }

    let sdl = sdl2::init()?;
    let video = sdl.video()?;

    // Pick a starting size: the native screen plus a border, scaled down if
    // it doesn't fit the display.
    let native_cols = app.spec.machine.screen_columns() as u32 + 4;
    let native_rows = 24 + STATUS_ROWS as u32 + 2;
    if let Ok(bounds) = video.display_usable_bounds(0) {
        while app.config.scale > config::MIN_SCALE
            && (native_cols * 8 * app.config.scale > bounds.width()
                || native_rows * 8 * app.config.scale > bounds.height())
        {
            app.config.scale -= 1;
        }
    }
    let s = app.config.scale * 8;
    let window = video
        .window(app.spec.title, native_cols * s, native_rows * s)
        .position_centered()
        .resizable()
        .allow_highdpi()
        .build()
        .map_err(|e| e.to_string())?;
    let mut canvas = window
        .into_canvas()
        .present_vsync()
        .build()
        .map_err(|e| e.to_string())?;
    let creator = canvas.texture_creator();
    let mut atlas = render::build_atlas(&creator, app.spec.machine.as_ref())?;

    // Audio is optional: carry on silently if it can't be opened.
    let audio = sdl.audio().ok();
    let queue: Option<AudioQueue<f32>> = audio.as_ref().and_then(|a| {
        let want = AudioSpecDesired {
            freq: Some(44_100),
            channels: Some(1),
            samples: Some(256),
        };
        let q = a.open_queue::<f32, _>(None, &want).ok()?;
        q.resume();
        Some(q)
    });
    let click = queue
        .as_ref()
        .map(|q| (app.spec.click)(q.spec().freq as u32, app.config.click_volume));

    let keyboard_util = sdl.keyboard();
    let mut events = sdl.event_pump()?;

    let mut first_frame = true;
    while app.running {
        // Draw the first frame straight away; after that, sleep until
        // something happens (the Atari cursor doesn't blink).
        let mut pending = if first_frame {
            Vec::new()
        } else {
            vec![events.wait_event()]
        };
        first_frame = false;
        pending.extend(events.poll_iter());

        for event in pending {
            // Size of the text area in cells, needed to lay out actions.
            let (pw, ph) = canvas.output_size()?;
            let cell = app.config.scale * 8;
            let win_cols = (pw / cell) as usize;
            let text_rows = ((ph / cell) as usize).saturating_sub(STATUS_ROWS);
            let layout = app
                .editor
                .layout(app.spec.machine.as_ref(), win_cols, text_rows);

            match event {
                Event::Quit { .. } => app.request_quit(),
                Event::Window {
                    win_event: WindowEvent::SizeChanged(..),
                    ..
                } => {
                    app.editor.scroll_to_cursor(&layout);
                }
                Event::KeyDown {
                    keycode: Some(k),
                    scancode,
                    keymod,
                    ..
                } => {
                    app.swallow_text = false;
                    // The key left of 1 (`/~ on US keyboards) is the Atari
                    // inverse key, wherever the layout puts its symbols.
                    let k = if scancode == Some(Scancode::Grave) {
                        app.swallow_text = true;
                        Keycode::F6
                    } else {
                        k
                    };
                    let clicked = app.key_down(k, keymod, &layout);
                    if clicked
                        && app.config.key_click
                        && let (Some(q), Some(samples)) = (&queue, &click)
                    {
                        // Drop any backlog so clicks never lag behind typing.
                        if q.size() > 4096 {
                            q.clear();
                        }
                        let _ = q.queue_audio(samples);
                    }
                }
                Event::TextInput { text, .. } => {
                    let held = keyboard_util.mod_state();
                    let swallow = std::mem::take(&mut app.swallow_text);
                    if let Some(input) = &mut app.prompt {
                        for c in text.chars().filter(char::is_ascii_digit) {
                            if input.len() < 4 {
                                input.push(c);
                            }
                        }
                    } else if app.overlay == Overlay::None && !app.ctrl_held(held) && !swallow {
                        for c in text.chars() {
                            app.machine_key(KeyInput::plain(Key::Char(c)), &layout);
                        }
                    }
                }
                Event::MouseButtonDown {
                    mouse_btn: MouseButton::Left,
                    x,
                    y,
                    ..
                } => {
                    let (ww, _) = canvas.window().size();
                    let ratio = if ww > 0 { pw as f32 / ww as f32 } else { 1.0 };
                    let (px, py) = ((x as f32 * ratio) as i32, (y as f32 * ratio) as i32);
                    let origin = app.origin_x(pw, cell, &layout);
                    let col = (px - origin).div_euclid(cell as i32) - layout.left_margin as i32;
                    let row = py / cell as i32;
                    if app.overlay == Overlay::None && row >= 0 && (row as usize) < layout.rows {
                        let pos = layout.position_at(
                            &app.editor.doc,
                            app.editor.top + row as usize,
                            col.max(0) as usize,
                        );
                        app.editor.doc.set_cursor(pos);
                    }
                }
                Event::MouseWheel { y, .. } => {
                    if app.overlay == Overlay::None {
                        app.editor.scroll_by(-y as isize * 3, &layout);
                    } else {
                        app.overlay_top =
                            (app.overlay_top as isize - y as isize * 3).max(0) as usize;
                    }
                }
                _ => {}
            }
        }

        app.draw(&mut canvas, &mut atlas)?;
        if let Some(path) = std::env::var_os("RETRO_EDITOR_SNAPSHOT") {
            // Test hook: save the first frame as a BMP and exit.
            save_snapshot(&canvas, Path::new(&path))?;
            break;
        }
        canvas.present();
        let title = format!(
            "{}{} - {}",
            app.file_name(),
            if app.editor.doc.is_modified() {
                "*"
            } else {
                ""
            },
            app.spec.title
        );
        if canvas.window().title() != title {
            let _ = canvas.window_mut().set_title(&title);
        }
    }

    if let Err(e) = app.config.save(app.spec.app_id) {
        eprintln!("could not save settings: {e}");
    }
    Ok(())
}

impl App {
    fn ctrl_held(&self, m: Mod) -> bool {
        m.intersects(Mod::LCTRLMOD) || m.intersects(self.atari_ctrl_mod())
    }

    fn atari_ctrl_mod(&self) -> Mod {
        match self.config.control_key {
            ControlKey::RightCtrl => Mod::RCTRLMOD,
            ControlKey::LeftAlt => Mod::LALTMOD,
        }
    }

    fn file_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into())
    }

    fn open(&mut self, path: &Path) {
        match std::fs::read(path) {
            Ok(bytes) => {
                let d = self.spec.machine.decode(&bytes);
                let mut doc = Document::from_lines(d.lines, d.final_eol);
                if let Some(from) = d.converted_from {
                    doc.mark_modified();
                    self.message = Some(format!("Converted from {from}; will save as native"));
                }
                self.editor = Editor::new(doc, self.config.wrap, self.editor.tabs.clone());
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.editor =
                    Editor::new(Document::new(), self.config.wrap, self.editor.tabs.clone());
                self.message = Some("New file".into());
            }
            Err(e) => {
                self.message = Some(format!("Can't open: {e}"));
                return;
            }
        }
        self.path = Some(path.to_path_buf());
    }

    fn save(&mut self) {
        let Some(path) = self.path.clone() else {
            self.message = Some("No file name: start with a file name for now".into());
            return;
        };
        let bytes = self.spec.machine.encode(&self.editor.doc);
        match write_atomically(&path, &bytes) {
            Ok(()) => {
                self.editor.doc.mark_saved();
                self.message = Some(format!("Saved {} bytes", bytes.len()));
            }
            Err(e) => self.message = Some(format!("Save failed: {e}")),
        }
    }

    fn request_quit(&mut self) {
        if self.editor.doc.is_modified() && !self.quit_armed {
            self.quit_armed = true;
            self.message = Some("Unsaved changes! Ctrl+Q again to quit, Ctrl+S to save".into());
        } else {
            self.running = false;
        }
    }

    fn save_config(&mut self) {
        if let Err(e) = self.config.save(self.spec.app_id) {
            self.message = Some(format!("Settings not saved: {e}"));
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
            .apply(action, layout, self.spec.machine.as_ref());
        if let Some(note) = self.editor.take_note() {
            self.message = Some(note);
        }
    }

    /// Handle a key press. Returns true if it should click.
    fn key_down(&mut self, k: Keycode, m: Mod, layout: &Layout) -> bool {
        let atari_ctrl = m.intersects(self.atari_ctrl_mod());
        let editor_ctrl = m.intersects(Mod::LCTRLMOD) && !atari_ctrl;
        let shift = m.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD);

        if let Some(input) = &mut self.prompt {
            match k {
                Keycode::RETURN | Keycode::KP_ENTER => {
                    let n = input.parse().unwrap_or(0);
                    self.prompt = None;
                    self.config.line_warning = n;
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
                Keycode::ESCAPE | Keycode::F1 | Keycode::F10 | Keycode::RETURN => {
                    self.overlay = Overlay::None
                }
                Keycode::UP => self.overlay_top = self.overlay_top.saturating_sub(1),
                Keycode::DOWN => self.overlay_top += 1,
                Keycode::PAGEUP => self.overlay_top = self.overlay_top.saturating_sub(20),
                Keycode::PAGEDOWN => self.overlay_top += 20,
                _ => {}
            }
            return false;
        }

        // Editor commands (left Ctrl and function keys) never click.
        if editor_ctrl {
            match k {
                Keycode::S => self.save(),
                Keycode::Q => self.request_quit(),
                Keycode::EQUALS | Keycode::PLUS | Keycode::KP_PLUS => self.zoom(1),
                Keycode::MINUS | Keycode::KP_MINUS => self.zoom(-1),
                Keycode::HOME => {
                    self.editor
                        .apply(EditAction::DocStart, layout, self.spec.machine.as_ref());
                }
                Keycode::END => {
                    self.editor
                        .apply(EditAction::DocEnd, layout, self.spec.machine.as_ref());
                }
                _ => {}
            }
            return false;
        }
        match k {
            Keycode::F1 => {
                self.overlay = Overlay::Help;
                self.overlay_top = 0;
                return false;
            }
            Keycode::F2 => {
                self.save();
                return false;
            }
            Keycode::F4 => {
                self.config.wrap = match self.config.wrap {
                    WrapMode::Native => WrapMode::Window,
                    WrapMode::Window => WrapMode::Native,
                };
                self.editor.wrap = self.config.wrap;
                self.save_config();
                return false;
            }
            Keycode::F7 => {
                self.config.key_click = !self.config.key_click;
                self.message = Some(format!(
                    "Key click {}",
                    if self.config.key_click { "on" } else { "off" }
                ));
                self.save_config();
                return false;
            }
            Keycode::F8 => {
                self.config.control_key = match self.config.control_key {
                    ControlKey::RightCtrl => ControlKey::LeftAlt,
                    ControlKey::LeftAlt => ControlKey::RightCtrl,
                };
                self.message = Some(format!(
                    "CONTROL key is now {}",
                    self.config.control_key.label()
                ));
                self.save_config();
                return false;
            }
            Keycode::F9 => {
                self.prompt = Some(match self.config.line_warning {
                    0 => String::new(),
                    n => n.to_string(),
                });
                return false;
            }
            Keycode::F10 => {
                self.overlay = Overlay::About;
                self.overlay_top = 0;
                return false;
            }
            _ => {}
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

    fn zoom(&mut self, delta: i32) {
        let s = (self.config.scale as i32 + delta)
            .clamp(config::MIN_SCALE as i32, config::MAX_SCALE as i32) as u32;
        if s != self.config.scale {
            self.config.scale = s;
            self.save_config();
        }
    }

    /// X pixel where column 0 is drawn (centres the native screen).
    fn origin_x(&self, pw: u32, cell: u32, layout: &Layout) -> i32 {
        match self.editor.wrap {
            WrapMode::Native => {
                let w = (layout.left_margin + layout.text_cols) as u32 * cell;
                (pw.saturating_sub(w) / 2) as i32
            }
            WrapMode::Window => 0,
        }
    }

    fn ascii(&self, s: &str) -> Vec<u8> {
        s.chars().map(|c| self.spec.machine.ascii_code(c)).collect()
    }

    fn draw(
        &mut self,
        canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
        atlas: &mut sdl2::render::Texture,
    ) -> Result<(), String> {
        let (pw, ph) = canvas.output_size()?;
        let cell = self.config.scale * 8;
        let win_cols = (pw / cell) as usize;
        let win_rows = (ph / cell) as usize;
        let text_rows = win_rows.saturating_sub(STATUS_ROWS);
        let layout = self
            .editor
            .layout(self.spec.machine.as_ref(), win_cols, text_rows);
        let pal: Palette = self.spec.palette;
        let (bg, fg) = (rgb(pal.background), rgb(pal.foreground));

        canvas.set_draw_color(rgb(pal.border));
        canvas.clear();
        let origin_x = self.origin_x(pw, cell, &layout);
        let screen_cols = layout.left_margin + layout.text_cols;

        // Text area background.
        canvas.set_draw_color(bg);
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(
            origin_x,
            0,
            screen_cols as u32 * cell,
            text_rows as u32 * cell,
        ));

        let mut grid = Grid {
            canvas,
            atlas,
            cell,
            origin_x,
        };

        if self.overlay != Overlay::None {
            let text = match self.overlay {
                Overlay::Help => self.help_text(),
                _ => self.spec.about.to_string(),
            };
            let lines: Vec<&str> = text.lines().collect();
            self.overlay_top = self.overlay_top.min(lines.len().saturating_sub(text_rows));
            for (r, line) in lines
                .iter()
                .skip(self.overlay_top)
                .take(text_rows)
                .enumerate()
            {
                let codes = self.ascii(line);
                let w = codes.len().min(screen_cols.saturating_sub(1));
                grid.codes(1, r, &codes[..w], fg);
            }
        } else {
            self.draw_text(&mut grid, &layout, fg, bg);
        }

        // Status bar across the full window width, in inverse colours.
        let status_row = text_rows;
        grid.origin_x = 0;
        grid.fill(0, status_row, win_cols.max(1), fg);
        let status = self.status_line(win_cols);
        grid.codes(0, status_row, &status, bg);

        Ok(())
    }

    fn draw_text(&self, grid: &mut Grid, layout: &Layout, fg: Color, bg: Color) {
        let doc = &self.editor.doc;
        let w = layout.text_cols;
        let warn_col = match self.config.line_warning {
            0 => usize::MAX,
            n => n,
        };
        let warn = rgb(self.spec.palette.warning);
        let mut row = 0usize; // absolute screen row of the current line's first row
        for line in doc.lines() {
            let rows = layout.rows_for_len(line.len());
            if row + rows > self.editor.top {
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
                    let x = layout.left_margin + split - start;
                    grid.codes(x, screen, &line[split..end], warn);
                }
            }
            row += rows;
            if row >= self.editor.top + layout.rows {
                break;
            }
        }

        // Cursor: the character under it drawn in inverse colours.
        let cur = doc.cursor();
        let (crow, cx) = layout.locate(doc, cur);
        if crow >= self.editor.top && crow < self.editor.top + layout.rows {
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

    fn status_line(&self, width: usize) -> Vec<u8> {
        if let Some(input) = &self.prompt {
            let mut codes = self.ascii(&format!(
                " Line length warning (0=off): {input}_   Enter=OK  Esc=cancel"
            ));
            codes.truncate(width);
            return codes;
        }
        let doc = &self.editor.doc;
        let cur = doc.cursor();
        let left = match &self.message {
            Some(m) => format!(" {m}"),
            None => format!(
                " {}{}",
                self.file_name(),
                if doc.is_modified() { "*" } else { "" }
            ),
        };
        let mut flags = vec![format!("L{} C{}", cur.line + 1, cur.col + 1)];
        let len = doc.line(cur.line).len();
        if self.config.line_warning > 0 && len > self.config.line_warning {
            flags.push(format!("LEN {len}!"));
        }
        flags.push(if doc.overwrite { "OVR" } else { "INS" }.into());
        flags.extend(
            self.spec
                .keyboard
                .indicators()
                .iter()
                .map(|s| s.to_string()),
        );
        flags.push(match self.editor.wrap {
            WrapMode::Native => format!("{}COL", self.spec.machine.screen_columns()),
            WrapMode::Window => "WIDE".into(),
        });
        flags.push("F1:HELP ".into());
        let right = flags.join("  ");

        let mut line = left;
        let pad = width.saturating_sub(line.len() + right.len());
        if pad == 0 {
            line.truncate(width.saturating_sub(right.len() + 1));
            line.push(' ');
        } else {
            line.push_str(&" ".repeat(pad));
        }
        line.push_str(&right);
        let mut codes = self.ascii(&line);
        codes.truncate(width);
        codes
    }

    fn help_text(&self) -> String {
        let ctrl = self.config.control_key.label();
        let warn = match self.config.line_warning {
            0 => "off".to_string(),
            n => n.to_string(),
        };
        let mut s = format!(
            "{} - KEYS  (Esc or F1 closes)\n\
             \n\
             Ctrl+S / F2   Save\n\
             Ctrl+Q        Quit\n\
             Ctrl+Home/End Start/end of file\n\
             Ctrl+ +/-     Zoom in/out\n\
             F4            40 columns / window width\n\
             ` or F6       Inverse video on/off (Atari key)\n\
             F7            Key click on/off\n\
             F8            CONTROL key: {ctrl}\n\
             F9            Line length warning: {warn}\n\
             F10           About and licences\n\
             \n\
             Insert        Insert/overwrite mode\n\
             Shift+Insert  Insert line\n\
             Shift+Delete  Delete line\n\
             Tab           Next tab stop\n\
             Shift+Tab     Set tab stop at cursor\n\
             CONTROL+Tab   Clear tab stop at cursor\n\
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

/// Save the current frame as a BMP image (used by tests and docs).
fn save_snapshot(
    canvas: &sdl2::render::Canvas<sdl2::video::Window>,
    path: &Path,
) -> Result<(), String> {
    use sdl2::pixels::PixelFormatEnum;
    let (w, h) = canvas.output_size()?;
    let mut pixels = canvas.read_pixels(None, PixelFormatEnum::RGB24)?;
    let surface =
        sdl2::surface::Surface::from_data(&mut pixels, w, h, w * 3, PixelFormatEnum::RGB24)?;
    surface.save_bmp(path)
}
