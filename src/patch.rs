use std::{
    fs::File,
    io::{self, BufRead, BufWriter, Seek, Write},
};

use eyre::{WrapErr, ensure, eyre};

use crate::{
    cli::PatchArgs,
    parse::{DumpLine, Offset, for_parsed_data, recognize_line},
};

pub fn patch(args: PatchArgs) -> eyre::Result<()> {
    let target = BufWriter::new(
        File::options()
            .write(true)
            .read(false)
            .open(&args.target)
            .wrap_err_with(|| {
                eyre!(
                    "failed to open target file at `{}`",
                    args.target.to_string_lossy()
                )
            })?,
    );

    let input = io::stdin().lock();

    patch_impl(input, target, args.offset)
}

pub(crate) fn patch_impl<R: BufRead, W: Write + Seek>(
    reader: R,
    mut writer: W,
    global_offset: i64,
) -> eyre::Result<()> {
    let mut last_offset = None;
    for line in reader.lines() {
        let line = line?;
        let DumpLine { offset, data, .. } = recognize_line(&line);
        if !offset.is_empty() {
            let offset = offset
                .parse::<Offset>()
                .wrap_err_with(|| eyre!("invalid offset `{offset}`"))?
                .to_absolute(last_offset)?;
            writer.seek(io::SeekFrom::Start(
                offset
                    .checked_add_signed(global_offset)
                    .ok_or_else(|| eyre!("failed to add `{global_offset}` to offset `{offset}`"))?,
            ))?;
            last_offset = Some(offset)
        }
        ensure!(
            last_offset.is_some(),
            "offset must be defined before any data in patch"
        );
        for_parsed_data(data, |b| {
            writer
                .write_all(&[b])
                .wrap_err("failed to write to target file")
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use io::Cursor;

    use super::*;

    fn patch(i: &str, o: impl Into<Vec<u8>>) -> eyre::Result<Vec<u8>> {
        patch_offset(i, o, 0)
    }

    fn patch_offset(i: &str, o: impl Into<Vec<u8>>, offset: i64) -> eyre::Result<Vec<u8>> {
        let mut o = o.into();
        patch_impl(i.as_bytes(), Cursor::new(&mut o), offset)?;
        Ok(o)
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
    fn test_patch() {
        assert!(patch("aa", []).is_err());
        assert_eq!(
            patch("0: aabbcc", hex("ddccbbaa")).unwrap(),
            hex("aabbccaa")
        );
        assert_eq!(patch("0: aabbcc", hex("dd")).unwrap(), hex("aabbcc"));
        assert_eq!(patch("1: aabbcc", hex("ddeeff")).unwrap(), hex("ddaabbcc"));
        assert_eq!(
            patch("0: aabb\ncc", hex("ddccbbaa")).unwrap(),
            hex("aabbccaa")
        );
        assert_eq!(
            patch("0: aa\nbb\ncc", hex("ddccbbaa")).unwrap(),
            hex("aabbccaa")
        );
        assert_eq!(
            patch("0: aabb\n2:cc", hex("ddccbbaa")).unwrap(),
            hex("aabbccaa")
        );
        assert_eq!(
            patch("0: aabb\n1:cc", hex("ddccbbaa")).unwrap(),
            hex("aaccbbaa")
        );
        assert_eq!(patch("", hex("ddccbbaa")).unwrap(), hex("ddccbbaa"));
        assert_eq!(patch("0:", hex("ddccbbaa")).unwrap(), hex("ddccbbaa"));
        assert_eq!(patch("2500:", hex("ddccbbaa")).unwrap(), hex("ddccbbaa"));
        assert_eq!(patch("0:\naabb", hex("ddccbbaa")).unwrap(), hex("aabbbbaa"));
        assert_eq!(
            patch("5:aabb", hex("ddccbbaa")).unwrap(),
            hex("ddccbbaa00aabb")
        );
        assert_eq!(
            patch("01:01\n+02:03", hex("ddccbbaa")).unwrap(),
            hex("dd01bb03")
        );
        assert_eq!(
            patch("01:01\n-01:00", hex("ddccbbaa")).unwrap(),
            hex("0001bbaa")
        );
        assert!(patch("+01:01", hex("ddccbbaa")).is_err());
        assert!(patch("00:\n-01:01", hex("ddccbbaa")).is_err());
    }

    #[test]
    fn test_patch_offset() {
        assert_eq!(
            patch_offset("0:aa", hex("ddccbbaa"), 1).unwrap(),
            hex("ddaabbaa")
        );
        assert_eq!(
            patch_offset("0:aa", hex("ddccbbaa"), 4).unwrap(),
            hex("ddccbbaaaa")
        );
        assert_eq!(
            patch_offset("1:01\n+1:02", hex("ddccbbaa"), 1).unwrap(),
            hex("ddcc0102")
        );
        assert_eq!(
            patch_offset("1:01\n+1:02", hex("ddccbbaa"), -1).unwrap(),
            hex("0102bbaa")
        );
        assert!(patch_offset("1:01\n+1:02", hex("ddccbbaa"), -2).is_err());
    }
}
