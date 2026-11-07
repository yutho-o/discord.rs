//! ISO 8601 timestamps with no date library, `ARCHITECTURE.md` 7.8.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Microseconds since the Unix epoch. Tolerant parser, stable writer:
/// `2021-01-01T00:00:00.123456+00:00`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp(pub i64);

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (i64::from(m) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

impl Timestamp {
    /// Accepts `YYYY-MM-DDTHH:MM:SS[.f{1,9}](Z|+HH:MM|-HH:MM)`. Never panics on bad input.
    pub fn parse(s: &str) -> Option<Self> {
        let b = s.as_bytes();
        let sep = |i: usize, c: u8| b.get(i) == Some(&c);
        if b.len() < 20 || !sep(4, b'-') || !sep(7, b'-') || !sep(10, b'T') || !sep(13, b':') || !sep(16, b':') {
            return None;
        }
        let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
        let (y, mo, d, h, mi, se) = (n(0..4)?, n(5..7)?, n(8..10)?, n(11..13)?, n(14..16)?, n(17..19)?);
        let (mut i, mut micros) = (19usize, 0i64);
        if b.get(i) == Some(&b'.') {
            i += 1;
            let start = i;
            let mut scale = 100_000i64;
            while let Some(c) = b.get(i).filter(|c| c.is_ascii_digit()) {
                micros += i64::from(c - b'0') * scale;
                scale /= 10;
                i += 1;
            }
            if i == start {
                return None;
            }
        }
        let off_s = match *b.get(i)? {
            b'Z' | b'z' => 0,
            sign @ (b'+' | b'-') => {
                let oh = n(i + 1..i + 3)?;
                let om = n(i + 4..i + 6)?;
                let v = oh * 3600 + om * 60;
                if sign == b'-' { -v } else { v }
            }
            _ => return None,
        };
        let days = days_from_civil(y, u32::try_from(mo).ok()?, u32::try_from(d).ok()?);
        Some(Self((days * 86_400 + h * 3600 + mi * 60 + se - off_s) * 1_000_000 + micros))
    }

    /// Milliseconds since the Unix epoch.
    pub const fn as_millis(self) -> i64 {
        self.0.div_euclid(1000)
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (secs, micros) = (self.0.div_euclid(1_000_000), self.0.rem_euclid(1_000_000));
        let (days, tod) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
        let (y, m, d) = civil_from_days(days);
        write!(f, "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{micros:06}+00:00", tod / 3600, tod / 60 % 60, tod % 60)
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = std::borrow::Cow::<str>::deserialize(d)?;
        Self::parse(&s).ok_or_else(|| serde::de::Error::custom(format_args!("invalid timestamp {s:?}")))
    }
}
