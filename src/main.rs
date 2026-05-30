use std::{io, process::ExitCode};

use nxd::{
    cli::{CliArgs, SubCmd},
    dump::dump,
    edit::edit,
    load::load,
    patch::patch,
};

fn main() -> ExitCode {
    let cli_args: CliArgs = argh::from_env();

    let result = match cli_args.subcmd {
        SubCmd::Dump(d) => dump(d),
        SubCmd::Load(l) => load(l),
        SubCmd::Edit(e) => edit(e),
        SubCmd::Patch(p) => patch(p),
    };

    if let Err(e) = result {
        if e.downcast_ref::<io::Error>()
            .is_none_or(|e| e.kind() != io::ErrorKind::BrokenPipe)
        {
            eprintln!("error: {e:?}");
        }
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
