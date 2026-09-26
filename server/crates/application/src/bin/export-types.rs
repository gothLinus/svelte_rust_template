use std::{env, path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let Some(dir) = env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: export-types <output-dir>");
        return ExitCode::FAILURE;
    };

    match application::types::export(&dir) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("export-types: {err}");
            ExitCode::FAILURE
        }
    }
}
