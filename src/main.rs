#[macro_use]
extern crate log;
extern crate fern;
extern crate libc;
extern crate clap;
extern crate failure;
extern crate sqlite;
extern crate mktemp;
extern crate ansi_term;

mod snippet;
mod commands;
mod content;
mod error;
mod util;
mod db;

use clap::{Arg, App, ArgMatches, AppSettings, SubCommand};
use std::str::FromStr;
use failure::*;

// process cli arguments with clap
fn process_cli<'a>() -> ArgMatches<'a> {
    App::new("rsm")
        .version("0.1")
        .author("Cristian Kubis <cristian.kubis@tsunix.de>")
        .about("Multi-user snippet manager")
        .setting(AppSettings::DeriveDisplayOrder)
        .setting(AppSettings::SubcommandRequired)
        .subcommand(
            SubCommand::with_name("add")
                .about("Used to add a new snippet")
                .setting(AppSettings::TrailingVarArg)
                .arg(Arg::with_name("tags")
                     .help("adds one or more tags to the snippet")
                     .long("--tags")
                     .short("-t")
                     .takes_value(true)
                     .multiple(true))
                .arg(Arg::with_name("name")
                     .help("name of the snippet")
                     .multiple(true)
                     .required(true)))
        .subcommand(
            SubCommand::with_name("show")
                .about("Used to display a snippet")
                .arg(Arg::with_name("id")
                     .help("id of the snippet")
                     .required(true)))
        .get_matches()
}

fn run() -> Result<(), Error> {
    let app_matches = process_cli();

    match app_matches.subcommand() {
        ("add", Some(sub_matches)) => {
            let name = sub_matches.values_of("name").unwrap().collect::<Vec<&str>>().as_slice().join(" ");
            let tags = sub_matches.values_of("tags").map(|x| x.collect::<Vec<&str>>());

            commands::add_snippet(name, tags)
        },
        ("show", Some(sub_matches)) => {
            let id_str = sub_matches.value_of("id").unwrap();
            let snippet_id = i64::from_str(id_str)
                .context("failed to parse snippet id")?;

            commands::show_snippet(snippet_id)
        },
        _ => panic!("unexpected error"),
    }
}

fn main() {
    // failure crate boilerplate
    if let Err(e) = run() {
        use std::io::Write;
        let mut stderr = std::io::stderr();
        let got_logger = log_enabled!(log::Level::Error);

        let mut fail: &Fail = e.as_fail();
        if got_logger {
            error!("{}", fail);
        } else {
            writeln!(&mut stderr, "{}", fail).ok();
        }

        while let Some(cause) = fail.cause() {
            if got_logger {
                error!("caused by: {}", cause);
            } else {
                writeln!(&mut stderr, "caused by: {}", cause).ok();
            }

            if let Some(bt) = cause.backtrace() {
                error!("backtrace: {}", bt)
            }
            fail = cause;
        }

        stderr.flush().ok();
        ::std::process::exit(1);
    }
}
