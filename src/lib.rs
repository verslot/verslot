mod cli;
pub mod distribution;
pub mod download;
mod install;
pub mod installation;
mod mutation;
pub mod storage;
pub mod target;

use clap::Parser;
use cli::{Cli, Command};

pub fn run() -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Command::Install { target } => {
            let message = install::install(&target)?;
            println!("{message}");
            Ok(())
        }
        Command::Uninstall { target } => Err(format!("not implemented: uninstall {target}")),
        Command::Use { target } => Err(format!("not implemented: use {target}")),
        Command::List => Err("not implemented: list".to_owned()),
        Command::Current => Err("not implemented: current".to_owned()),
    }
}
