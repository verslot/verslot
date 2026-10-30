use clap::{Parser, Subcommand};

use crate::logger::{self, Symbol};
use crate::target::Target;

pub(crate) fn parse() -> Cli {
    match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let stderr = error.use_stderr();
            let color = if stderr {
                logger::stderr_color_enabled()
            } else {
                logger::stdout_color_enabled()
            };
            let message = if color {
                error.render().ansi().to_string()
            } else {
                error.render().to_string()
            };
            let symbol = match error.kind() {
                clap::error::ErrorKind::DisplayHelp
                | clap::error::ErrorKind::DisplayVersion
                | clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => Symbol::None,
                _ => Symbol::Error,
            };
            // Match clap's exit path, which ignores diagnostic write failures.
            if stderr {
                let _ = logger::write_message(&mut std::io::stderr(), symbol, &message, color);
            } else {
                let _ = logger::write_message(&mut std::io::stdout(), symbol, &message, color);
            }
            std::process::exit(error.exit_code());
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "verslot",
    version,
    about = "A minimal, extensible tool version manager written in Rust."
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Install a tool version.
    Install {
        #[arg(value_name = "TOOL@VERSION")]
        target: Target,
    },
    /// Uninstall a tool version.
    Uninstall {
        #[arg(value_name = "TOOL@VERSION")]
        target: Target,
    },
    /// Select an installed tool version.
    #[command(name = "use")]
    Use {
        #[arg(value_name = "TOOL@VERSION")]
        target: Target,
    },
    /// List installed tool versions.
    #[command(visible_alias = "ls")]
    List,
    /// Show the selected tool version.
    Current,
}
