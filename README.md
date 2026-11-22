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
  - The **`** key (left of 1) is the Atari inverse key: it turns inverse
    video on or off for typed characters.
  - **Caps Lock** clicks like the Atari's CAPS key.
- **F6** switches between the standard and the XL/XE international
  character set (accented letters in place of the graphics at $00-$1A, $60
  and $7B). With it on, accented letters typed on the PC keyboard are
  inserted as their Atari codes, and copy/paste uses real accented letters.
  - **Tab** follows the Atari's tab stops (power-up stops at screen columns
    7, 15, 23, 31 and 39 of each row). Shift+Tab sets a stop at the cursor
    and CONTROL+Tab clears one; with no stops left in the logical line, Tab
    moves to the start of the next line.
- Line length warning: characters past column 114 (the Atari screen
  editor's 3 x 38 logical line) are shown in orange, and the status bar
  shows the line's length. Change the limit or turn it off in Preferences.
- 40-column Atari layout with the 2-column margin, or wrapping at the window
  width (F4).
- The XL/XE key click, reproduced from the Atari OS click routine (F7 turns it
  off).
- Imports and exports PC text: UTF-8 for the Atari font, or plain ASCII
  with `{tags}` (see [File types](#file-types)).
- Opens tokenized programs from nine Atari languages and saves Atari BASIC
  programs tokenized (see [File types](#file-types)).

- File, Edit, Options and Help menus, with the usual shortcuts (Ctrl+N, O,
  S, Z, Y, X, C, V, A), underlined hot keys (Alt+F then Q quits), a
  right-click menu, and the system's own Open and Save dialogs.
- A Preferences page (Options > Preferences, Ctrl+, or F9): display width,
  zoom, text and background colours, character set, key click, CONTROL
  key, line length warning, attract mode, default folder (last used or a
  fixed one), and whether to save preferences on exit. Changes apply at
  once; "Save Preferences" writes them immediately.
- Colours are chosen the Atari way, by hue (0-15) and luminance (0-14), from
  an approximation of the NTSC palette; the default is the text screen's
  $94 background with $9A text.
- Attract mode: after a few minutes without input (5 by default), the
  screen colours change every 4.3 seconds exactly as the Atari OS does it
  (XOR with a changing value, then dimmed with DRKMSK = $F6). Any key press
  or mouse use restores them.
- Select text with Shift+arrows or the mouse; selected text is shown in its
  own colour so it isn't confused with inverse characters. Copy and paste work with
  other programs: ordinary characters paste as plain text, and Atari
  graphics characters use the same private Unicode characters as the
  EightBit Atari font, so they paste back exactly.
- Undo and redo.

Press **F1** (or Help > Keyboard Help) for the full list of keys.

## File types

The editor reads programs as each language saves them, shown as the
language's own LIST would show them, and plain ATASCII text. Help > File
Types lists these in the program too.

| Extension | Contents                                                  |
|-----------|-----------------------------------------------------------|
| `.BAS`    | Atari BASIC, tokenized (see below)                        |
| `.TUR`    | Turbo-BASIC XL, tokenized                                 |
| `.ATB`    | Altirra BASIC, tokenized                                  |
| `.AXB`    | Altirra Extended BASIC, tokenized                         |
| `.BXL`    | OSS BASIC XL, tokenized                                   |
| `.BXE`    | OSS BASIC XE, tokenized                                   |
| `.BAP`    | OSS BASIC A+, tokenized                                   |
| `.AMB`    | Microsoft BASIC II, tokenized (including LOCKed programs) |
| `.M65`    | MAC/65 source, tokenized                                  |
| `.LST`    | Program listings (`LIST "D:FILE"`), ATASCII text          |
| `.ASM`    | Assembler source (MAC/65, Assembler Editor), ATASCII text |
| `.LGO`    | Atari Logo (Logo's SAVE writes text)                      |
| `.PIL`    | Atari PILOT (PILOT's SAVE writes text)                    |
| `.FB`     | FastBASIC source, ATASCII text                            |
| `.ACT`    | Action! source, ATASCII text                              |
| `.TXT`    | Other ATASCII text                                        |

Any BASIC may also be saved as `.BAS`, as was usual at the time: the
dialect is recognized from the file itself.

- **Opening** reads any of these whatever the file is called. The Open
  and Save dialogs list each language as its own file type. The
  extensions in the table help tell apart dialects that share token numbers
  (Turbo-BASIC XL and BASIC XL, say); File > Reopen As reads a file as
  another format.
- **Saving**: every format can be saved, tokenized exactly as its language
  saves it.
  Save As picks the format from the extension you give, asking which BASIC
  for `.BAS`. File > Save Listing As always saves ATASCII text (a `.LST`
  listing), whatever the name. Lines with syntax errors are saved, if you
  agree, as `ERROR-` lines, as Atari BASIC itself stores them.
- **Exporting**: File > Export writes the document as PC text, with the
  PC's line endings, in one of three forms:
  - *Atari Font*: UTF-8, with printable ASCII as itself and every other Atari
    character in the Unicode Private Use Area (U+E000 + its code), where
    the EightBit Atari font has the Atari characters. Install
    [`assets/fonts/atari/EightBit Atari-Regular.ttf`](assets/fonts/atari/)
    to see them in other programs. The copy and paste uses the same
    characters.
  - *Standard*: UTF-8 with ordinary Unicode symbols, readable in any font:
    the symbols the EightBit Atari font itself gives the graphics (heavy
    box drawing ┣ ━ ┛, blocks ▄ ▌ ▖, ♥ ♣ ♦ ♠, arrows, ◀ ▶, ⤣ for CLEAR).
    Inverse video has no Unicode equivalent except a few block shapes
    (inverse space is █); other inverse characters export as normal ones,
    and Export warns first, saying how many.
  - *ASCII*: plain ASCII, with other characters as tags: `{CTRL-A}` to
    `{CTRL-Z}`, `{CTRL-,}` `{CTRL-.}` `{CTRL-;}`, `{ESC}`, `{UP}` `{DOWN}`
    `{LEFT}` `{RIGHT}`, `{CLEAR}`, `{BACK}`, `{TAB}`, `{DEL LINE}`
    `{INS LINE}` `{CLR TAB}` `{SET TAB}` `{BELL}` `{DEL CHAR}` `{INS CHAR}`,
    and `{INV}`...`{/INV}` around inverse characters.

  The document keeps its own file and format.
- **Importing**: File > Import reads a PC text file (any line endings,
  UTF-8 or not) as ATASCII: tags and Private Use characters become their
  Atari codes, a tab becomes the Atari TAB, and accented letters the
  international character set's codes. Characters with no Atari equivalent
  become `?`, and the status bar says how many. Exported files read back
  exactly. The result is a new document: Save asks for a name (suggesting
  `.LST`), so the PC file isn't overwritten.

  If the file is a numbered BASIC or assembler listing with lower case,
  Import offers to convert it to capitals, which Atari BASIC and the
  assemblers need. Strings are left as they are, and so are REM and DATA
  (and the `.` `!` `'` `--` remarks) in BASIC and `;` comments and `'c`
  constants in assembler.

  Open never converts: every byte is read as the Atari's, so a `$9B` is
  always a line end and a PC line feed is a CTRL-J character. If a file
  looks like PC text, the status bar suggests File > Import.

## Usage

```
atari_editor FILE.LST
```

If the file doesn't exist it is created when you save (Ctrl+S). You can also
start the editor without a file and use File > Open.

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
grid. The international set (`E100 256 crates/atari/src/font_international.bin
docs/images/font_international_sheet.png --lenient`) needs `--lenient`: 16
accented letters have outline points between pixel centres that don't change
any pixels, and were checked by eye.

## Licences

- The editor's source code is licensed under the GNU General Public License,
  version 3 or later (see `LICENSE`).
- The character set is derived from the font "EightBit Atari", © 2018 Steve
  Boswell, used under the SIL Open Font License 1.1 (see
  `assets/fonts/atari/OFL.txt`).
- The menus and status bar use the system's interface font. When it can't
  be found, the built-in Noto Sans (© Google, SIL Open Font License 1.1, see
  `assets/fonts/ui/OFL.txt`) is used instead.
- The libraries built into the program (SDL2 and Rust crates) are under
  permissive licences (Zlib, MIT, Apache-2.0); they're listed in
  `apps/atari_editor/THIRD_PARTY.txt`. Regenerate it with
  `tools/third_party.sh` after changing dependencies.
- All of these are shown in the editor under Help > Licensing.
