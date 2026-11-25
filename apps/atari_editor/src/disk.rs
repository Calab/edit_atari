//! Atari disk images (.ATR and .XFD) for File > Open from Image and Save
//! to Image, using the atari-disk crate: Atari DOS 2, MyDOS and
//! SpartaDOS.

use atari_disk::{DiskImage, Error, FileSystem, Timestamp, Volume};
use retro_core::disk::{
    DateTime, DiskEntry, DiskError, DiskErrorKind, DiskImages, DiskResult, DiskVolume,
};

/// Atari DOS's separator between directories in a path.
const SEPARATOR: u8 = b'>';

pub struct AtariDisks;

impl DiskImages for AtariDisks {
    fn name(&self) -> &'static str {
        "Atari disk images"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ATR", "XFD"]
    }

    fn top_directory(&self) -> &'static [u8] {
        &[SEPARATOR]
    }

    fn open(&self, bytes: &[u8]) -> DiskResult<Box<dyn DiskVolume>> {
        let vol = Volume::open(DiskImage::from_bytes(bytes).map_err(error)?).map_err(error)?;
        if vol.file_system() == FileSystem::Raw {
            return Err(DiskError::other(
                "the disk has no DOS the editor knows (Atari DOS 2, MyDOS or \
                 SpartaDOS): it may be a game or a boot disk",
            ));
        }
        Ok(Box::new(AtariVolume(vol)))
    }
}

struct AtariVolume(Volume);

impl AtariVolume {
    /// Refuse to change a write-protected image.
    fn writable(&self) -> DiskResult<()> {
        if self.0.image().write_protected() {
            Err(DiskError::new(
                DiskErrorKind::WriteProtected,
                "the image is write-protected",
            ))
        } else {
            Ok(())
        }
    }
}

/// The file system's name, as the info line shows it.
fn dos_name(fs: FileSystem) -> &'static str {
    match fs {
        FileSystem::Dos2 => "Atari DOS 2",
        FileSystem::Dos25Enhanced => "Atari DOS 2.5",
        FileSystem::MyDos => "MyDOS",
        FileSystem::SpartaDos => "SpartaDOS",
        FileSystem::SpartaDosX => "SpartaDOS X",
        _ => "No DOS",
    }
}

/// `GAMES>PAC.COM` from `["GAMES", "PAC.COM"]`. A name holding `>`
/// (written by some other tool) can't be reached by a path.
fn join(path: &[Vec<u8>]) -> DiskResult<Vec<u8>> {
    if let Some(name) = path.iter().find(|n| n.contains(&SEPARATOR)) {
        return Err(DiskError::other(format!(
            "{} has > in its name, so it can't be reached",
            atari_disk::atascii::atascii_to_ascii(name)
        )));
    }
    Ok(path.join(&SEPARATOR))
}

fn error(e: Error) -> DiskError {
    let kind = match e {
        Error::AlreadyExists(_) => DiskErrorKind::AlreadyExists,
        Error::Locked(_) => DiskErrorKind::Locked,
        Error::NotFound(_) => DiskErrorKind::NotFound,
        _ => DiskErrorKind::Other,
    };
    DiskError::new(kind, e.to_string())
}

/// A date as SpartaDOS keeps it. One it can't hold (before 1980 or after
/// 2079, from a computer with its clock wrong) is left off rather than
/// stopping the save.
fn timestamp(when: Option<DateTime>) -> Option<Timestamp> {
    when.and_then(|t| Timestamp::new(t.year, t.month, t.day, t.hour, t.minute, t.second).ok())
}

impl DiskVolume for AtariVolume {
    fn info(&self) -> String {
        let image = self.0.image();
        let volume = match self.0.volume_name() {
            Some(n) if !n.trim_ascii().is_empty() => format!(
                ", volume {}",
                atari_disk::atascii::atascii_to_ascii(n.trim_ascii())
            ),
            _ => String::new(),
        };
        format!(
            "{}{volume}, {} sectors of {} bytes, {} free",
            dos_name(self.0.file_system()),
            image.sector_count(),
            image.sector_size(),
            self.0.free_sectors()
        )
    }

    fn volume_name(&self) -> Option<Vec<u8>> {
        self.0.volume_name()
    }

    fn has_directories(&self) -> bool {
        self.0.features().directories
    }

    fn has_dates(&self) -> bool {
        self.0.features().dates
    }

    fn write_protected(&self) -> bool {
        self.0.image().write_protected()
    }

    fn list(&self, dir: &[Vec<u8>]) -> DiskResult<Vec<DiskEntry>> {
        let entries = self.0.list(join(dir)?).map_err(error)?;
        Ok(entries
            .into_iter()
            .map(|e| DiskEntry {
                is_dir: e.is_dir(),
                size: e.size,
                sectors: e.sectors,
                locked: e.locked,
                modified: e.modified.map(|t| DateTime {
                    year: t.year,
                    month: t.month,
                    day: t.day,
                    hour: t.hour,
                    minute: t.minute,
                    second: t.second,
                }),
                damaged: e.damaged,
                name: e.name,
            })
            .collect())
    }

    fn read(&self, path: &[Vec<u8>]) -> DiskResult<Vec<u8>> {
        self.0.read(join(path)?).map_err(error)
    }

    fn write(
        &mut self,
        path: &[Vec<u8>],
        data: &[u8],
        replace: bool,
        when: Option<DateTime>,
    ) -> DiskResult<()> {
        self.writable()?;
        let path = join(path)?;
        let when = timestamp(when);
        self.0.set_overwrite(replace);
        let done = self.0.write(path, data, when).map_err(error);
        self.0.set_overwrite(false);
        done
    }

    fn make_dir(&mut self, path: &[Vec<u8>], when: Option<DateTime>) -> DiskResult<()> {
        self.writable()?;
        let path = join(path)?;
        self.0.make_dir(path, timestamp(when)).map_err(error)
    }

    fn check_name(&self, name: &[u8]) -> DiskResult<()> {
        self.0.check_name(name).map_err(error)
    }

    fn name_char(&self, code: u8) -> Option<u8> {
        match code {
            b'A'..=b'Z' | b'0'..=b'9' | b'.' => Some(code),
            b'a'..=b'z' => Some(code.to_ascii_uppercase()),
            b'_' if self.0.features().underscore_in_names => Some(code),
            _ => None,
        }
    }

    fn to_bytes(&self) -> Vec<u8> {
        self.0.image().to_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atari_disk::{Container, Layout};

    fn image(layout: Layout, container: Container) -> Vec<u8> {
        Volume::format(layout, container)
            .unwrap()
            .image()
            .to_bytes()
    }

    fn open(bytes: &[u8]) -> Box<dyn DiskVolume> {
        AtariDisks.open(bytes).unwrap()
    }

    fn p(names: &[&str]) -> Vec<Vec<u8>> {
        names.iter().map(|n| n.as_bytes().to_vec()).collect()
    }

    const WHEN: DateTime = DateTime {
        year: 2026,
        month: 11,
        day: 24,
        hour: 15,
        minute: 30,
        second: 0,
    };

    #[test]
    fn write_then_read_back_through_the_image_bytes() {
        for (layout, container) in [
            (Layout::Dos2Single, Container::Atr),
            (Layout::Dos2Single, Container::Xfd),
            (Layout::Dos25Enhanced, Container::Atr),
            (Layout::Dos2Double, Container::Atr),
            (Layout::MyDosSingle, Container::Atr),
            (Layout::SpartaSingle, Container::Atr),
            (Layout::SpartaDouble, Container::Atr),
        ] {
            let mut vol = open(&image(layout, container));
            let data: Vec<u8> = (0..5000).map(|i| i as u8).collect();
            vol.write(&p(&["SAMPLE.LST"]), &data, false, Some(WHEN))
                .unwrap();
            // The editor writes the bytes to the file and reads them back.
            let again = open(&vol.to_bytes());
            assert_eq!(again.read(&p(&["SAMPLE.LST"])).unwrap(), data, "{layout:?}");
            let list = again.list(&[]).unwrap();
            assert_eq!(list.len(), 1);
            assert_eq!(list[0].name, b"SAMPLE.LST");
            assert_eq!(list[0].size, 5000);
            assert!(!list[0].is_dir);
            let dated = again.has_dates();
            assert_eq!(list[0].modified, dated.then_some(WHEN), "{layout:?}");
        }
    }

    #[test]
    fn dates_sparta_cant_hold_are_left_off() {
        let mut vol = open(&image(Layout::SpartaSingle, Container::Atr));
        let wrong = DateTime { year: 1970, ..WHEN };
        vol.write(&p(&["A"]), b"x", false, Some(wrong)).unwrap();
        assert_eq!(vol.list(&[]).unwrap()[0].modified, None);
    }

    #[test]
    fn info_line() {
        let vol = open(&image(Layout::Dos2Single, Container::Atr));
        assert_eq!(
            vol.info(),
            "Atari DOS 2, 720 sectors of 128 bytes, 707 free"
        );
        let vol = open(&image(Layout::SpartaDouble, Container::Atr));
        assert!(
            vol.info()
                .starts_with("SpartaDOS, 720 sectors of 256 bytes, ")
        );
        let mut named = Volume::format(Layout::SpartaDouble, Container::Atr).unwrap();
        named.set_volume_name("TOOLKIT").unwrap();
        let vol = open(&named.image().to_bytes());
        assert!(
            vol.info()
                .starts_with("SpartaDOS, volume TOOLKIT, 720 sectors of 256 bytes, ")
        );
    }

    #[test]
    fn replace_asks_and_locked_is_refused() {
        let mut vol = open(&image(Layout::Dos2Single, Container::Atr));
        let name = p(&["A.TXT"]);
        vol.write(&name, b"one", false, None).unwrap();
        let e = vol.write(&name, b"two", false, None).unwrap_err();
        assert_eq!(e.kind, DiskErrorKind::AlreadyExists);
        assert_eq!(e.to_string(), "A.TXT already exists");
        vol.write(&name, b"two", true, None).unwrap();
        assert_eq!(vol.read(&name).unwrap(), b"two");
        // Replacing doesn't stay switched on.
        let e = vol.write(&name, b"three", false, None).unwrap_err();
        assert_eq!(e.kind, DiskErrorKind::AlreadyExists);

        let mut locked = Volume::open(DiskImage::from_bytes(&vol.to_bytes()).unwrap()).unwrap();
        locked.lock("A.TXT").unwrap();
        let mut vol = open(&locked.image().to_bytes());
        let e = vol.write(&name, b"four", true, None).unwrap_err();
        assert_eq!(e.kind, DiskErrorKind::Locked);
        assert!(vol.list(&[]).unwrap()[0].locked);
        assert_eq!(vol.read(&name).unwrap(), b"two");
    }

    #[test]
    fn write_protected_images_are_never_changed() {
        let mut v = Volume::format(Layout::SpartaSingle, Container::Atr).unwrap();
        v.set_write_protected(true).unwrap();
        let bytes = v.image().to_bytes();
        let mut vol = open(&bytes);
        assert!(vol.write_protected());
        let e = vol.write(&p(&["A"]), b"x", false, None).unwrap_err();
        assert_eq!(e.kind, DiskErrorKind::WriteProtected);
        let e = vol.make_dir(&p(&["GAMES"]), None).unwrap_err();
        assert_eq!(e.kind, DiskErrorKind::WriteProtected);
        assert_eq!(vol.to_bytes(), bytes);
    }

    #[test]
    fn directories() {
        let mut vol = open(&image(Layout::SpartaDouble, Container::Atr));
        assert!(vol.has_directories());
        vol.make_dir(&p(&["GAMES"]), Some(WHEN)).unwrap();
        vol.make_dir(&p(&["GAMES", "OLD"]), None).unwrap();
        vol.write(&p(&["GAMES", "OLD", "PAC.BAS"]), b"x", false, None)
            .unwrap();
        let top = vol.list(&[]).unwrap();
        assert!(top[0].is_dir && top[0].name == b"GAMES");
        let old = vol.list(&p(&["GAMES", "OLD"])).unwrap();
        assert_eq!(old[0].name, b"PAC.BAS");
        let e = vol.list(&p(&["NONE"])).unwrap_err();
        assert_eq!(e.kind, DiskErrorKind::NotFound);

        let mut dos = open(&image(Layout::Dos2Single, Container::Atr));
        assert!(!dos.has_directories());
        assert!(dos.make_dir(&p(&["GAMES"]), None).is_err());
    }

    #[test]
    fn names() {
        let dos = open(&image(Layout::Dos2Single, Container::Atr));
        let sparta = open(&image(Layout::SpartaSingle, Container::Atr));
        assert!(dos.check_name(b"GAME.BAS").is_ok());
        assert!(dos.check_name(b"MY_GAME").is_err());
        assert!(sparta.check_name(b"MY_GAME").is_ok());
        let e = dos.check_name(b"TOOLONGNAME").unwrap_err();
        assert!(e.to_string().starts_with("bad name: "), "{e}");

        assert_eq!(dos.name_char(b'a'), Some(b'A'));
        assert_eq!(dos.name_char(b'7'), Some(b'7'));
        assert_eq!(dos.name_char(b'.'), Some(b'.'));
        assert_eq!(dos.name_char(b'_'), None);
        assert_eq!(sparta.name_char(b'_'), Some(b'_'));
        assert_eq!(dos.name_char(b' '), None);
        assert_eq!(dos.name_char(b'>'), None);
        assert_eq!(dos.name_char(0xC1), None); // inverse A
    }

    #[test]
    fn not_images_and_disks_without_a_dos() {
        let e = AtariDisks.open(b"hello").err().unwrap();
        assert!(e.to_string().starts_with("not a disk image"), "{e}");
        let blank = DiskImage::new(Container::Atr, 128, 720).unwrap().to_bytes();
        let e = AtariDisks.open(&blank).err().unwrap();
        assert!(e.to_string().contains("no DOS"), "{e}");
    }

    #[test]
    fn names_with_the_separator_cant_be_reached() {
        let vol = open(&image(Layout::Dos2Single, Container::Atr));
        assert!(vol.read(&[b"A>B".to_vec()]).is_err());
    }
}
