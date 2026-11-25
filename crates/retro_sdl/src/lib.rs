//! SDL2 front end shared by the retro text editors.
//!
//! A machine-specific program (e.g. `atari_editor`) fills in an [`AppSpec`]
//! and calls [`run`]. Everything here is machine-independent.

mod app;
pub mod config;
mod dialog;
mod find_page;
mod fonts_page;
mod image_page;
pub mod menu;
mod prefs;
mod print_page;
pub mod render;
mod source;
pub mod ui_font;
mod view;
mod widgets;

use std::path::{Path, PathBuf};

use retro_core::KeyboardRules;
use retro_core::Machine;
use sdl2::audio::{AudioQueue, AudioSpecDesired};
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Scancode;
use sdl2::mouse::MouseButton;

pub use config::{Config, ControlKey};
use render::{ColorModel, Palette};

/// The window icon: the icon character filling 64x64 pixels, as the
/// icon files in `assets/icons` are drawn.
fn window_icon(spec: &AppSpec) -> Option<sdl2::surface::Surface<'static>> {
    const SIZE: u32 = 64;
    let glyph = spec.machine.glyph(spec.icon);
    let (bg, fg) = (spec.palette.background, spec.palette.foreground);
    let mut surface =
        sdl2::surface::Surface::new(SIZE, SIZE, sdl2::pixels::PixelFormatEnum::RGBA32).ok()?;
    let pitch = surface.pitch() as usize;
    surface.with_lock_mut(|px| {
        for y in 0..SIZE as usize {
            for x in 0..SIZE as usize {
                let on = glyph[y / 8] & (0x80 >> (x / 8)) != 0;
                let (r, g, b) = if on { fg } else { bg };
                px[y * pitch + x * 4..][..4].copy_from_slice(&[r, g, b, 255]);
            }
        }
    });
    Some(surface)
}

/// Everything that makes an editor specific to one machine.
pub struct AppSpec {
    /// Program name and window title, e.g. "Edit Atari".
    pub title: &'static str,
    /// Folder name for settings, e.g. "atari_editor".
    pub app_id: &'static str,
    pub machine: Box<dyn Machine>,
    /// The disk images the machine reads, for File > Open from Image and
    /// Save to Image. Without them, those menu items aren't shown.
    pub disks: Option<Box<dyn retro_core::disk::DiskImages>>,
    pub keyboard: Box<dyn KeyboardRules>,
    /// Generates the key-click sound: (sample rate, volume) -> mono samples.
    pub click: fn(u32, f32) -> Vec<f32>,
    pub palette: Palette,
    /// The character code drawn as the window icon, in the palette's
    /// foreground and background colours.
    pub icon: u8,
    /// Selectable text/background colours and attract mode, if the machine
    /// has a colour model. Without one, `palette` is used as is.
    pub colors: Option<ColorModel>,
    /// File-dialog filters, one per language: (name, extensions without
    /// dots, in upper case). The dialogs also offer all of them together
    /// as "Atari files", and "All files".
    pub file_types: &'static [(&'static str, &'static [&'static str])],
    /// Machine-specific lines appended to the help screen. A run of two or
    /// more spaces separates a key from its description.
    pub help: &'static [&'static str],
    /// The file types the editor reads and writes, for Help > File Types.
    /// Laid out like `help`: two or more spaces separate the extension
    /// from its description.
    pub file_types_help: &'static [&'static str],
    /// Program name, version, author and copyright, for Help > About.
    pub about: String,
    /// Every licence that applies, for Help > Licensing (including the
    /// interface font's, `ui_font::FALLBACK_LICENSE`).
    pub licensing: String,
}

/// Open `file` (if given) and run the editor until the user quits.
pub fn run(spec: AppSpec, file: Option<PathBuf>) -> Result<(), String> {
    // Tell the desktop what the program is called (otherwise GNOME shows
    // "Unknown" in its window list and messages).
    sdl2::hint::set("SDL_APP_NAME", spec.title);
    sdl2::hint::set("SDL_VIDEO_X11_WMCLASS", spec.app_id);
    sdl2::hint::set("SDL_VIDEO_WAYLAND_WMCLASS", spec.app_id);
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let mut app = app::App::new(spec);

    // The window opens small; its real size is set below, once the
    // display's pixel density is known.
    let native_cols = app.spec.machine.screen_columns() as u32 + 4;
    let native_rows = 24 + 5;
    let mut window = video
        .window(app.spec.title, native_cols * 8, native_rows * 8)
        .position_centered()
        .resizable()
        .allow_highdpi()
        .build()
        .map_err(|e| e.to_string())?;
    if let Some(icon) = window_icon(&app.spec) {
        window.set_icon(icon);
    }
    // Fonts chosen in the settings, now that the icon has the built-in A.
    app.load_fonts();
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
    let mut font = ui_font::UiFont::new(&creator, 15.0 * app.ratio, ui_font::Style::Proportional);
    let mut mono = ui_font::UiFont::new(&creator, 14.0 * app.ratio, ui_font::Style::Monospace);
    // The Preferences page packs a lot in, so it uses a smaller size.
    let mut small = ui_font::UiFont::new(&creator, 12.0 * app.ratio, ui_font::Style::Proportional);
    app.set_ui_metrics(font.line_height());
    app.gutter_advance = mono.digit_advance();
    // The line numbers, in the monospace font sized to the characters
    // (made again when the zoom changes).
    let mut gutter = mono.resized(app::App::gutter_font_px(app.config.scale * 8));
    // Room for the line numbers beside the 40-column screen at `cell`.
    let advance = app.gutter_advance;
    let numbers = |on: bool, cell: u32| {
        if on {
            app::App::gutter_width(advance, cell)
        } else {
            0
        }
    };

    // Size the window. Zoom is in real pixels but window sizes are in
    // desktop units, which differ on high-DPI displays.
    //
    // Start with the machine's screen plus a border and room for the menu
    // and status bars, lowering the zoom if that doesn't fit the display,
    // and make it big enough for the Preferences page too. It can't be made
    // smaller than the screen at 2x zoom.
    {
        let to_window = |n: u32| (n as f32 / app.ratio).ceil() as u32;
        // Line numbers make the 40-column screen that much wider.
        let on = app.config.line_numbers;
        let bounds = video.display_usable_bounds(0).ok();
        let fit = |w: u32, h: u32| match bounds {
            Some(b) => (w.min(b.width()), h.min(b.height())),
            None => (w, h),
        };
        let screen = |scale: u32| {
            (
                to_window(native_cols * 8 * scale + numbers(on, 8 * scale)),
                to_window(native_rows * 8 * scale),
            )
        };
        if let Some(b) = bounds {
            while app.config.scale > config::MIN_SCALE && {
                let (w, h) = screen(app.config.scale);
                w > b.width() || h > b.height()
            } {
                app.config.scale -= 1;
            }
        }

        let bars = app.bar_height as u32 * 2;
        let (min_w, min_h) = fit(
            to_window(native_cols * 16 + numbers(on, 16)),
            to_window(24 * 16 + bars),
        );
        let _ = canvas.window_mut().set_minimum_size(min_w, min_h);

        let (pw, ph) = prefs::page_size(&mut small, app.prefs_pad, &app.prefs_view());
        let page = (
            to_window((pw + app.pad * 8) as u32),
            to_window((ph + app.pad) as u32 + bars),
        );
        // The size it had when last closed, if any (it may have been on a
        // bigger display); otherwise the screen at the zoom, or the
        // Preferences page if that's bigger.
        let (w, h) = match app.config.window_size {
            Some((w, h)) => fit(w.max(min_w), h.max(min_h)),
            None => {
                let (sw, sh) = screen(app.config.scale);
                fit(sw.max(page.0).max(min_w), sh.max(page.1).max(min_h))
            }
        };
        let window = canvas.window_mut();
        let _ = window.set_size(w, h);
        window.set_position(
            sdl2::video::WindowPos::Centered,
            sdl2::video::WindowPos::Centered,
        );
        if app.config.window_maximized {
            window.maximize();
        }
    }

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
    // RETRO_EDITOR_HELP = help | filetypes | licensing | about | prefs | print | reopen | export
    app.overlay = match std::env::var("RETRO_EDITOR_HELP").as_deref() {
        Ok("licensing") => app::Overlay::Licensing,
        Ok("filetypes") => app::Overlay::FileTypes,
        Ok("prefs") => {
            app.open_prefs(prefs::Field::Wrap);
            app::Overlay::None
        }
        Ok("reopen") => {
            app.test_reopen_as();
            app::Overlay::None
        }
        Ok("print") => {
            app.test_open_print();
            app::Overlay::None
        }
        Ok("fonts") => {
            app.test_open_fonts();
            app::Overlay::None
        }
        Ok("export") => {
            app.test_export_dialog();
            app::Overlay::None
        }
        Ok("about") => {
            app.show_about();
            app::Overlay::None
        }
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
    // Keys, as if typed: ^X is Ctrl+X, \n is Enter, { } Ctrl+Left/Right,
    // [ ] Ctrl+Shift+Left/Right, anything else is typed (key down, then
    // its text).
    if let Ok(keys) = std::env::var("RETRO_EDITOR_KEYS") {
        use sdl2::keyboard::{Keycode, Mod};
        let mut chars = keys.chars();
        while let Some(c) = chars.next() {
            let frame = app.frame(canvas.output_size()?);
            let ctx = app::Ctx {
                clipboard: &clipboard,
                frame: &frame,
            };
            match c {
                '^' => {
                    let k = chars.next().unwrap_or('A').to_ascii_lowercase();
                    let code = Keycode::from_i32(k as i32).unwrap_or(Keycode::A);
                    app.key_down(code, Mod::LCTRLMOD, false, &ctx);
                }
                '\n' => {
                    app.key_down(Keycode::RETURN, Mod::NOMOD, false, &ctx);
                }
                // Ctrl+Left/Right, with Shift for [ and ].
                '{' | '}' | '[' | ']' => {
                    let k = if matches!(c, '{' | '[') {
                        Keycode::LEFT
                    } else {
                        Keycode::RIGHT
                    };
                    let shift = if matches!(c, '[' | ']') {
                        Mod::LSHIFTMOD
                    } else {
                        Mod::NOMOD
                    };
                    app.key_down(k, Mod::LCTRLMOD | shift, false, &ctx);
                }
                c => {
                    let code = Keycode::from_i32(c.to_ascii_lowercase() as i32);
                    if let Some(code) = code {
                        app.key_down(code, Mod::NOMOD, false, &ctx);
                    }
                    app.text_input(&c.to_string(), Mod::NOMOD, &frame.layout);
                }
            }
        }
    }
    // Fonts to use: "standard.fnt" or "standard.fnt,international.fnt".
    if let Ok(list) = std::env::var("RETRO_EDITOR_FONTS") {
        for (i, path) in list.split(',').enumerate().filter(|(_, p)| !p.is_empty()) {
            app.test_set_font(i, std::path::Path::new(path));
        }
    }
    // Go to this line number.
    if let Ok(n) = std::env::var("RETRO_EDITOR_GOTO") {
        let frame = app.frame(canvas.output_size()?);
        app.test_go_to(&n, &frame.layout);
    }
    // Open the Find panel and find this text.
    if let Ok(text) = std::env::var("RETRO_EDITOR_FIND") {
        let frame = app.frame(canvas.output_size()?);
        app.test_open_find(&text, &frame.layout);
    }
    // Files to drop, separated as in PATH (':' or ';').
    if let Some(paths) = std::env::var_os("RETRO_EDITOR_DROP") {
        app.drop_files(std::env::split_paths(&paths).collect());
    }
    if let Some(path) = std::env::var_os("RETRO_EDITOR_SAVE_PDF") {
        app.test_save_pdf(path.into());
    }
    if let Some(path) = std::env::var_os("RETRO_EDITOR_IMPORT") {
        app.test_import(path.into());
    }
    if let Some(path) = std::env::var_os("RETRO_EDITOR_EXPORT_UNICODE") {
        app.test_export(path.into(), app::ExportKind::MachineFont);
    }
    if let Some(path) = std::env::var_os("RETRO_EDITOR_EXPORT_STANDARD") {
        app.test_export(path.into(), app::ExportKind::Standard);
    }
    if let Some(path) = std::env::var_os("RETRO_EDITOR_EXPORT_ASCII") {
        app.test_export(path.into(), app::ExportKind::Ascii);
    }
    // Choose a button in the Export dialog (e.g. 1 to see the warning).
    if let Some(n) = std::env::var("RETRO_EDITOR_EXPORT_CHOICE")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        app.test_export_choice(n);
    }
    if let Some(path) = std::env::var_os("RETRO_EDITOR_SAVE_LISTING_AS") {
        app.test_save_listing_as(path.into());
    }
    if let Some(path) = std::env::var_os("RETRO_EDITOR_SAVE_AS") {
        app.test_save_as(path.into());
    }
    // Open a document by its recent-files text (e.g.
    // "image:GAMES/PAC.BAS:/path/DISK.ATR" for a file in a disk image).
    if let Ok(text) = std::env::var("RETRO_EDITOR_OPEN_SOURCE") {
        app.test_open_source(&text);
    }
    // File > Open from Image ("1", or an image to show), then keys on its
    // page ("down,return").
    if let Some(v) = std::env::var_os("RETRO_EDITOR_IMAGE_PAGE") {
        app.test_open_image_page((v != "1").then(|| v.into()));
    }
    if let Ok(keys) = std::env::var("RETRO_EDITOR_IMAGE_KEYS") {
        app.test_image_keys(&keys);
    }
    // Type this text, then File > Save (as Ctrl+S does).
    if let Ok(text) = std::env::var("RETRO_EDITOR_TYPE_AND_SAVE") {
        let frame = app.frame(canvas.output_size()?);
        app.text_input(&text, sdl2::keyboard::Mod::NOMOD, &frame.layout);
        app.test_save();
    }
    // Press dialog buttons in turn, e.g. "0,0".
    if let Ok(list) = std::env::var("RETRO_EDITOR_CHOOSE") {
        for n in list.split(',').filter_map(|n| n.trim().parse().ok()) {
            app.test_choose(n);
        }
    }
    if std::env::var_os("RETRO_EDITOR_ATTRACT").is_some() {
        app.test_attract();
    }
    if let Some(n) = std::env::var("RETRO_EDITOR_SELECT")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        app.test_select_lines(n);
    }
    if let Some(n) = std::env::var("RETRO_EDITOR_SELECT_BACK")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        app.test_select_back(n);
    }

    // Files dropped on the window, collected until the drop is complete
    // (several files dropped at once arrive one event at a time).
    let mut dropped: Vec<PathBuf> = Vec::new();
    let mut dropping = false;
    let mut first_frame = true;
    let mut min_numbers = numbers(app.config.line_numbers, 16);
    while app.running {
        // Draw the first frame straight away; after that, sleep until
        // something happens (the Atari cursor doesn't blink).
        // While a file dialog is open, wake up regularly to check for its
        // answer.
        // Sleep until something happens, waking for a pending file dialog's
        // answer or for attract mode.
        let wake = [
            app.file_job_pending()
                .then_some(std::time::Duration::from_millis(50)),
            app.next_wakeup(),
        ]
        .into_iter()
        .flatten()
        .min();
        let mut pending = if first_frame {
            Vec::new()
        } else if let Some(d) = wake {
            let ms = d.as_millis().clamp(1, u128::from(u32::MAX)) as u32;
            events.wait_event_timeout(ms).into_iter().collect()
        } else {
            vec![events.wait_event()]
        };
        app.poll_file_job();
        first_frame = false;
        pending.extend(events.poll_iter());

        for event in pending {
            if matches!(
                event,
                Event::KeyDown { .. }
                    | Event::TextInput { .. }
                    | Event::MouseButtonDown { .. }
                    | Event::MouseMotion { .. }
                    | Event::MouseWheel { .. }
            ) {
                app.activity();
            }
            if matches!(
                event,
                Event::KeyDown { .. } | Event::TextInput { .. } | Event::MouseButtonDown { .. }
            ) {
                app.restart_flash();
            }
            let frame = app.frame(canvas.output_size()?);
            let ctx = app::Ctx {
                clipboard: &clipboard,
                frame: &frame,
            };
            let ratio = app.ratio;
            let px = |v: i32| (v as f32 * ratio) as i32;
            match event {
                Event::Quit { .. } => app.command(menu::Command::Quit, &ctx),
                // Files dragged onto the window, or (on macOS) opened from
                // the Finder with this app.
                Event::DropBegin { .. } => dropping = true,
                Event::DropFile { filename, .. } => dropped.push(PathBuf::from(filename)),
                Event::DropComplete { .. } => dropping = false,
                Event::Window {
                    win_event: WindowEvent::SizeChanged(..),
                    ..
                } => {
                    app.editor.scroll_to_cursor(&frame.layout);
                    note_window_size(&mut app.config, canvas.window());
                }
                // Back in the window: has another program changed the file?
                Event::Window {
                    win_event: WindowEvent::FocusGained,
                    ..
                } => app.check_disk(),
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
                    clicks,
                    ..
                } => {
                    use sdl2::keyboard::Mod;
                    let mods = keyboard_util.mod_state();
                    let shift = mods.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD);
                    // Control-click is the Mac's right-click.
                    if cfg!(target_os = "macos") && mods.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) {
                        app.mouse_right(px(x), px(y), &frame);
                    } else {
                        app.mouse_down(px(x), px(y), shift, clicks, &frame);
                    }
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
                Event::MouseWheel {
                    y,
                    mouse_x,
                    mouse_y,
                    ..
                } => app.wheel(y, (px(mouse_x), px(mouse_y)), &frame),
                _ => {}
            }
        }
        // A whole drop has arrived (or a file came on its own, as the
        // Finder may send them).
        if !dropping && !dropped.is_empty() {
            app.drop_files(std::mem::take(&mut dropped));
        }

        app.tick();
        if std::mem::take(&mut app.charset_changed) {
            atlas = render::build_atlas(&creator, app.spec.machine.as_ref())?;
        }
        if app::App::gutter_font_px(app.config.scale * 8) != gutter.px() {
            gutter = mono.resized(app::App::gutter_font_px(app.config.scale * 8));
        }
        // The smallest window keeps room for the line numbers beside the
        // 40-column screen at 2x.
        let gutter_min = numbers(app.config.line_numbers, 16);
        if gutter_min != min_numbers {
            min_numbers = gutter_min;
            let to_window = |n: u32| (n as f32 / app.ratio).ceil() as u32;
            let (_, min_h) = canvas.window().minimum_size();
            let _ = canvas
                .window_mut()
                .set_minimum_size(to_window(native_cols * 16 + gutter_min), min_h);
        }
        let frame = app.frame(canvas.output_size()?);
        app.draw(
            &mut canvas,
            &mut atlas,
            (&mut font, &mut mono, &mut small, &mut gutter),
            &frame,
            &clipboard,
        );
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

    note_window_size(&mut app.config, canvas.window());
    if let Err(e) = app.save_prefs_on_exit() {
        eprintln!("could not save settings: {e}");
    }
    Ok(())
}

/// Remember the window's size for next time: whether it's maximized, and
/// its size when it isn't (a maximized or full-screen size isn't kept, so
/// un-maximizing next time goes back to the size the user chose).
fn note_window_size(config: &mut Config, window: &sdl2::video::Window) {
    use sdl2::sys::SDL_WindowFlags as F;
    let flags = window.window_flags();
    let has = |f: F| flags & f as u32 != 0;
    if has(F::SDL_WINDOW_MINIMIZED) {
        return;
    }
    config.window_maximized = has(F::SDL_WINDOW_MAXIMIZED);
    let full = has(F::SDL_WINDOW_FULLSCREEN) || has(F::SDL_WINDOW_FULLSCREEN_DESKTOP);
    if !config.window_maximized && !full {
        config.window_size = Some(window.size());
    }
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
