mod cli;
pub mod distribution;
pub mod download;
mod install;
pub mod installation;
mod inventory;
mod mutation;
pub mod storage;
pub mod target;
mod uninstall;

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
        Command::Uninstall { target } => {
            let message = uninstall::uninstall(&target)?;
            println!("{message}");
            Ok(())
        }
        Command::Use { target } => {
            let storage =
                storage::Storage::from_env().map_err(|error| format!("use {target}: {error}"))?;
            let changed = storage
                .select(&target)
                .map_err(|error| format!("use {target}: {error}"))?;
            if changed {
                println!("using {target}");
            } else {
                println!("already using {target}");
            }
            Ok(())
        }
        Command::List => {
            let installations = inventory::list_installed()?;
            for target in installations {
                println!("{target}");
            }
            Ok(())
        }
        Command::Current => {
            let storage =
                storage::Storage::from_env().map_err(|error| format!("current: {error}"))?;
            if let Some(version) = storage
                .read_selected()
                .map_err(|error| format!("current: {error}"))?
            {
                println!("node@{version}");
            }
            Ok(())
        }
    }
}
