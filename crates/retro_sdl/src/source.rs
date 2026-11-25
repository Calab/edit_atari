//! Where a document came from, and where Save writes it back: a file on
//! the computer, or a file inside a disk image.

use std::path::{Path, PathBuf};

/// Starts a recent-files entry for a file in an image. Saved paths are
/// absolute, so no ordinary entry starts this way.
const IMAGE_PREFIX: &str = "image:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A file on the computer.
    File(PathBuf),
    /// A file inside a disk image: the image file, and the file's path
    /// in it (directories first, names as stored on the disk).
    Image { image: PathBuf, path: Vec<Vec<u8>> },
}

impl Source {
    /// The file on the computer: the file itself, or the image it's in.
    /// Its time and size show whether another program changed it.
    pub fn host(&self) -> &Path {
        match self {
            Source::File(p) => p,
            Source::Image { image, .. } => image,
        }
    }

    /// The document's own name: the file's, or (in an image) the name of
    /// the file on the disk.
    pub fn name(&self) -> String {
        match self {
            Source::File(p) => p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            Source::Image { path, .. } => path.last().map(|n| name_text(n)).unwrap_or_default(),
        }
    }

    /// The name with its image, for the window title and the recent
    /// files: "MYDISK.ATR > SAMPLE.LST" (directories aren't shown).
    pub fn label(&self) -> String {
        match self {
            Source::File(_) => self.name(),
            Source::Image { image, .. } => {
                let disk = image
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                format!("{disk} > {}", self.name())
            }
        }
    }

    /// The name's extension, for choosing the file's format.
    pub fn extension(&self) -> Option<String> {
        match self {
            Source::File(p) => p.extension().map(|e| e.to_string_lossy().into_owned()),
            Source::Image { path, .. } => {
                let name = path.last()?;
                let dot = name.iter().rposition(|&c| c == b'.')?;
                Some(name_text(&name[dot + 1..]))
            }
        }
    }

    /// The text kept in the settings' recent files: the path for a file;
    /// for a file in an image, `image:`, the path inside (names joined by
    /// `/`, each byte other than plain ASCII letters, digits and `._-` as
    /// `%XX`), `:`, then the image's path.
    pub fn to_recent(&self) -> String {
        match self {
            Source::File(p) => p.to_string_lossy().into_owned(),
            Source::Image { image, path } => {
                let inside: Vec<String> = path.iter().map(|n| escape(n)).collect();
                format!(
                    "{IMAGE_PREFIX}{}:{}",
                    inside.join("/"),
                    image.to_string_lossy()
                )
            }
        }
    }

    /// Read back [`Source::to_recent`].
    pub fn from_recent(s: &str) -> Source {
        if let Some(rest) = s.strip_prefix(IMAGE_PREFIX)
            && let Some((inside, image)) = rest.split_once(':')
            && let Some(path) = inside.split('/').map(unescape).collect::<Option<Vec<_>>>()
        {
            return Source::Image {
                image: PathBuf::from(image),
                path,
            };
        }
        Source::File(PathBuf::from(s))
    }
}

/// A disk name as text: printable ASCII as itself, anything else `?`.
pub fn name_text(name: &[u8]) -> String {
    name.iter()
        .map(|&c| match c {
            0x20..=0x7E => c as char,
            _ => '?',
        })
        .collect()
}

fn escape(name: &[u8]) -> String {
    name.iter()
        .map(|&c| match c {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-' => (c as char).to_string(),
            _ => format!("%{c:02X}"),
        })
        .collect()
}

fn unescape(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(path: &[&[u8]]) -> Source {
        Source::Image {
            image: PathBuf::from("/home/steve/Atari/MY DISK:2.ATR"),
            path: path.iter().map(|n| n.to_vec()).collect(),
        }
    }

    #[test]
    fn recent_round_trips() {
        let odd: &[u8] = &[b'A', 0x9B, b'%', b'/', b':', 0xC1, b'>', b' '];
        for s in [
            Source::File(PathBuf::from("/home/steve/PROG.BAS")),
            Source::File(PathBuf::from(r"C:\Users\steve\PROG.BAS")),
            image(&[b"SAMPLE.LST"]),
            image(&[b"GAMES", b"OLD", b"PAC.BAS"]),
            image(&[b"GAMES", odd]),
        ] {
            assert_eq!(Source::from_recent(&s.to_recent()), s, "{}", s.to_recent());
        }
        assert_eq!(
            image(&[b"GAMES", b"PAC.BAS"]).to_recent(),
            "image:GAMES/PAC.BAS:/home/steve/Atari/MY DISK:2.ATR"
        );
    }

    #[test]
    fn names_and_labels() {
        let s = image(&[b"GAMES", b"PAC.BAS"]);
        assert_eq!(s.name(), "PAC.BAS");
        assert_eq!(s.label(), "MY DISK:2.ATR > PAC.BAS");
        assert_eq!(s.extension().as_deref(), Some("BAS"));
        assert_eq!(s.host(), Path::new("/home/steve/Atari/MY DISK:2.ATR"));
        assert_eq!(image(&[b"README"]).extension(), None);
        assert_eq!(image(&[&[b'A', 0x9B]]).name(), "A?");

        let f = Source::File(PathBuf::from("/x/PROG.TUR"));
        assert_eq!(f.name(), "PROG.TUR");
        assert_eq!(f.label(), "PROG.TUR");
        assert_eq!(f.extension().as_deref(), Some("TUR"));
    }
}
