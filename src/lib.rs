use std::io::{self, IsTerminal};

mod cli;
pub mod distribution;
mod doctor;
pub mod download;
mod install;
pub mod installation;
mod inventory;
mod logger;
mod mutation;
mod output;
mod progress;
mod remote;
mod setup;
mod shim;
pub mod storage;
pub mod target;
mod uninstall;
mod yarn;

use cli::Command;
use target::{Target, Tool};

pub fn run() -> Result<(), String> {
    if let Some(command) = shim::invoked_command() {
        return shim::execute(command).map_err(|error| format!("{command}: {error}"));
    }
    let cli = cli::parse();

    match cli.command {
        Command::Setup => {
            let directory = setup::setup().map_err(|error| format!("setup: {error}"))?;
            let directory = output::format_path_for_display(&directory);
            output::print_result(&format!("configured shims at {directory}"));
            logger::print_stdout(
                logger::Symbol::None,
                &format!("Prepend this directory to PATH: {directory}\n"),
            );
            Ok(())
        }
        Command::Doctor { verbose } => {
            let failed = doctor::write_report(&mut io::stdout().lock(), verbose)
                .map_err(|error| format!("doctor: cannot write report: {error}"))?;
            if failed {
                std::process::exit(1);
            }
            Ok(())
        }
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
            let (resolved, changed) = storage
                .select_requested(&target)
                .map_err(|error| format!("use {target}: {error}"))?;
            if changed {
                output::print_result(&format!("selected {resolved}"));
            } else {
                output::print_result(&format!("already selected {resolved}"));
            }
            Ok(())
        }
        Command::Unuse { tool } => {
            let storage =
                storage::Storage::from_env().map_err(|error| format!("unuse {tool}: {error}"))?;
            let changed = storage
                .clear_selection(tool)
                .map_err(|error| format!("unuse {tool}: {error}"))?;
            if changed {
                output::print_result(&format!("unselected {tool}"));
            } else {
                output::print_result(&format!("already unselected {tool}"));
            }
            Ok(())
        }
        Command::List => {
            let installations = inventory::list_installed()?;
            let terminal = io::stdout().is_terminal();
            // Listing remains available when selection state cannot be read.
            let mut selected = Vec::new();
            if terminal && let Ok(storage) = storage::Storage::from_env() {
                for tool in Tool::ALL {
                    if let Ok(Some(version)) = storage.read_selected(tool) {
                        selected.push(Target { tool, version });
                    }
                }
            }
            output::print_list(&installations, &selected, terminal);
            Ok(())
        }
        Command::Current => {
            let storage =
                storage::Storage::from_env().map_err(|error| format!("current: {error}"))?;
            let selected = storage
                .read_selections()
                .map_err(|error| format!("current: {error}"))?;
            logger::print_stdout(
                logger::Symbol::None,
                &output::format_current(&selected, io::stdout().is_terminal()),
            );
            Ok(())
        }
        Command::ListRemote { query } => {
            remote::list(&query).map_err(|error| format!("list-remote {query}: {error}"))
        }
    }
}

/// Prints an operational diagnostic to stderr using the CLI output style.
pub fn print_error(error: &str) {
    output::print_error(error);
}
