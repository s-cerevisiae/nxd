use std::{
    fs::File,
    io::{self, BufRead, BufReader, Write},
};

use eyre::{WrapErr, eyre};

use crate::{
    cli::LoadArgs,
    parse::{for_parsed_data, recognize_line},
};

pub fn load(options: LoadArgs) -> eyre::Result<()> {
    let writer = io::stdout().lock();
    if let Some(path) = options.input {
        let reader = BufReader::new(
            File::open(&path)
                .wrap_err_with(|| eyre!("failed to open file `{}`", path.to_string_lossy()))?,
        );
        load_impl(reader, writer)
    } else {
        let reader = io::stdin().lock();
        load_impl(reader, writer)
    }
}

pub(crate) fn load_impl<R: BufRead, W: Write>(reader: R, mut writer: W) -> eyre::Result<()> {
    for line in reader.lines() {
        let line = line?;
        let data = recognize_line(&line).data;
        for_parsed_data(data, |b| {
            writer.write_all(&[b]).wrap_err("failed to write output")
        })?;
    }

    Ok(())
}
