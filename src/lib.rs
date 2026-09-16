mod cli;

use clap::Parser;
use cli::{Cli, Command};

pub fn run() -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Command::Install { target } => Err(format!("not implemented: install {target}")),
        Command::Uninstall { target } => Err(format!("not implemented: uninstall {target}")),
        Command::Use { target } => Err(format!("not implemented: use {target}")),
        Command::List => Err("not implemented: list".to_owned()),
        Command::Current => Err("not implemented: current".to_owned()),
    }
}
