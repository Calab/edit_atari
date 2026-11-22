//! Atari 8-bit ATASCII text editor.
//!
//! Usage: atari_editor [FILE]

// Don't open a console window alongside the editor on Windows.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::PathBuf;
use std::process::ExitCode;

use atari::{Atari, keyboard::AtariKeyboard};
use retro_sdl::{AppSpec, render::Palette};

const HELP: &[&str] = &[
    "ATARI KEYS",
    "",
    "CONTROL+letter  Control graphics ($01-$1A)",
    "CONTROL+, . ;   Heart, diamond, spade",
    "CONTROL+- =     Cursor up/down",
    "Esc then key    Insert the key's code instead",
    "                of acting: Esc+arrow inserts",
    "                the arrow character",
    "Esc Esc         Insert the ESC character",
    "Esc Shift+Home  Clear-screen character",
    "Esc Backspace   Backspace character",
    "Esc Tab         Tab character",
    "Esc Delete      Delete-char character",
    "Esc Insert      Insert-char character",
    "Esc CONTROL+2   Bell character",
    "",
    "Tab stops start at screen columns 7, 15,",
    "23, 31 and 39 of each row, as at power-up.",
    "",
    "The ` key is the Atari inverse key. Keys",
    "with no Atari equivalent ({ } ~) are",
    "ignored.",
];

fn about() -> &'static str {
    let text = format!(
        "ATARI EDITOR {}\n\
         \n\
         Licensed under the GNU GPL, version 3 or later.\n\
         \n\
         Character set derived from the font\n\
         \"EightBit Atari\" by Steve Boswell,\n\
         used under the SIL Open Font License 1.1:\n\
         \n\
         {}",
        env!("CARGO_PKG_VERSION"),
        atari::FONT_LICENSE
    );
    Box::leak(text.into_boxed_str())
}

fn main() -> ExitCode {
    let file = std::env::args_os().nth(1).map(PathBuf::from);
    let spec = AppSpec {
        title: "Atari Editor",
        app_id: "atari_editor",
        machine: Box::new(Atari),
        keyboard: Box::new(AtariKeyboard::new()),
        click: atari::click::key_click,
        // Default Atari text screen: COLOR2 $94 background, COLOR1 $CA
        // luminance text, black border.
        palette: Palette {
            background: (0x1A, 0x55, 0xA8),
            foreground: (0x8C, 0xC4, 0xFF),
            border: (0x00, 0x00, 0x00),
        },
        help: HELP,
        about: about(),
    };
    match retro_sdl::run(spec, file) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("atari_editor: {e}");
            ExitCode::FAILURE
        }
    }
}
