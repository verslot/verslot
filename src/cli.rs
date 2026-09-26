use clap::{Parser, Subcommand};

use crate::target::Target;

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
    /// Uninstall a tool version (not implemented yet).
    Uninstall {
        #[arg(value_name = "TOOL@VERSION")]
        target: Target,
    },
    /// Select a tool version (not implemented yet).
    #[command(name = "use")]
    Use {
        #[arg(value_name = "TOOL@VERSION")]
        target: Target,
    },
    /// List installed tool versions (not implemented yet).
    List,
    /// Show the current tool versions (not implemented yet).
    Current,
}
