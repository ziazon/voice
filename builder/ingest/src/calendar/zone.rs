//! System TZif v2+ data; tzfile(5), DESCRIPTION (headers, data blocks and footer).
use std::{fs::OpenOptions, io::Read, os::unix::fs::OpenOptionsExt, path::Path};

pub const ZONEINFO: &str = "/usr/share/zoneinfo";
const MAX_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone)]
pub struct Zone {
    transitions: Vec<(i64, i32)>,
    before: i32,
    offsets: Vec<i32>,
    future: Future,
}
#[derive(Debug, Clone)]
enum Future {
    Last,
    Fixed(i32),
    Daylight,
}

impl Zone {
    pub fn valid_name(name: &str) -> bool {
        (1..=64).contains(&name.len())
            && name.split('/').all(|part| {
                !part.is_empty()
                    && part
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_+-".contains(&b))
            })
    }
    pub fn load(root: &Path, name: &str) -> Option<Self> {
        if !Self::valid_name(name) {
            return None;
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(root.join(name))
            .ok()?;
        let metadata = file.metadata().ok()?;
        if !metadata.is_file() || metadata.len() > MAX_BYTES {
            return None;
        }
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1).read_to_end(&mut bytes).ok()?;
        if bytes.len() as u64 > MAX_BYTES {
            return None;
        }
        let mut input = Input(&bytes);
        let first = input.header()?;
        input.block(first, 4)?;
        let second = input.header()?;
        let (transitions, types) = input.block(second, 8)?;
        let footer = std::str::from_utf8(input.0.strip_prefix(b"\n")?.strip_suffix(b"\n")?).ok()?;
        if footer.contains(['\n', '\r', '\0']) {
            return None;
        }
        let future = if footer.contains(',') {
            daylight_footer(footer)?;
            Future::Daylight
        } else if footer.is_empty() {
            Future::Last
        } else {
            Future::Fixed(fixed_footer(footer)?)
        };
        let before = types.iter().find(|(_, dst)| !dst).unwrap_or(&types[0]).0;
        let mut offsets: Vec<_> = types.iter().map(|(offset, _)| *offset).collect();
        if let Future::Fixed(offset) = future {
            offsets.push(offset);
        }
        offsets.sort_unstable();
        offsets.dedup();
        Some(Self {
            transitions,
            before,
            offsets,
            future,
        })
    }
    pub fn utc() -> Self {
        Self {
            transitions: Vec::new(),
            before: 0,
            offsets: vec![0],
            future: Future::Fixed(0),
        }
    }
    pub fn offset_at(&self, utc: i64) -> Option<i32> {
        if self.transitions.last().is_none_or(|(last, _)| utc > *last) {
            match self.future {
                Future::Daylight => return None,
                Future::Fixed(offset) => return Some(offset),
                Future::Last => (),
            }
        }
        let i = self.transitions.partition_point(|(at, _)| *at <= utc);
        Some(if i == 0 {
            self.before
        } else {
            self.transitions[i - 1].1
        })
    }
    pub fn to_utc(&self, wall: i64) -> Option<i64> {
        // RFC 5545 §3.3.5: a repeated wall time means the earlier instant.
        if let Some(utc) = self
            .offsets
            .iter()
            .filter_map(|offset| {
                let utc = wall.checked_sub(i64::from(*offset))?;
                (self.offset_at(utc) == Some(*offset)).then_some(utc)
            })
            .min()
        {
            return Some(utc);
        }
        // A missing wall time uses the offset before the spring-forward gap.
        let mut before = self.before;
        for &(at, after) in &self.transitions {
            if after > before
                && wall >= at.checked_add(i64::from(before))?
                && wall < at.checked_add(i64::from(after))?
            {
                let utc = wall.checked_sub(i64::from(before))?;
                self.offset_at(utc)?;
                return Some(utc);
            }
            before = after;
        }
        None
    }
}

type Types = Vec<(i32, bool)>;
type Transitions = Vec<(i64, i32)>;
struct Input<'a>(&'a [u8]);
impl<'a> Input<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let out = self.0.get(..n)?;
        self.0 = &self.0[n..];
        Some(out)
    }
    fn header(&mut self) -> Option<[usize; 6]> {
        let header = self.take(44)?;
        if &header[..4] != b"TZif"
            || !matches!(header[4], b'2'..=b'4')
            || header[5..20].iter().any(|b| *b != 0)
        {
            return None;
        }
        let mut counts = [0; 6];
        for (count, bytes) in counts.iter_mut().zip(header[20..].as_chunks::<4>().0) {
            *count = u32::from_be_bytes(*bytes) as usize;
        }
        let [ut, standard, _, _, types, chars] = counts;
        if !(1..=256).contains(&types)
            || chars == 0
            || (ut != 0 && ut != types)
            || (standard != 0 && standard != types)
        {
            return None;
        }
        Some(counts)
    }
    fn block(&mut self, counts: [usize; 6], width: usize) -> Option<(Transitions, Types)> {
        let [ut, standard, leaps, times, types, chars] = counts;
        let length = times
            .checked_mul(width + 1)?
            .checked_add(types.checked_mul(6)?)?
            .checked_add(chars)?
            .checked_add(leaps.checked_mul(width + 4)?)?
            .checked_add(standard)?
            .checked_add(ut)?;
        let mut block = Input(self.take(length)?);
        let time_bytes = block.take(times.checked_mul(width)?)?;
        let indices = block.take(times)?;
        let type_bytes = block.take(types.checked_mul(6)?)?;
        let designations = block.take(chars)?;
        let mut ttinfo = Vec::new();
        for info in type_bytes.as_chunks::<6>().0 {
            let offset = i32::from_be_bytes(info[..4].try_into().ok()?);
            if info[4] > 1
                || !(-89_999..=93_599).contains(&offset)
                || !designations.get(info[5] as usize..)?.contains(&0)
            {
                return None;
            }
            ttinfo.push((offset, info[4] == 1));
        }
        let mut transitions = Vec::new();
        for (bytes, index) in time_bytes.chunks_exact(width).zip(indices) {
            let at = if width == 4 {
                i64::from(i32::from_be_bytes(bytes.try_into().ok()?))
            } else {
                i64::from_be_bytes(bytes.try_into().ok()?)
            };
            if transitions.last().is_some_and(|(last, _)| *last >= at) {
                return None;
            }
            transitions.push((at, ttinfo.get(*index as usize)?.0));
        }
        let mut last_leap = None;
        for leap in block
            .take(leaps.checked_mul(width + 4)?)?
            .chunks_exact(width + 4)
        {
            let at = if width == 4 {
                i64::from(i32::from_be_bytes(leap[..width].try_into().ok()?))
            } else {
                i64::from_be_bytes(leap[..width].try_into().ok()?)
            };
            if last_leap.is_some_and(|last| last >= at) {
                return None;
            }
            last_leap = Some(at);
        }
        let standard = block.take(standard)?;
        let ut = block.take(ut)?;
        if standard.iter().chain(ut).any(|b| *b > 1)
            || ut
                .iter()
                .enumerate()
                .any(|(i, b)| *b == 1 && standard.get(i) != Some(&1))
        {
            return None;
        }
        Some((transitions, ttinfo))
    }
}

fn fixed_footer(text: &str) -> Option<i32> {
    Some(-clock(designation(text)?, 24)?)
}

// tzfile(5)'s POSIX footer syntax is validated even though DST extrapolation is refused.
fn designation(text: &str) -> Option<&str> {
    if let Some(text) = text.strip_prefix('<') {
        let (name, rest) = text.split_once('>')?;
        (name.len() >= 3
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+-".contains(&b)))
        .then_some(rest)
    } else {
        let n = text.bytes().take_while(u8::is_ascii_alphabetic).count();
        (n >= 3).then_some(&text[n..])
    }
}
fn clock(text: &str, max_hour: i32) -> Option<i32> {
    let (sign, rest) = if let Some(rest) = text.strip_prefix('-') {
        (-1, rest)
    } else {
        (1, text.strip_prefix('+').unwrap_or(text))
    };
    let parts = rest
        .split(':')
        .map(|p| {
            if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
                None
            } else {
                p.parse::<i32>().ok()
            }
        })
        .collect::<Option<Vec<_>>>()?;
    let (h, m, s) = match parts.as_slice() {
        [h] => (*h, 0, 0),
        [h, m] => (*h, *m, 0),
        [h, m, s] => (*h, *m, *s),
        _ => return None,
    };
    if h > max_hour || m > 59 || s > 59 {
        return None;
    }
    Some(sign * (h * 3600 + m * 60 + s))
}
fn offset_prefix(text: &str) -> Option<&str> {
    let n = text
        .bytes()
        .take_while(|b| b.is_ascii_digit() || b"+-:".contains(b))
        .count();
    clock(&text[..n], 24)?;
    Some(&text[n..])
}
fn transition_rule(text: &str) -> Option<()> {
    let (day, time) = text
        .split_once('/')
        .map_or((text, None), |(day, time)| (day, Some(time)));
    if let Some(time) = time {
        clock(time, 167)?;
    }
    if let Some(month) = day.strip_prefix('M') {
        let parts = month
            .split('.')
            .map(|p| {
                if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(());
                }
                p.parse::<u16>().map_err(|_| ())
            })
            .collect::<std::result::Result<Vec<_>, _>>()
            .ok()?;
        let [m, w, d] = parts.as_slice() else {
            return None;
        };
        if !(1..=12).contains(m) || !(1..=5).contains(w) || *d > 6 {
            return None;
        }
    } else {
        let (min, day) = day.strip_prefix('J').map_or((0, day), |day| (1, day));
        if day.is_empty() || !day.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        if !(min..=365).contains(&day.parse::<u16>().ok()?) {
            return None;
        }
    }
    Some(())
}
fn daylight_footer(text: &str) -> Option<()> {
    let mut parts = text.split(',');
    let names = parts.next()?;
    let dst = designation(offset_prefix(designation(names)?)?)?;
    if !dst.is_empty() && !offset_prefix(dst)?.is_empty() {
        return None;
    }
    transition_rule(parts.next()?)?;
    transition_rule(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }
    Some(())
}
