#[macro_use]
extern crate log;
extern crate fern;
extern crate libc;
extern crate clap;
extern crate failure;

use clap::{Arg, App, ArgMatches, AppSettings, SubCommand};
use failure::*;

// process cli arguments with clap
fn process_cli<'a>() -> ArgMatches<'a> {
    App::new("snip")
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
                     .takes_value(true)
                     .multiple(true))
                .arg(Arg::with_name("name")
                     .help("name of the snippet")
                     .value_name("NAME")
                     .multiple(true)
                     .last(true)
                     .required(true)))
        .get_matches()
}

fn run() -> Result<(), Error> {
    let app_matches = process_cli();
    println!("{:?}", app_matches);

    match app_matches.subcommand() {
        ("add", Some(sub_matches)) => {
            let name = sub_matches.values_of("name").unwrap().collect::<Vec<&str>>().as_slice().join(" ");
            let tags = sub_matches.values_of("tags").map(|x| x.collect::<Vec<&str>>());

            println!("{}", name);
            println!("{:?}", tags);
        },
        _ => panic!("unexpected error"),
    }

    Ok(())
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
