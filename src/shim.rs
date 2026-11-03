use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

use crate::installation::{real_directory, regular_file};
use crate::storage::Storage;
use crate::target::{Target, Tool};

pub(crate) const COMMANDS: [&str; 4] = ["verslot", "node", "npm", "npx"];

pub(crate) fn executable_name(command: &str) -> String {
    if cfg!(windows) {
        format!("{command}.exe")
    } else {
        command.to_owned()
    }
}

pub(crate) fn invoked_command() -> Option<&'static str> {
    // cmd.exe may pass argv[0] without .exe; use the actual copied entry on Windows.
    #[cfg(windows)]
    let argument = std::env::current_exe().ok()?.into_os_string();
    #[cfg(not(windows))]
    let argument = std::env::args_os().next()?;
    let name = Path::new(&argument).file_name()?.to_str()?;
    COMMANDS[1..].iter().copied().find(|command| {
        let expected = executable_name(command);
        if cfg!(windows) {
            name.eq_ignore_ascii_case(&expected)
        } else {
            name == expected
        }
    })
}

pub(crate) fn execute(name: &str) -> io::Result<()> {
    let mut command = selected_command(name)?;
    command.args(std::env::args_os().skip(1));
    execute_process(&mut command)
}

fn selected_command(name: &str) -> io::Result<Command> {
    let storage = Storage::from_env()?;
    // read_selected validates under a shared lock and drops it before returning.
    let version = storage.read_selected()?.ok_or_else(|| {
        io::Error::other(
            "no Node.js version selected; run `verslot install node@<version>` and `verslot use node@<version>`",
        )
    })?;
    let root = fs::canonicalize(storage.root())?;
    let installation = storage.installation_path(&Target {
        tool: Tool::Node,
        version,
    })?;
    let installation = fs::canonicalize(installation)?;
    real_directory(&root, &installation)?;
    let executable = installation.join(if cfg!(windows) {
        "node.exe"
    } else {
        "bin/node"
    });
    regular_file(&root, &executable)?;
    let mut command = Command::new(executable);
    if name != "node" {
        let npm = installation.join(if cfg!(windows) {
            "node_modules/npm/bin"
        } else {
            "lib/node_modules/npm/bin"
        });
        let script = npm.join(format!("{name}-cli.js"));
        regular_file(&root, &script).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("selected node@{version} has no usable bundled {name}: {error}"),
            )
        })?;
        command.arg(script);
    }
    Ok(command)
}

#[cfg(unix)]
fn execute_process(command: &mut Command) -> io::Result<()> {
    use std::os::unix::process::CommandExt;

    // No locks or cleanup guards survive the successful process replacement.
    Err(command.exec())
}

#[cfg(windows)]
fn execute_process(command: &mut Command) -> io::Result<()> {
    // Console events also reach the child. Handle them here without exiting early
    // or setting the inheritable "ignore Ctrl+C" flag on the child.
    ctrlc::set_handler(|| {}).map_err(io::Error::other)?;
    let status = command.status()?;
    std::process::exit(status.code().unwrap_or(1));
}

pub(crate) fn is_command_file(name: &OsStr) -> bool {
    COMMANDS
        .iter()
        .any(|command| name == OsStr::new(&executable_name(command)))
}
