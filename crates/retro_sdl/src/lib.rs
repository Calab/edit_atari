//! SDL2 front end shared by the retro text editors.
//!
//! A machine-specific program (e.g. `atari_editor`) fills in an [`AppSpec`]
//! and calls [`run`]. Everything here is machine-independent.

mod app;
pub mod config;
pub mod menu;
pub mod render;
pub mod ui_font;
mod view;

use std::path::{Path, PathBuf};

use retro_core::KeyboardRules;
use retro_core::Machine;
use sdl2::audio::{AudioQueue, AudioSpecDesired};
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Scancode;
use sdl2::mouse::MouseButton;

pub use config::{Config, ControlKey};
use render::Palette;

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
    /// File-dialog filter: (description, extensions without dots).
    pub file_types: (&'static str, &'static [&'static str]),
    /// Machine-specific lines appended to the help screen. A run of two or
    /// more spaces separates a key from its description.
    pub help: &'static [&'static str],
    /// Program name, version, author and copyright, for Help > About.
    pub about: String,
    /// Every licence that applies, for Help > Licensing (including the
    /// interface font's, `ui_font::FALLBACK_LICENSE`).
    pub licensing: String,
}

/// Open `file` (if given) and run the editor until the user quits.
pub fn run(spec: AppSpec, file: Option<PathBuf>) -> Result<(), String> {
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let mut app = app::App::new(spec);

    // Pick a starting size: the native screen plus a border and room for
    // the menu and status bars, scaled down if it doesn't fit the display.
    let native_cols = app.spec.machine.screen_columns() as u32 + 4;
    let native_rows = 24 + 5;
    if let Ok(bounds) = video.display_usable_bounds(0) {
        while app.config.scale > config::MIN_SCALE
            && (native_cols * 8 * app.config.scale > bounds.width()
                || native_rows * 8 * app.config.scale > bounds.height())
        {
            app.config.scale -= 1;
        }
    }
    let s = app.config.scale * 8;
    let mut window = video
        .window(app.spec.title, native_cols * s, native_rows * s)
        .position_centered()
        .resizable()
        .allow_highdpi()
        .build()
        .map_err(|e| e.to_string())?;
    let _ = window.set_minimum_size(320, 200);
    let mut canvas = window
        .into_canvas()
        .present_vsync()
        .build()
        .map_err(|e| e.to_string())?;
    let creator = canvas.texture_creator();
    let mut atlas = render::build_atlas(&creator, app.spec.machine.as_ref())?;

    // Size the interface for the display's pixel density.
    let (pw, _) = canvas.output_size()?;
    let (ww, _) = canvas.window().size();
    app.ratio = if ww > 0 { pw as f32 / ww as f32 } else { 1.0 };
    let mut font = ui_font::UiFont::new(&creator, 15.0 * app.ratio);
    app.set_ui_metrics(font.line_height());

    if let Some(p) = file {
        app.open_path(&p);
    }

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
    let clipboard = video.clipboard();
    let mut events = sdl.event_pump()?;

    // Test hooks for screenshots: open a menu, select some lines.
    if let Some(m) = std::env::var("RETRO_EDITOR_MENU")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        app.open_menu(m);
    }
    // RETRO_EDITOR_HELP = help | licensing | about
    app.overlay = match std::env::var("RETRO_EDITOR_HELP").as_deref() {
        Ok("licensing") => app::Overlay::Licensing,
        Ok("about") => app::Overlay::About,
        Ok(_) => app::Overlay::Help,
        Err(_) => app::Overlay::None,
    };
    if let Some((x, y)) = std::env::var("RETRO_EDITOR_CONTEXT")
        .ok()
        .and_then(|v| v.split_once(',').map(|(a, b)| (a.parse(), b.parse())))
        .and_then(|(a, b)| Some((a.ok()?, b.ok()?)))
    {
        app.menu.context = Some((x, y));
    }
    if let Some(n) = std::env::var("RETRO_EDITOR_SELECT")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        app.test_select_lines(n);
    }

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
            let frame = app.frame(canvas.output_size()?);
            let ctx = app::Ctx {
                window: canvas.window(),
                clipboard: &clipboard,
                frame: &frame,
            };
            let ratio = app.ratio;
            let px = |v: i32| (v as f32 * ratio) as i32;
            match event {
                Event::Quit { .. } => app.command(menu::Command::Quit, &ctx),
                Event::Window {
                    win_event: WindowEvent::SizeChanged(..),
                    ..
                } => app.editor.scroll_to_cursor(&frame.layout),
                Event::KeyDown {
                    keycode: Some(k),
                    scancode,
                    keymod,
                    ..
                } => {
                    // The key left of 1 (`/~ on US keyboards) is the Atari
                    // inverse key, wherever the layout puts its symbols.
                    let inverse_key = scancode == Some(Scancode::Grave);
                    let clicked = app.key_down(k, keymod, inverse_key, &ctx);
                    if clicked && app.config.key_click {
                        play_click(queue.as_ref(), click.as_deref());
                    }
                }
                Event::TextInput { text, .. } => {
                    let typed = app.text_input(&text, keyboard_util.mod_state(), &frame.layout);
                    if typed && app.config.key_click {
                        play_click(queue.as_ref(), click.as_deref());
                    }
                }
                Event::MouseButtonDown {
                    mouse_btn: MouseButton::Left,
                    x,
                    y,
                    ..
                } => {
                    let shift = keyboard_util.mod_state().intersects(
                        sdl2::keyboard::Mod::LSHIFTMOD | sdl2::keyboard::Mod::RSHIFTMOD,
                    );
                    app.mouse_down(px(x), px(y), shift, &frame);
                }
                Event::MouseButtonDown {
                    mouse_btn: MouseButton::Right,
                    x,
                    y,
                    ..
                } => app.mouse_right(px(x), px(y), &frame),
                Event::MouseMotion {
                    x, y, mousestate, ..
                } => app.mouse_move(px(x), px(y), mousestate.left(), &frame),
                Event::MouseButtonUp {
                    mouse_btn: MouseButton::Left,
                    x,
                    y,
                    ..
                } => app.mouse_up(px(x), px(y), &ctx),
                Event::MouseWheel { y, .. } => app.wheel(y, &frame),
                _ => {}
            }
        }

        if std::mem::take(&mut app.charset_changed) {
            atlas = render::build_atlas(&creator, app.spec.machine.as_ref())?;
        }
        let frame = app.frame(canvas.output_size()?);
        app.draw(&mut canvas, &mut atlas, &mut font, &frame, &clipboard);
        if let Some(path) = std::env::var_os("RETRO_EDITOR_SNAPSHOT") {
            // Test hook: save the first frame as a BMP and exit.
            save_snapshot(&canvas, Path::new(&path))?;
            break;
        }
        canvas.present();
        let title = app.window_title();
        if canvas.window().title() != title {
            let _ = canvas.window_mut().set_title(&title);
        }
    }

    if let Err(e) = app.config.save(app.spec.app_id) {
        eprintln!("could not save settings: {e}");
    }
    Ok(())
}

/// Queue the key-click sound.
fn play_click(queue: Option<&AudioQueue<f32>>, samples: Option<&[f32]>) {
    if let (Some(q), Some(s)) = (queue, samples) {
        // Drop any backlog so clicks never lag behind typing.
        if q.size() > 4096 {
            q.clear();
        }
        let _ = q.queue_audio(s);
    }
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
