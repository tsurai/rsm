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

use clap::{Arg, App, ArgGroup, ArgMatches, AppSettings, SubCommand};
use std::str::FromStr;
use failure::*;

// process cli arguments with clap
fn process_cli<'a>() -> ArgMatches<'a> {
    let id_arg = Arg::with_name("id")
        .help("id of the snippet")
        .required(true);

    let name_arg = Arg::with_name("name")
        .help("name of the snippet")
        .multiple(true);

    let tag_arg = Arg::with_name("tags")
        .help("tags to modify")
        .short("-t")
        .long("--tags")
        .takes_value(true)
        .multiple(true);

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
                .arg(&tag_arg)
                .arg(&name_arg
                    .clone()
                    .required(true)))
        .subcommand(
            SubCommand::with_name("show")
                .about("Used to display a snippet")
                .arg(&id_arg))
        .subcommand(
            SubCommand::with_name("modify")
                .about("Used to modify a snippet")
                .arg(&tag_arg
                    .clone()
                    .conflicts_with("name")
                    .requires("modifier"))
                .arg(Arg::with_name("add")
                    .help("add a new tag")
                    .short("-a")
                    .long("--add"))
                .arg(Arg::with_name("remove")
                    .help("remove a tag")
                    .short("-r")
                    .long("--remove"))
                .arg(Arg::with_name("name")
                    .help("new snippet name")
                    .short("-n")
                    .long("--name")
                    .takes_value(true)
                    .multiple(true)
                    .conflicts_with_all(&["modifier", "tags"]))
                .group(ArgGroup::with_name("modifier")
                    .args(&["add", "remove"])
                    .conflicts_with("name")
                    .requires("tags"))
                .arg(&id_arg))
        .subcommand(
            SubCommand::with_name("delete")
                .about("Used to delete a snippet")
                .arg(Arg::with_name("confirm")
                    .help("don't ask for confirmation")
                    .short("-y")
                    .long("--yes"))
                .arg(&id_arg))
        .subcommand(
            SubCommand::with_name("list")
                .about("Used to list snippets")
                .setting(AppSettings::TrailingVarArg)
                .arg(&tag_arg)
                .arg(&name_arg))
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
        ("modify", Some(sub_matches)) => {
            let name = sub_matches.values_of("name").map(|x| x.collect::<Vec<&str>>().as_slice().join(" "));
            let tags = sub_matches.values_of("tags").map(|x| x.collect::<Vec<&str>>());
            let id_str = sub_matches.value_of("id").unwrap();
            let snippet_id = i64::from_str(id_str)
                .context("failed to parse snippet id")?;

            let op = if let Some(x) = name {
                commands::ModifyOperation::Name(x)
            } else if sub_matches.is_present("add") {
                commands::ModifyOperation::Add(tags.unwrap())
            } else {
                commands::ModifyOperation::Remove(tags.unwrap())
            };

            commands::modify_snippet(snippet_id, op)
        },
        ("delete", Some(sub_matches)) => {
            let confirmation = sub_matches.is_present("confirm");
            let id_str = sub_matches.value_of("id").unwrap();
            let snippet_id = i64::from_str(id_str)
                .context("failed to parse snippet id")?;

            commands::delete_snippet(snippet_id, confirmation)
        },
        ("list", Some(sub_matches)) => {
            let name = sub_matches.values_of("name").map(|x| x.collect::<Vec<&str>>().as_slice().join(" "));
            let tags = sub_matches.values_of("tags").map(|x| x.collect::<Vec<&str>>());

            commands::list_snippets(name, tags)
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
