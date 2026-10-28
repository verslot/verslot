use std::io::{self, IsTerminal};

mod cli;
pub mod distribution;
pub mod download;
mod install;
pub mod installation;
mod inventory;
mod mutation;
mod output;
mod progress;
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
            output::print_result(&message);
            Ok(())
        }
        Command::Uninstall { target } => {
            let message = uninstall::uninstall(&target)?;
            output::print_result(&message);
            Ok(())
        }
        Command::Use { target } => {
            let storage =
                storage::Storage::from_env().map_err(|error| format!("use {target}: {error}"))?;
            let changed = storage
                .select(&target)
                .map_err(|error| format!("use {target}: {error}"))?;
            if changed {
                output::print_result(&format!("selected {target}"));
            } else {
                output::print_result(&format!("already selected {target}"));
            }
            Ok(())
        }
        Command::List => {
            let installations = inventory::list_installed()?;
            let terminal = io::stdout().is_terminal();
            // Listing remains available when selection state cannot be read.
            let selected = if terminal {
                storage::Storage::from_env()
                    .ok()
                    .and_then(|storage| storage.read_selected().ok().flatten())
            } else {
                None
            };
            print!(
                "{}",
                output::format_list(&installations, selected, terminal)
            );
            Ok(())
        }
        Command::Current => {
            let storage =
                storage::Storage::from_env().map_err(|error| format!("current: {error}"))?;
            let selected = storage
                .read_selected()
                .map_err(|error| format!("current: {error}"))?;
            print!(
                "{}",
                output::format_current(selected, io::stdout().is_terminal())
            );
            Ok(())
        }
    }
}

/// Prints an operational diagnostic to stderr using the CLI output style.
pub fn print_error(error: &str) {
    output::print_error(error);
}
