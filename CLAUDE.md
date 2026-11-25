# edit_atari: notes for Claude

Most project knowledge (Steve's preferences, decisions, release steps)
is in Claude's memory for this project. This file covers the disk-image
crate behind File > Open from Image and Save to Image.

## The atari-disk crate
`atari-disk` (`use atari_disk::...`) is Steve's own crate for Atari disk
images, in its own repository: `~/Coding/atari-disk`,
github.com/Calab/atari-disk (MIT OR Apache-2.0, no dependencies). It's
finished and checked in AltirraSDL. Its source documents every public
call with examples: read `~/Coding/atari-disk/src/volume.rs` first, or
run `cargo doc --open` there. Its `CLAUDE.md` and `tmp/API.md` hold the
decisions behind it. Change the crate there, not here.

It's a dependency of `apps/atari_editor` from GitHub (a path dependency
would break this repository's GitHub builds). `Cargo.lock` pins one
commit: after Steve pushes a change to atari-disk, run
`cargo update -p atari-disk` here to pick it up.

**Where the code is:** `retro_core::disk` is the machine-neutral
interface (`DiskImages`, `DiskVolume`). The Atari version, the only
code that uses atari-disk, is `apps/atari_editor/src/disk.rs`. The
pages are in `crates/retro_sdl`: `image_page.rs` (Open from Image and
Save to Image, with the Create Directory pop-up), `new_image.rs` (the
New Image pop-up), `source.rs` (a document's file, or file in an
image) and `local_time.rs` (dates for SpartaDOS, from the OS, no
crate).

### What it does
- Images: `.ATR` and `.XFD`, 128/256/512-byte sectors, ATR write-protect
  flag, raw sector reads and writes.
- File systems: Atari DOS 2 (2.0S, 2.0D, 2.5 enhanced), MyDOS 4.x
  (subdirectories, up to 65535 sectors), SpartaDOS 2.x/3.x and
  SpartaDOS X (subdirectories, dates, hidden/archive flags, volume
  names). Anything else opens as `FileSystem::Raw` (sectors only).
- List, read (all or a byte range), write, rename, move between
  directories, delete (empty directories only), lock/unlock, make
  directory, set dates and flags, check a name, will-it-fit, format
  blank disks, validate a disk, map every sector, make a disk bootable
  from a DOS disk the user supplies.
- Not done: copying files (read, then write), undelete, DCM/ATX/PRO.

### How to use it
```rust
use atari_disk::{Container, DiskImage, Layout, Timestamp, Volume};

let mut vol = Volume::open(DiskImage::from_bytes(&std::fs::read(path)?)?)?;
for e in vol.list("GAMES")? {          // "" is the top directory
    println!("{} {} bytes", e.name_text(), e.size);
}
let data = vol.read("GAMES>PACMAN.LST")?;
vol.set_overwrite(true);               // or ask the user first
vol.write("GAMES>PACMAN.LST", &data, Some(Timestamp::new(2026, 11, 24, 15, 30, 0)?))?;
std::fs::write(path, vol.image().to_bytes())?; // the editor saves, safely
let blank = Volume::format(Layout::SpartaDouble, Container::Atr)?;
```

### Things that aren't obvious from the API
- **No file I/O in the crate:** the editor reads the image file and
  writes `vol.image().to_bytes()` back (with its usual
  temp-file-then-replace).
- **Names and paths are ATASCII bytes**, exactly as stored, with `>`
  between directories and no `D:` (`"GAMES>PACMAN.LST"`; plain strings
  work for ordinary names). `DirEntry::name` is the stored bytes: pass
  it back in paths unchanged, and show `name_text()`, which turns
  characters ASCII lacks into `?`. The editor already holds ATASCII,
  so names typed in Atari characters can go straight through.
  `atari_disk::atascii` converts to and from ASCII.
- **`write` refuses to replace an existing file** (`Error::AlreadyExists`)
  unless `set_overwrite(true)`: ask "Replace?" and then turn it on.
  Locked files are refused (`Error::Locked`) either way: offer Unlock.
- **Lower case is refused** in new names (`Error::BadName`) unless
  `set_auto_uppercase(true)`. `check_name(name)` checks a name before
  trying, for the Rename / New Dir pop-ups, and says why in the error.
- **Dates come from the program:** `write` and `make_dir` take
  `Option<Timestamp>`; `None` leaves a new file undated and keeps a
  replaced file's date. Only SpartaDOS keeps dates.
- **`vol.features()` says which buttons to offer** (directories, dates,
  volume name, bootable, ...); `Layout::file_system().features()` does
  the same before a disk exists, for the New Image dialog.
- **Write-protect is only reported** (`vol.image().write_protected()`):
  the crate still changes the disk; the editor decides whether to warn
  or refuse.
- **Every change is all or nothing:** on any error the image is as it
  was.
- **Errors** (`atari_disk::Error`, `#[non_exhaustive]`, so `match` needs
  a `_` arm) all have plain-English `Display` text fit to show the
  user: DiskFull, DirectoryFull, BadName, Locked, AlreadyExists,
  NotFound, IsDirectory, NotADirectory, DirectoryNotEmpty, Damaged,
  Unsupported, ...
- **Damaged disks:** on SpartaDOS, changes are refused if anything on
  the disk can't be followed; `vol.validate()` lists the problems
  without changing anything.
- **AltirraSDL** with the image mounted read/write can overwrite the
  editor's changes: warn the user (part of the agreed design).
