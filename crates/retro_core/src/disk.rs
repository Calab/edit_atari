//! Disk images: the interface a machine implements so the editor can
//! browse an image and read and write the files on it (File > Open from
//! Image, Save to Image).
//!
//! The editor never keeps an image open. For every action it reads the
//! image file, opens it with [`DiskImages::open`], makes the change, and
//! writes [`DiskVolume::to_bytes`] back itself. Nothing here does file
//! I/O.
//!
//! Names are machine codes exactly as stored on the disk (ATASCII on the
//! Atari), so they can be drawn in the machine's font and given back
//! unchanged. A path is a list of names, directories first: the machine
//! joins them with its own separator.

use std::fmt;

/// A date and time, as kept for files on disks that keep them. Local
/// time, as the computer showed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateTime {
    pub year: u16,
    /// 1-12.
    pub month: u8,
    /// 1-31.
    pub day: u8,
    /// 0-23.
    pub hour: u8,
    /// 0-59.
    pub minute: u8,
    /// 0-59.
    pub second: u8,
}

/// One entry in a directory listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskEntry {
    /// The name as stored, in machine codes, e.g. `SAMPLE.LST`. Give it
    /// back unchanged in a path to reach the entry.
    pub name: Vec<u8>,
    /// A directory, not a file.
    pub is_dir: bool,
    /// Length in bytes.
    pub size: u32,
    /// Sectors (or blocks) it takes up.
    pub sectors: u32,
    pub locked: bool,
    /// When it was last written, on disks that keep dates.
    pub modified: Option<DateTime>,
    /// The disk's records for it don't add up: reading it will fail.
    pub damaged: bool,
}

/// What kind of problem a disk call had, for the cases the editor
/// handles differently (asking "Replace?", for one).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskErrorKind {
    /// A file or directory of that name is already there.
    AlreadyExists,
    /// The file is locked.
    Locked,
    /// The image is write-protected: the editor never changes it.
    WriteProtected,
    /// No such file or directory.
    NotFound,
    /// Anything else: the message says what.
    Other,
}

/// A problem with a disk image, with text fit to show the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskError {
    pub kind: DiskErrorKind,
    /// Plain English, starting in lower case: "disk full: 12 sectors
    /// needed, 3 free".
    pub message: String,
}

impl DiskError {
    pub fn new(kind: DiskErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn other(message: impl Into<String>) -> Self {
        Self::new(DiskErrorKind::Other, message)
    }
}

impl fmt::Display for DiskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DiskError {}

pub type DiskResult<T> = Result<T, DiskError>;

/// The kinds of disk image a machine reads: one for the whole program.
pub trait DiskImages {
    /// What to call them in the file dialog, e.g. "Atari disk images".
    fn name(&self) -> &'static str;

    /// Their file extensions, in upper case without dots, e.g.
    /// `["ATR", "XFD"]`.
    fn extensions(&self) -> &'static [&'static str];

    /// How the top directory is shown, in machine codes, e.g. `>` on the
    /// Atari (as SpartaDOS writes `D1:>`).
    fn top_directory(&self) -> &'static [u8] {
        b"/"
    }

    /// Open an image file's bytes. Fails if they aren't an image, or the
    /// disk has no file system the machine knows.
    fn open(&self, bytes: &[u8]) -> DiskResult<Box<dyn DiskVolume>>;
}

/// One open disk image, held in memory.
pub trait DiskVolume {
    /// A line describing the disk, e.g. "SpartaDOS, volume GAMES, 720
    /// sectors of 256 bytes, 312 free".
    fn info(&self) -> String;

    /// The disk's own name, if its file system has one.
    fn volume_name(&self) -> Option<Vec<u8>>;

    /// The file system has directories.
    fn has_directories(&self) -> bool;

    /// The file system keeps dates.
    fn has_dates(&self) -> bool;

    /// The image is write-protected. [`DiskVolume::write`] and
    /// [`DiskVolume::make_dir`] then always fail.
    fn write_protected(&self) -> bool;

    /// The entries in directory `dir` (`&[]` for the top one), in the
    /// order the disk keeps them.
    fn list(&self, dir: &[Vec<u8>]) -> DiskResult<Vec<DiskEntry>>;

    /// A file's contents.
    fn read(&self, path: &[Vec<u8>]) -> DiskResult<Vec<u8>>;

    /// Write a file. If it's already there, fails with
    /// [`DiskErrorKind::AlreadyExists`] unless `replace`; a locked file is
    /// refused either way. `when` dates it, on disks that keep dates
    /// (`None`: a new file is undated, a replaced one keeps its date).
    /// On any error the disk is unchanged.
    fn write(
        &mut self,
        path: &[Vec<u8>],
        data: &[u8],
        replace: bool,
        when: Option<DateTime>,
    ) -> DiskResult<()>;

    /// Make a directory (the last name in `path`) inside an existing one.
    fn make_dir(&mut self, path: &[Vec<u8>], when: Option<DateTime>) -> DiskResult<()>;

    /// Is `name` allowed for a new file or directory? `Err` says why not.
    fn check_name(&self, name: &[u8]) -> DiskResult<()>;

    /// A character typed into a name: the code to put in the name (lower
    /// case as capitals, say), or `None` if names can't contain it.
    fn name_char(&self, code: u8) -> Option<u8>;

    /// The image file's bytes, with any changes.
    fn to_bytes(&self) -> Vec<u8>;
}
