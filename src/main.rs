// Copyright 2021 Aaron Bentley <aaron@aaronbentley.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
#![cfg_attr(feature = "strict", deny(warnings))]
use clap::{Parser};
use std::env;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::exit;

mod git;
use git::make_git_command;
mod commands;
mod worktree;
use commands::{ArgMaker, NativeCommand, RewriteCommand, Runnable};

#[derive(Debug, Parser)]
#[command()]
enum Opt {
    #[structopt(flatten)]
    NativeCommand(NativeCommand),
    #[structopt(flatten)]
    RewriteCommand(RewriteCommand),
}

enum Args {
    NativeCommand(NativeCommand),
    GitCommand(Vec<String>),
}

fn parse_args() -> Args {
    let mut args_iter = env::args();
    let progpath = PathBuf::from(args_iter.next().unwrap());
    let args_vec: Vec<String> = args_iter.collect();
    let args_vec2: Vec<String> = env::args().collect();
    let progname = progpath.file_name().unwrap().to_str().unwrap();
    let opt = match progname {
        "oaf" => {
            if args_vec2.len() > 1 {
                let x = Opt::try_parse_from(&args_vec2[0..2]);
                if let Err(e) = x {
                    if let clap::error::ErrorKind::UnknownArgument | clap::error::ErrorKind::InvalidSubcommand = e.kind()
                    {
                        return Args::GitCommand(args_vec);
                    }
                }
            }
            Opt::parse()
        }
        _ => {
            let mut args = vec!["oaf".to_string()];
            args.push(match progname.split_once('-') {
                Some(("git", _)) => {
                    eprintln!("Unsupported command name {}", progname);
                    exit(1);
                }
                Some((_, cmd)) => cmd.to_owned(),
                _ => {
                    eprintln!("Unsupported command name {}", progname);
                    exit(1);
                }
            });
            args.extend(args_vec.into_iter());
            Opt::parse_from(args)
        }
    };
    match opt {
        Opt::RewriteCommand(cmd) => Args::GitCommand(match cmd.make_args() {
            Ok(args) => args,
            Err(_) => {
                exit(1);
            }
        }),
        Opt::NativeCommand(cmd) => Args::NativeCommand(cmd),
    }
}

fn main() {
    let opt = parse_args();
    match opt {
        Args::NativeCommand(cmd) => exit(cmd.run()),
        Args::GitCommand(args_vec) => {
            make_git_command(&args_vec).exec();
        }
    };
}
