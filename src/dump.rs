use std::{
    fs::File,
    io::{self, BufReader, Read, Write},
    num::NonZeroUsize,
};

use eyre::{bail, eyre, WrapErr};

use crate::cli::DumpArgs;

enum ReadResult {
    Eof(usize),
    Ok,
    Err(io::Error),
}

fn read_till_full<R: Read>(mut reader: R, buf: &mut [u8]) -> ReadResult {
    let total_len = buf.len();
    let mut remaining_buf = buf;
    while !remaining_buf.is_empty() {
        match reader.read(remaining_buf) {
            Ok(0) => return ReadResult::Eof(total_len - remaining_buf.len()),
            Ok(n) => remaining_buf = &mut remaining_buf[n..],
            Err(e) => return ReadResult::Err(e),
        }
    }
    ReadResult::Ok
}

struct Printer {
    octets_per_group: usize,
    output_width: usize,
    current_offset: u64,
    line_buf: Vec<u8>,
}

impl Printer {
    fn new(octets_per_group: usize, starting_offset: u64) -> Self {
        Self {
            octets_per_group,
            output_width: 0,
            current_offset: starting_offset,
            line_buf: Vec::new(),
        }
    }

    fn write_line<W: Write>(&mut self, mut out: W, buf: &[u8]) -> io::Result<()> {
        write!(self.line_buf, "{:08x}: ", self.current_offset)?;
        for (i, b) in buf.iter().enumerate() {
            if self.octets_per_group != 0 && i != 0 && i % self.octets_per_group == 0 {
                write!(self.line_buf, " ")?;
            }
            write!(self.line_buf, "{b:02x}")?;
        }

        self.output_width = self.output_width.max(self.line_buf.len());
        let padding = self.output_width - self.line_buf.len();
        out.write_all(&self.line_buf)?;
        self.line_buf.clear();

        write!(out, "{:>1$}", " | ", padding + 3)?;
        for &b in buf {
            let c = if b.is_ascii_graphic() || b == b' ' {
                b.into()
            } else {
                '.'
            };
            write!(out, "{c}")?;
        }
        writeln!(out)?;

        self.current_offset += buf.len() as u64;

        Ok(())
    }
}

pub fn dump(options: DumpArgs) -> eyre::Result<()> {
    let DumpArgs {
        columns,
        groupsize,
        offset,
        input,
    } = options;
    let writer = io::stdout().lock();
    if let Some(path) = &input {
        let reader = BufReader::new(
            File::open(path)
                .wrap_err_with(|| eyre!("failed to open file `{}`", path.to_string_lossy()))?,
        );
        dump_impl(reader, writer, columns, groupsize, offset)
    } else {
        let reader = io::stdin().lock();
        dump_impl(reader, writer, columns, groupsize, offset)
    }
}

pub(crate) fn dump_impl<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    columns: NonZeroUsize,
    groupsize: usize,
    offset: u64,
) -> eyre::Result<()> {
    let mut printer = Printer::new(groupsize, offset);
    let mut buf = vec![0; columns.into()];
    loop {
        match read_till_full(&mut reader, &mut buf) {
            ReadResult::Eof(0) => break,
            ReadResult::Eof(n) => {
                printer
                    .write_line(&mut writer, &buf[..n])
                    .wrap_err("failed to write output")?;
                break;
            }
            ReadResult::Ok => printer
                .write_line(&mut writer, &buf)
                .wrap_err("failed to write output")?,
            ReadResult::Err(e) => bail!(eyre::Report::new(e).wrap_err("failed to read input")),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use proptest::prelude::*;

    use crate::parse::for_parsed_data;

    use super::*;

    fn dump(
        i: impl AsRef<[u8]>,
        columns: usize,
        groupsize: usize,
        offset: u64,
    ) -> eyre::Result<String> {
        let mut o = Vec::new();
        dump_impl(
            i.as_ref(),
            Cursor::new(&mut o),
            columns.try_into()?,
            groupsize,
            offset,
        )?;
        Ok(String::from_utf8(o)?)
    }

    fn hex(s: &str) -> Vec<u8> {
        let mut result = Vec::new();
        for_parsed_data(s, |b| {
            result.push(b);
            Ok(())
        })
        .unwrap();
        result
    }

    #[test]
    fn test_dump() {
        assert!(dump("abcd", 0, 4, 0).is_err());
        assert_eq!(
            dump("abcd", 4, 0, 0).unwrap(),
            "00000000: 61626364 | abcd\n"
        );
        assert_eq!(
            dump("abcd", 1, 0, 0).unwrap(),
            "00000000: 61 | a\n00000001: 62 | b\n00000002: 63 | c\n00000003: 64 | d\n"
        );
        assert_eq!(
            dump("abcd", 2, 0, 0).unwrap(),
            "00000000: 6162 | ab\n00000002: 6364 | cd\n"
        );
        assert_eq!(
            dump("abc", 2, 0, 0).unwrap(),
            "00000000: 6162 | ab\n00000002: 63   | c\n"
        );
        assert_eq!(dump("\n", 2, 0, 0).unwrap(), "00000000: 0a | .\n");
        assert_eq!(dump(hex("aaaa"), 2, 0, 0).unwrap(), "00000000: aaaa | ..\n");
        assert_eq!(dump(hex("aaaa"), 2, 0, 1).unwrap(), "00000001: aaaa | ..\n");
        assert_eq!(
            dump("abc", 2, 0, 3).unwrap(),
            "00000003: 6162 | ab\n00000005: 63   | c\n"
        );
    }

    proptest! {
        #[test]
        fn test_dump_alignment(b in any::<Vec<u8>>(), c in 1..640usize, g in any::<usize>(), o in any::<u64>()) {
            let s = dump(b, c, g, o).unwrap();
            let mut width = 0;
            for l in s.lines() {
                let (o_d, _) = l.split_once('|').unwrap();
                assert!(width <= o_d.len());
                width = o_d.len();
            }
        }
    }
}
