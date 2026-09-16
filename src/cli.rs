use clap::{Parser, Subcommand};

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
    Install {
        #[arg(value_name = "TOOL@VERSION")]
        target: String,
    },
    Uninstall {
        #[arg(value_name = "TOOL@VERSION")]
        target: String,
    },
    #[command(name = "use")]
    Use {
        #[arg(value_name = "TOOL@VERSION")]
        target: String,
    },
    List,
    Current,
}
