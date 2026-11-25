//! The computer's local date and time, for dating files written to disk
//! images. Rust's standard library only gives UTC, so this asks the
//! operating system directly: `localtime_r` on Linux and macOS,
//! `GetLocalTime` on Windows.

use retro_core::disk::DateTime;

/// The local date and time now, or `None` if the system won't say.
pub fn now() -> Option<DateTime> {
    imp::now()
}

#[cfg(unix)]
mod imp {
    use super::DateTime;
    use std::ffi::{c_char, c_int, c_long};

    /// C's `struct tm` as Linux (glibc, musl) and macOS lay it out.
    #[repr(C)]
    struct Tm {
        tm_sec: c_int,
        tm_min: c_int,
        tm_hour: c_int,
        tm_mday: c_int,
        tm_mon: c_int,
        tm_year: c_int,
        tm_wday: c_int,
        tm_yday: c_int,
        tm_isdst: c_int,
        tm_gmtoff: c_long,
        tm_zone: *const c_char,
    }

    // `time_t` is 64 bits on every platform the editor is built for.
    unsafe extern "C" {
        fn time(t: *mut i64) -> i64;
        fn localtime_r(t: *const i64, out: *mut Tm) -> *mut Tm;
    }

    pub fn now() -> Option<DateTime> {
        let mut tm = Tm {
            tm_sec: 0,
            tm_min: 0,
            tm_hour: 0,
            tm_mday: 0,
            tm_mon: 0,
            tm_year: 0,
            tm_wday: 0,
            tm_yday: 0,
            tm_isdst: 0,
            tm_gmtoff: 0,
            tm_zone: std::ptr::null(),
        };
        // SAFETY: `time` with a null pointer only returns the time;
        // `localtime_r` writes only into `tm`, which outlives the call.
        let ok = unsafe {
            let t = time(std::ptr::null_mut());
            !localtime_r(&t, &mut tm).is_null()
        };
        if !ok {
            return None;
        }
        Some(DateTime {
            year: u16::try_from(tm.tm_year + 1900).ok()?,
            month: u8::try_from(tm.tm_mon + 1).ok()?,
            day: u8::try_from(tm.tm_mday).ok()?,
            hour: u8::try_from(tm.tm_hour).ok()?,
            minute: u8::try_from(tm.tm_min).ok()?,
            // A leap second (60) as 59: SpartaDOS stops at 59.
            second: u8::try_from(tm.tm_sec.min(59)).ok()?,
        })
    }
}

#[cfg(windows)]
mod imp {
    use super::DateTime;

    #[repr(C)]
    #[allow(non_snake_case)]
    struct SystemTime {
        wYear: u16,
        wMonth: u16,
        wDayOfWeek: u16,
        wDay: u16,
        wHour: u16,
        wMinute: u16,
        wSecond: u16,
        wMilliseconds: u16,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLocalTime(out: *mut SystemTime);
    }

    pub fn now() -> Option<DateTime> {
        let mut st = SystemTime {
            wYear: 0,
            wMonth: 0,
            wDayOfWeek: 0,
            wDay: 0,
            wHour: 0,
            wMinute: 0,
            wSecond: 0,
            wMilliseconds: 0,
        };
        // SAFETY: GetLocalTime only fills in the struct it's given.
        unsafe { GetLocalTime(&mut st) };
        Some(DateTime {
            year: st.wYear,
            month: u8::try_from(st.wMonth).ok()?,
            day: u8::try_from(st.wDay).ok()?,
            hour: u8::try_from(st.wHour).ok()?,
            minute: u8::try_from(st.wMinute).ok()?,
            second: u8::try_from(st.wSecond).ok()?,
        })
    }
}

#[cfg(not(any(unix, windows)))]
mod imp {
    pub fn now() -> Option<super::DateTime> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_is_a_sensible_date() {
        let t = now().expect("the system gives the time");
        assert!(t.year >= 2024 && t.year < 2100, "{t:?}");
        assert!((1..=12).contains(&t.month));
        assert!((1..=31).contains(&t.day));
        assert!(t.hour < 24 && t.minute < 60 && t.second < 60);
    }

    /// The same day as UTC from the standard library, give or take the
    /// time zone (at most 14 hours either way).
    #[test]
    fn close_to_utc() {
        let t = now().unwrap();
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let local = days_from_civil(t.year as i64, t.month as i64, t.day as i64) * 86400
            + t.hour as i64 * 3600
            + t.minute as i64 * 60
            + t.second as i64;
        assert!((local - secs).abs() <= 14 * 3600 + 60, "{t:?}");
    }

    /// Days since 1970-01-01 (Howard Hinnant's algorithm).
    fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (m + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe - 719468
    }
}
