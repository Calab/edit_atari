# Atari Editor

A text editor for Atari 8-bit computer files, for Linux, Windows and macOS.
It edits ATASCII text (Atari BASIC `LIST` files, Action!, Assembler Editor and
MAC/65 source saved as text) using the real Atari character set, with `$9B`
line endings.

Each release is a single program file with nothing else to install: the Atari
character set is built in.

![The Atari character set](docs/images/font_atascii_sheet.png)

## Features

- Shows all 256 ATASCII characters, including graphics and inverse video.
- Atari keyboard behaviour on a PC keyboard:
  - **RIGHT-CTRL** acts as the Atari CONTROL key (switchable to LEFT-ALT with
    F8), so left Ctrl stays free for editor commands.
  - **Esc** before a key inserts that key's character instead of performing
    its action, as on the Atari: Esc then cursor-up inserts the up-arrow
    character.
  - The **`** key (left of 1; also **F6**) is the Atari inverse key: it
    turns inverse video on or off for typed characters.
  - **Tab** follows the Atari's tab stops (power-up stops at screen columns
    7, 15, 23, 31 and 39 of each row). Shift+Tab sets a stop at the cursor
    and CONTROL+Tab clears one; with no stops left in the logical line, Tab
    moves to the start of the next line.
- Line length warning: characters past column 114 (the Atari screen
  editor's 3 x 38 logical line) are shown in orange, and the status bar
  shows the line's length. Press F9 to change the limit (0 turns it off).
- 40-column Atari layout with the 2-column margin, or wrapping at the window
  width (F4).
- The XL/XE key click, reproduced from the Atari OS click routine (F7 turns it
  off).
- Opens PC text files (LF or CR/LF) and saves them as ATASCII.

Press **F1** in the editor for the full list of keys.

## Usage

```
atari_editor FILE.LST
```

If the file doesn't exist it is created when you save (Ctrl+S).

## Building

Install Rust from <https://rustup.rs> and CMake, then:

```
cargo build --release -p atari_editor
```

The program is written to `target/release/atari_editor`. SDL2 is compiled
from source and linked in, so the result doesn't need SDL installed.

On Linux, building SDL needs the X11/Wayland/audio development packages; see
`.github/workflows/build.yml` for the list. GitHub Actions builds Linux,
Windows and macOS versions on every push.

### Developing in VS Code

Open the folder in VS Code and install the recommended extensions when
prompted (rust-analyzer, CodeLLDB, Even Better TOML). Then press **F5** to
build and run the editor on `samples/SAMPLE.LST`, or pick "Atari Editor:
choose a file" in the Run and Debug view.

## Project layout

| Path | Contents |
|------|----------|
| `crates/retro_core` | Machine-independent editor: document, layout, keyboard interface |
| `crates/retro_sdl` | SDL2 window, drawing, sound and settings, shared by all machines |
| `crates/atari` | Atari specifics: ATASCII, character set, keyboard rules, key click |
| `apps/atari_editor` | The Atari editor program |
| `tools/fontgen` | Converts the pixel TrueType font into the built-in 8x8 character set |
| `docs/` | Design notes and tokenized-language references |

The machine-specific parts sit behind the `Machine` and `KeyboardRules`
traits, so an editor for another 8-bit computer (e.g. a Commodore 64 PETSCII
editor) can reuse `retro_core` and `retro_sdl`.

### Regenerating the character set

```
cargo run -p fontgen -- build "assets/fonts/atari/EightBit Atari-Regular.ttf" \
    E000 256 crates/atari/src/font_atascii.bin docs/images/font_atascii_sheet.png
```

The tool refuses to write output if any glyph outline is off the 8x8 pixel
grid.

## Licences

- The editor's source code is licensed under the GNU General Public License,
  version 3 or later (see `LICENSE`).
- The character set is derived from the font "EightBit Atari", © 2018 Steve
  Boswell, used under the SIL Open Font License 1.1 (see
  `assets/fonts/atari/OFL.txt`). The licence text is also shown in the
  editor's About screen (F10).
