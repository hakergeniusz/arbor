//! Display formatting helpers: sizes, item counts, relative dates.
//!
//! Size formatting deliberately uses GLib's `g_format_size`, whose decimal
//! output ("4.1 kB", "122.3 MB", "40 bytes") matches every value observed in
//! the approved specification (docs/04 §4). GLib separates the number and the
//! unit with a non-breaking space (U+00A0); the observed application does the
//! same, so this is the spec-exact rendering.


// Some helpers are wired up in later implementation phases or used only
// by unit tests.
#![allow(dead_code)]

use gtk::glib;

/// Non-breaking space used by GLib between number and unit.
pub const NBSP: &str = "\u{a0}";

/// Format a byte count in decimal units, e.g. `122.3 MB` or `40 bytes`.
pub fn size(bytes: u64) -> String {
    glib::format_size(bytes).to_string()
}

/// Format an item count, e.g. `28 items` / `1 item`.
pub fn items(count: u64) -> String {
    if count == 1 {
        "1 item".to_string()
    } else {
        format!("{count} items")
    }
}

/// Format a modification time as a relative day label: "Today", "1 day",
/// "13 days". Calendar-day based (docs decision D13); future mtimes clamp to
/// "Today".
pub fn relative_day(mtime_unix: i64, now: &glib::DateTime) -> String {
    let Ok(mtime) = glib::DateTime::from_unix_local(mtime_unix) else {
        return "Unknown".to_string();
    };
    let days = calendar_days_between(now, &mtime); // positive → mtime in the past
    match days {
        d if d <= 0 => "Today".to_string(),
        1 => "1 day".to_string(),
        d => format!("{d} days"),
    }
}

/// Whole days between two local dates (a - b), ignoring time of day.
fn calendar_days_between(a: &glib::DateTime, b: &glib::DateTime) -> i64 {
    let (ay, am, ad) = (a.year() as i64, a.month() as i64, a.day_of_month() as i64);
    let (by, bm, bd) = (b.year() as i64, b.month() as i64, b.day_of_month() as i64);
    days_from_civil(ay, am, ad) - days_from_civil(by, bm, bd)
}

/// Days since 1970-01-01 for a proleptic Gregorian civil date
/// (Howard Hinnant's `days_from_civil` algorithm, public domain formulation).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (m + 9) % 12; // March=0 .. February=11
    let doy = (153 * mp + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_match_spec_values() {
        // Values verified in docs/04 §4 against fixtures.
        // GLib renders the unit separator as a non-breaking space.
        assert_eq!(size(256), "256 bytes");
        assert_eq!(size(4096), format!("4.1{NBSP}kB"));
        assert_eq!(size(8252), format!("8.3{NBSP}kB"));
        assert_eq!(size(10_485_760), format!("10.5{NBSP}MB"));
        assert_eq!(size(122_297_731), format!("122.3{NBSP}MB"));
        assert_eq!(size(102_400), format!("102.4{NBSP}kB"));
        assert_eq!(size(1_000_000), format!("1.0{NBSP}MB"));
        assert_eq!(size(40), "40 bytes");
        assert_eq!(size(0), "0 bytes");
    }

    #[test]
    fn item_counts() {
        assert_eq!(items(1), "1 item");
        assert_eq!(items(28), "28 items");
        assert_eq!(items(0), "0 items");
    }

    #[test]
    fn relative_days() {
        let base = glib::DateTime::from_unix_local(0).unwrap(); // 1970-01-01 local
        assert_eq!(relative_day(0, &base), "Today");
        assert_eq!(relative_day(-86_400, &base), "1 day");
        assert_eq!(relative_day(-13 * 86_400, &base), "13 days");
        assert_eq!(relative_day(3_600, &base), "Today"); // future clamps to Today
    }

    #[test]
    fn civil_days() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(1970, 1, 2), 1);
        assert_eq!(days_from_civil(2026, 11, 4) - days_from_civil(2026, 10, 22), 13);
        assert_eq!(days_from_civil(2000, 3, 1) - days_from_civil(2000, 2, 28), 2);
    }
}
