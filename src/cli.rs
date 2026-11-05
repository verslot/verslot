use std::ffi::OsStr;

use clap::{CommandFactory, Parser, Subcommand};

use crate::logger::{self, Symbol};
use crate::remote::RemoteQuery;
use crate::target::{Target, Tool, VersionRequest};

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
            let message = format_cli_message(&error, color, std::env::args_os().nth(1).as_deref());
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

fn format_cli_message(error: &clap::Error, color: bool, subcommand_name: Option<&OsStr>) -> String {
    let rendered = error.render();
    let mut message = if color {
        rendered.ansi().to_string()
    } else {
        rendered.to_string()
    };
    let help_hint = "\n\nFor more information, try '";
    if let Some(index) = message.rfind(help_hint) {
        let cli = Cli::command();
        let command = match subcommand_name.and_then(|name| cli.find_subcommand(name)) {
            Some(subcommand) => format!("verslot {}", subcommand.get_name()),
            None => "verslot".to_owned(),
        };
        message.insert_str(index + help_hint.len(), &format!("{command} "));
    }
    message
}

#[derive(Debug, Parser)]
#[command(
    name = "verslot",
    bin_name = "verslot",
    version,
    about = "A minimal, extensible tool version manager written in Rust."
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Create or refresh the unified command entry points.
    Setup,
    /// Install a tool version.
    Install {
        /// Exact version, major, or major.minor; partial versions install the newest remote match.
        #[arg(value_name = "TOOL@VERSION")]
        target: VersionRequest,
    },
    /// Uninstall a tool version.
    Uninstall {
        #[arg(value_name = "TOOL@VERSION")]
        target: Target,
    },
    /// Select an installed tool version.
    #[command(name = "use")]
    Use {
        /// Exact version, major, or major.minor; partial versions select the newest installed match.
        #[arg(value_name = "TOOL@VERSION")]
        target: VersionRequest,
    },
    /// Clear a tool's global version selection, keeping its installed versions.
    Unuse {
        /// Tool name without a version.
        #[arg(value_name = "TOOL")]
        tool: Tool,
    },
    /// List installed tool versions.
    #[command(visible_alias = "ls")]
    List,
    /// List remotely available tool versions.
    #[command(visible_alias = "ls-remote")]
    ListRemote {
        #[arg(value_name = "QUERY")]
        query: RemoteQuery,
    },
    /// Show the selected tool version.
    Current,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_help_commands_preserve_colored_diagnostics() {
        for arguments in [
            vec!["verslot.exe", "-v"],
            vec!["verslot.exe", "use"],
            vec!["verslot.exe", "use", "node@22."],
        ] {
            let error = Cli::try_parse_from(arguments.iter().copied()).unwrap_err();
            let subcommand_name = arguments.get(1).map(OsStr::new);
            let plain = format_cli_message(&error, false, subcommand_name);
            let colored = format_cli_message(&error, true, subcommand_name);
            assert!(colored.contains('\x1b'));
            assert_eq!(clap::builder::StyledStr::from(colored).to_string(), plain);
        }
    }
}
