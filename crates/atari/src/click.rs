//! The XL/XE key click.
//!
//! The OS click routine is:
//!
//! ```text
//!         LDX #$7F
//! LOOP    STX CONSOL   ; bit 3 drives the console speaker
//!         STX WSYNC    ; wait for the next scanline
//!         DEX
//!         BPL LOOP
//! ```
//!
//! so the speaker follows bit 3 of a counter running 127 -> 0, one step per
//! scanline: a square wave toggling every 8 scanlines (~984 Hz on NTSC) for
//! 128 scanlines (~8.1 ms).

/// NTSC scanline period in seconds (114 cycles at 1.7898 MHz).
const SCANLINE: f64 = 114.0 / 1_789_773.0;

/// Generate the click as mono f32 samples in -volume..=volume.
pub fn key_click(sample_rate: u32, volume: f32) -> Vec<f32> {
    let lines = 128.0;
    let n = (lines * SCANLINE * sample_rate as f64).ceil() as usize;
    let mut out = Vec::with_capacity(n + 1);
    for i in 0..n {
        let t = i as f64 / sample_rate as f64;
        let line = (t / SCANLINE) as i32; // 0..127
        let x = 127 - line;
        out.push(if x & 8 != 0 { volume } else { -volume });
    }
    // Return the speaker to rest so the click doesn't end in a pop.
    out.push(0.0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_is_about_8ms_with_8_cycles() {
        let s = key_click(48_000, 0.5);
        let ms = s.len() as f64 / 48.0;
        assert!((7.5..9.0).contains(&ms), "{ms} ms");
        let rising = s.windows(2).filter(|w| w[0] < 0.0 && w[1] > 0.0).count();
        assert_eq!(rising, 7); // 8 high phases, the first starts the click
    }
}
