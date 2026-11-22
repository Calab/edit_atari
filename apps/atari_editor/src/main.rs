//! Atari 8-bit ATASCII text editor.
//!
//! Usage: atari_editor [FILE]

// Don't open a console window alongside the editor on Windows.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::PathBuf;
use std::process::ExitCode;

use atari::palette;
use atari::{Atari, keyboard::AtariKeyboard};
use retro_sdl::{
    AppSpec,
    render::{ColorModel, Palette},
};

/// Help > File Types: the extensions for each language.
const FILE_TYPES: &[&str] = &[
    "FILE TYPES",
    "",
    "Programs as each language SAVEs them (tokenized):",
    "",
    ".BAS  Atari BASIC. Any BASIC below may also be",
    "      saved as .BAS: the dialect is recognized",
    "      from the file itself.",
    ".TUR  Turbo-BASIC XL",
    ".ATB  Altirra BASIC",
    ".AXB  Altirra Extended BASIC",
    ".BXL  OSS BASIC XL",
    ".BXE  OSS BASIC XE",
    ".BAP  OSS BASIC A+",
    ".AMB  Microsoft BASIC II (also LOCKed programs)",
    ".M65  MAC/65 source",
    "",
    "Plain ATASCII text:",
    "",
    ".LST  Program listings (LIST \"D:FILE\")",
    ".ASM  Assembler source (MAC/65, Assembler Editor)",
    ".LGO  Atari Logo (its SAVE writes text)",
    ".PIL  Atari PILOT (its SAVE writes text)",
    ".ACT  Action! source",
    ".TXT  Other text",
    "",
    "Opening reads any of these, whatever its name.",
    "File > Reopen As reads a file as another format.",
    "",
    "Saving: every BASIC above except Microsoft BASIC",
    "can be saved tokenized; Microsoft BASIC and",
    "MAC/65 are saved as .LST text for now.",
    "Save As picks the format from the extension you",
    "give, asking which BASIC for .BAS.",
    "File > Save Listing As always saves ATASCII text",
    "(a .LST listing), whatever the name.",
];

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
    "The ` key (left of 1) is the Atari inverse key.",
    "Keys with no Atari equivalent ({ } ~) are ignored.",
    "",
    "F6 switches to the XL/XE international character",
    "set: accented letters replace the graphics at",
    "$00-$1A, $60 and $7B. With it on, accented letters",
    "typed on the PC keyboard (e.g. é, ñ, ü) are",
    "inserted as their Atari codes.",
];

/// Copyright notice details, for About and Licensing.
const COPYRIGHT_YEAR: &str = "2026";
const COPYRIGHT_HOLDER: &str = "Steve Norton";

/// The authors from Cargo.toml, as "A and B" (or "A, B and C").
fn authors() -> String {
    let names: Vec<&str> = env!("CARGO_PKG_AUTHORS").split(':').collect();
    match names.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => names.concat(),
    }
}

/// Program details for Help > About.
fn about() -> String {
    format!(
        "Atari Editor\n\
         Version {}\n\
         \n\
         A text editor for Atari 8-bit computer files.\n\
         \n\
         Author: {}\n\
         Copyright \u{a9} {COPYRIGHT_YEAR} {COPYRIGHT_HOLDER}\n\
         \n\
         Licensed under the GNU GPL, version 3 or later.\n\
         See Help > Licensing for details.",
        env!("CARGO_PKG_VERSION"),
        authors(),
    )
}

/// Everything in the program and its licences, for Help > Licensing.
fn licensing() -> String {
    format!(
        "ATARI EDITOR\n\
         Copyright \u{a9} {COPYRIGHT_YEAR} {COPYRIGHT_HOLDER}\n\
         \n\
         This program is free software: you can redistribute it and/or modify \
         it under the terms of the GNU General Public License as published by \
         the Free Software Foundation, either version 3 of the License, or (at \
         your option) any later version. It is distributed in the hope that it \
         will be useful, but WITHOUT ANY WARRANTY; without even the implied \
         warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. The \
         full licence text is at the end of this page.\n\
         \n\
         \n\
         ATARI CHARACTER SET\n\
         \n\
         The Atari characters are converted from the font \"EightBit Atari\" \
         by Steve Boswell, used under the SIL Open Font License 1.1:\n\
         \n\
         {}\n\
         \n\
         INTERFACE FONT\n\
         \n\
         The menus and this page use the system's own fonts. When they can't \
         be found, the built-in Noto Sans and Noto Sans Mono are used, under \
         the SIL Open Font License 1.1:\n\
         \n\
         {}\n\
         \n\
         LIBRARIES BUILT INTO THIS PROGRAM\n\
         \n\
         {}\n\
         \n\
         GNU GENERAL PUBLIC LICENSE\n\
         \n\
         {}",
        atari::FONT_LICENSE,
        retro_sdl::ui_font::FALLBACK_LICENSE,
        include_str!("../THIRD_PARTY.txt"),
        include_str!("../../../LICENSE"),
    )
}

fn main() -> ExitCode {
    let file = std::env::args_os().nth(1).map(PathBuf::from);
    let spec = AppSpec {
        title: "Atari Editor",
        app_id: "atari_editor",
        machine: Box::new(Atari::default()),
        keyboard: Box::new(AtariKeyboard::new()),
        click: atari::click::key_click,
        // Default Atari text screen: COLOR2 $94 background, COLOR1 $CA
        // luminance text, black border.
        palette: Palette {
            background: (0x1A, 0x55, 0xA8),
            foreground: (0x8C, 0xC4, 0xFF),
            border: (0x00, 0x00, 0x00),
            warning: (0xFF, 0xA8, 0x40),
            selection: (0xF2, 0xC8, 0x4B),
            selection_text: (0x10, 0x1C, 0x40),
        },
        file_types: (
            "Atari files",
            &[
                "bas", "BAS", "tur", "TUR", "atb", "ATB", "axb", "AXB", "bxl", "BXL", "bxe", "BXE",
                "bap", "BAP", "amb", "AMB", "m65", "M65", "lst", "LST", "asm", "ASM", "lgo", "LGO",
                "pil", "PIL", "act", "ACT", "txt", "TXT",
            ],
        ),
        colors: Some(ColorModel {
            hues: palette::HUES,
            lums: palette::LUMS,
            lum_step: 2,
            rgb: palette::rgb,
            label: palette::label,
            attract: palette::attract,
            default_text: palette::DEFAULT_TEXT,
            default_background: palette::DEFAULT_BACKGROUND,
            default_border: palette::BORDER,
        }),
        help: HELP,
        file_types_help: FILE_TYPES,
        about: about(),
        licensing: licensing(),
    };
    match retro_sdl::run(spec, file) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("atari_editor: {e}");
            ExitCode::FAILURE
        }
    }
}
