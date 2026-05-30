use std::{
    env,
    fs::File,
    io::{BufReader, BufWriter},
    process::Command,
};

use eyre::{WrapErr, bail, eyre};

use crate::{cli::EditArgs, dump::dump_impl, load::load_impl};

pub fn edit(options: EditArgs) -> eyre::Result<()> {
    let EditArgs {
        columns,
        groupsize,
        input,
    } = options;
    let Some((file_name, dir)) = input
        .is_file()
        .then(|| input.file_name().zip(input.parent()))
        .flatten()
    else {
        bail!("`{}` does not exist or is a directory", input.display());
    };
    let mut dump_tmp = tempfile::Builder::new()
        .prefix(file_name)
        .suffix(".nxd")
        .tempfile_in(dir)
        .wrap_err_with(|| {
            eyre!(
                "failed to create temporary file for editing in `{}`",
                dir.display()
            )
        })?;
    let input_file =
        File::open(&input).wrap_err_with(|| eyre!("failed to open file `{}`", input.display()))?;
    let input_file_perm = input_file.metadata()?.permissions();
    dump_impl(
        BufReader::new(input_file),
        BufWriter::new(&mut dump_tmp),
        columns,
        groupsize,
        0,
    )?;
    let file_to_edit = dump_tmp.into_temp_path();

    let editor = env::var_os("EDITOR").ok_or_else(|| eyre!("$EDITOR is not set"))?;
    Command::new(editor)
        .arg(&file_to_edit)
        .spawn()
        .wrap_err("failed to start editor")?
        .wait()?;

    let mut target = tempfile::Builder::new()
        .prefix(file_name)
        .permissions(input_file_perm)
        .tempfile_in(dir)?;
    load_impl(
        BufReader::new(File::open(&file_to_edit)?),
        BufWriter::new(&mut target),
    )?;
    target
        .persist(&input)
        .wrap_err_with(|| eyre!("failed to save file at `{}`", input.display()))?;
    Ok(())
}
