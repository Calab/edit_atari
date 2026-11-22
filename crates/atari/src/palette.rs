//! The Atari's colours: 16 hues x 8 luminances, as set by SETCOLOR.
//!
//! An approximation of the NTSC GTIA palette, generated in YIQ space: hue 0
//! is grey, hues 1-15 step 24 degrees around the colour wheel (1 gold,
//! 2 orange, 3 red, 4 pink, 5 purple, 6-8 blues, 9 light blue, 10 turquoise,
//! 11 green-blue, 12 green, 13-15 yellow-greens).

/// Number of hues and of luminance levels.
pub const HUES: u8 = 16;
pub const LUMS: u8 = 8;

/// The text screen at power-up: background $94 (SETCOLOR 2,9,4) and text
/// with luminance $A of the same hue (SETCOLOR 1 gives $CA, but in GRAPHICS 0
/// only its luminance is used). Border $00.
pub const DEFAULT_BACKGROUND: (u8, u8) = (9, 2);
pub const DEFAULT_TEXT: (u8, u8) = (9, 5);
pub const BORDER: (u8, u8) = (0, 0);

/// RGB for a hue (0-15) and luminance level (0-7, i.e. Atari luminance / 2).
pub fn rgb(hue: u8, lum: u8) -> (u8, u8, u8) {
    let (hue, lum) = (hue.min(HUES - 1), lum.min(LUMS - 1));
    let y = if hue == 0 { 0.0 } else { 0.03 } + f32::from(lum) / 7.0 * 0.93;
    let (i, q) = if hue == 0 {
        (0.0, 0.0)
    } else {
        let a = (-27.0 + f32::from(hue - 1) * 24.0_f32).to_radians();
        (0.3 * a.cos(), 0.3 * a.sin())
    };
    let c = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    (
        c(y + 0.956 * i + 0.621 * q),
        c(y - 0.272 * i - 0.647 * q),
        c(y - 1.106 * i + 1.703 * q),
    )
}

/// The colour as an Atari colour byte, e.g. "$94".
pub fn label(hue: u8, lum: u8) -> String {
    format!("${:X}{:X}", hue, lum * 2)
}

/// Attract mode, as the OS does it: the colour register value is XORed
/// with COLRSH (`shift`, which changes every 256 frames) and ANDed with
/// DRKMSK ($F6), which clears the top luminance bit so everything dims.
pub fn attract((hue, lum): (u8, u8), shift: u8) -> (u8, u8) {
    let byte = ((hue << 4) | (lum << 1)) ^ shift;
    let byte = byte & 0xF6;
    (byte >> 4, (byte & 0x0E) >> 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_colours() {
        assert_eq!(rgb(0, 0), (0, 0, 0));
        // $94 is a mid blue: blue clearly strongest.
        let (r, g, b) = rgb(9, 2);
        assert!(b > g && g > r, "{r} {g} {b}");
        // $C4 is green.
        let (r, g, b) = rgb(12, 2);
        assert!(g > r && g > b, "{r} {g} {b}");
        assert_eq!(label(9, 2), "$94");
        assert_eq!(label(9, 5), "$9A");
    }

    #[test]
    fn attract_dims() {
        for shift in 0..=255u8 {
            let (_, lum) = attract(DEFAULT_TEXT, shift);
            // DRKMSK clears luminance bit 3, so at most $6.
            assert!(lum <= 3);
        }
        assert_eq!(attract((9, 5), 0), (9, 1)); // $9A & $F6 = $92
    }
}
