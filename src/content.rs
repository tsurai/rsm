use std::process::{Command, ExitStatus};
use std::io::prelude::*;
use std::ffi::OsStr;
use std::path::Path;
use std::{env, io, fs};
use mktemp::Temp;
use failure::*;

fn run_editor<S: AsRef<OsStr>>(file: S) -> Result<(), Error> {
    // get the prefered editor
    let editor = env::var("EDITOR")
        .unwrap_or("/usr/bin/editor".to_string());

    // start the editor and wait for its exit status
    let status: ExitStatus = Command::new(editor.as_str())
        .arg(file)
        .status()
        .map_err::<Error, _>(|e| e.into())?;

    // check if it exited successfully
    (match status.code() {
        Some(0) => Ok(()),
        Some(code) => Err(format_err!("status code: {}", code)),
        None => Err(format_err!("terminated by signal")),
    })
    .context("editor exited unexpectedly")?;

    Ok(())
}

fn read_file_content<P: AsRef<Path>>(path: P) -> Result<String, Error> {
    let mut file = fs::File::open(path.as_ref())
        .context("failed to open tmp file: {}")?;

    let mut content = String::new();
    file.read_to_string(&mut content)
        .context("failed to read tmp file")?;

    Ok(content)
}

pub fn get_from_stdin() -> Result<String, Error> {
    let reader = io::stdin();
    let mut content = String::new();

    loop {
        match reader.read_line(&mut content) {
            // size of zero implies EOF
            Ok(0) => break,
            Ok(_) => continue,
            Err(e) => return Err(e.into()),
        }
    }

    Ok(content)
}

pub fn get_from_editor() -> Result<String, Error> {
    // tmp file gets deleted when going out of scope
    let tmp_file = Temp::new_file()
        .context("failed to create temporary file")?;

    // run the editor and write the content to the tmp file
    run_editor(tmp_file.as_ref())
        .context("failed to start editor")?;

    // read the content written by the editor
    let content = read_file_content(tmp_file.as_ref())
        .context("failed to fetch content from tmp file")?;

    Ok(content)
}
