//! Atari font files (`.FNT`): 128 characters of 8 bytes, in the order the
//! character set sits in ROM (the order the hardware reads from CHBASE),
//! as Atari font editors save them. The editor's character sets are 256
//! characters in ATASCII order with the inverse half drawn in; this turns
//! one into the other.
//!
//! ROM order is ATASCII order with the first three quarters rotated:
//!
//! | ROM codes   | ATASCII codes | Characters                  |
//! |-------------|---------------|-----------------------------|
//! | `$00-$1F`   | `$20-$3F`     | space, punctuation, digits  |
//! | `$20-$3F`   | `$40-$5F`     | `@`, capitals, `[\]^_`      |
//! | `$40-$5F`   | `$00-$1F`     | control graphics            |
//! | `$60-$7F`   | `$60-$7F`     | diamond, lower case, etc.   |

/// Bytes in a font file: 128 characters of 8.
pub const FNT_SIZE: usize = 1024;

/// The ATASCII code of the character at ROM position `rom` (`$00-$7F`).
fn atascii_of_rom(rom: usize) -> usize {
    match rom {
        0x00..=0x1F => rom + 0x20,
        0x20..=0x3F => rom + 0x20,
        0x40..=0x5F => rom - 0x40,
        _ => rom,
    }
}

/// A font file's 1024 bytes, also accepted saved as a DOS binary-load
/// file (`FF FF`, start and end address, then the 1024 bytes).
fn font_bytes(file: &[u8]) -> Result<&[u8], String> {
    if file.len() == FNT_SIZE {
        return Ok(file);
    }
    if file.len() == FNT_SIZE + 6 && file[..2] == [0xFF, 0xFF] {
        let start = u16::from_le_bytes([file[2], file[3]]) as usize;
        let end = u16::from_le_bytes([file[4], file[5]]) as usize;
        if end >= start && end - start + 1 == FNT_SIZE {
            return Ok(&file[6..]);
        }
    }
    Err(format!(
        "it's {} bytes; an Atari font is {FNT_SIZE} (128 characters of 8 bytes)",
        file.len()
    ))
}

/// A font file as a 256-character set in ATASCII order: the 128
/// characters rearranged, and the inverse ones made from them, as the
/// Atari's hardware does.
pub fn charset_from_fnt(file: &[u8]) -> Result<Vec<u8>, String> {
    let rom = font_bytes(file)?;
    let mut set = vec![0u8; 2048];
    for (pos, glyph) in rom.as_chunks::<8>().0.iter().enumerate() {
        let code = atascii_of_rom(pos);
        set[code * 8..code * 8 + 8].copy_from_slice(glyph);
        let inverse = (code + 0x80) * 8;
        for (i, &row) in glyph.iter().enumerate() {
            set[inverse + i] = !row;
        }
    }
    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The built-in character set's normal half, written out as a font
    /// file in ROM order.
    fn builtin_as_fnt() -> Vec<u8> {
        let set = include_bytes!("font_atascii.bin");
        let mut fnt = vec![0u8; FNT_SIZE];
        for pos in 0..128 {
            let code = atascii_of_rom(pos);
            fnt[pos * 8..pos * 8 + 8].copy_from_slice(&set[code * 8..code * 8 + 8]);
        }
        fnt
    }

    #[test]
    fn rom_order_becomes_atascii_order() {
        // Reading the built-in set back from ROM order gives the same set,
        // inverse half included.
        let set = charset_from_fnt(&builtin_as_fnt()).unwrap();
        assert_eq!(&set[..], &include_bytes!("font_atascii.bin")[..]);
    }

    #[test]
    fn binary_load_header_is_skipped() {
        let fnt = builtin_as_fnt();
        let mut file = vec![0xFF, 0xFF, 0x00, 0xE0, 0xFF, 0xE3];
        file.extend_from_slice(&fnt);
        assert_eq!(charset_from_fnt(&file), charset_from_fnt(&fnt));
        // A header that doesn't describe 1024 bytes isn't a font.
        file[5] = 0xE4;
        assert!(charset_from_fnt(&file).is_err());
        assert!(charset_from_fnt(&fnt[..1000]).is_err());
    }
}
