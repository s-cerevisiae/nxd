use std::{num::ParseIntError, str::FromStr};

use eyre::{WrapErr, bail, eyre};

#[derive(Debug, PartialEq, Eq)]
pub struct DumpLine<'src> {
    pub offset: &'src str,
    pub data: &'src str,
    pub comment: &'src str,
}

/// Recognizes parts from a single line of the dump format, does not verify.
pub fn recognize_line(line: &str) -> DumpLine<'_> {
    let (rest, comment) = line.split_once('|').unwrap_or((line, ""));
    let (offset, data) = rest.split_once(':').unwrap_or(("", rest));
    DumpLine {
        offset: offset.trim(),
        data: data.trim(),
        comment,
    }
}

/// Iterates over a line of data and feeds resulting bytes into `f`. Stops when encounting any
/// error.
pub fn for_parsed_data(data: &str, mut f: impl FnMut(u8) -> eyre::Result<()>) -> eyre::Result<()> {
    for mut group in data.split_ascii_whitespace() {
        if group.len() % 2 != 0 {
            bail!("incomplete octet in input:\n{data}")
        }
        while let Some(b) = group.get(..2) {
            group = &group[2..];
            let b = u8::from_str_radix(b, 16)
                .wrap_err_with(|| eyre!("failed to parse octet `{b}` in\n{data}"))?;
            f(b)?;
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Offset {
    Absolute(u64),
    Relative(i64),
}

impl FromStr for Offset {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(if s.starts_with(['+', '-']) {
            Self::Relative(i64::from_str_radix(s, 16)?)
        } else {
            Self::Absolute(u64::from_str_radix(s, 16)?)
        })
    }
}

impl Offset {
    pub fn to_absolute(self, basis: Option<u64>) -> eyre::Result<u64> {
        match (self, basis) {
            (Offset::Absolute(n), _) => Ok(n),
            (Offset::Relative(n), None) => {
                bail!("relative offset `{n:x}` found before any absolute ones")
            }
            (Offset::Relative(n), Some(m)) => m
                .checked_add_signed(n)
                .ok_or_else(|| eyre!("relative offset overflown at `{m} + {n}`")),
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn test_recognize_line() {
        let p = |offset, data, comment| DumpLine {
            offset,
            data,
            comment,
        };
        assert_eq!(recognize_line(""), p("", "", ""));
        assert_eq!(recognize_line("abcd"), p("", "abcd", ""));
        assert_eq!(recognize_line("0: abcd"), p("0", "abcd", ""));
        assert_eq!(recognize_line("00000000: abcd"), p("00000000", "abcd", ""));
        assert_eq!(
            recognize_line("  deadbeef: abcd"),
            p("deadbeef", "abcd", "")
        );
        assert_eq!(recognize_line(": abcd"), p("", "abcd", ""));
        assert_eq!(recognize_line(":abcd"), p("", "abcd", ""));
        assert_eq!(recognize_line("0: abcd|????"), p("0", "abcd", "????"));
        assert_eq!(recognize_line("0: abcd |????"), p("0", "abcd", "????"));
        assert_eq!(recognize_line("0: abcd| ????"), p("0", "abcd", " ????"));
        assert_eq!(recognize_line("0: abcd | ????"), p("0", "abcd", " ????"));
        assert_eq!(recognize_line("0: abcd "), p("0", "abcd", ""));
        assert_eq!(recognize_line("0:    abcd "), p("0", "abcd", ""));
        assert_eq!(recognize_line(" abcd "), p("", "abcd", ""));
        assert_eq!(recognize_line("  abcd "), p("", "abcd", ""));
        assert_eq!(recognize_line(" abcd | ????"), p("", "abcd", " ????"));
        assert_eq!(
            recognize_line("00 abcd |  ????"),
            p("", "00 abcd", "  ????")
        );
        assert_eq!(recognize_line("abcd |0:  ????"), p("", "abcd", "0:  ????"));
    }

    #[test]
    fn test_offset_parsing() {
        let abs = Offset::Absolute;
        let rel = Offset::Relative;
        assert_eq!("00000000".parse(), Ok(abs(0)));
        assert_eq!("00000001".parse(), Ok(abs(1)));
        assert_eq!("a".parse(), Ok(abs(10)));
        assert_eq!("A".parse(), Ok(abs(10)));
        assert!(Offset::from_str("g").is_err());
        assert!(Offset::from_str("+").is_err());
        assert!(Offset::from_str("-").is_err());
        assert_eq!("+1".parse(), Ok(rel(1)));
        assert_eq!("-1".parse(), Ok(rel(-1)));
    }

    prop_compose! {
        fn offset_u()(o in "[0-9a-fA-F]{16}") -> String {
            o
        }
    }

    prop_compose! {
        fn offset_i()(o in "[+-][0-9a-fA-F]{8}") -> String {
            o
        }
    }

    proptest! {
        #[test]
        fn test_offset_props_u(o in offset_u()) {
            Offset::from_str(&o).unwrap();
        }

        #[test]
        fn test_offset_props_i(o in offset_i()) {
            Offset::from_str(&o).unwrap();
        }

        #[test]
        fn test_recognize_props(
            o in prop_oneof![offset_i(), offset_u()],
            d in "[0-9a-fA-F]{2}*",
            c in ".*",
        ) {
            assert_eq!(
                recognize_line(&format!("{o}:{d}|{c}")),
                DumpLine { offset: &o, data: &d, comment: &c },
            );
            assert_eq!(
                recognize_line(&format!("|{c}")),
                DumpLine { offset: "", data: "", comment: &c },
            );
        }
    }
}
