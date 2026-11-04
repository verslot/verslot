use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

use crate::installation::{real_directory, regular_file};
use crate::storage::Storage;
use crate::target::{Target, Tool};

pub(crate) const COMMANDS: [&str; 6] = ["verslot", "node", "npm", "npx", "yarn", "yarnpkg"];

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
    if matches!(name, "yarn" | "yarnpkg") {
        return selected_yarn_command(&storage);
    }

    // read_selected validates under a shared lock and drops it before returning.
    let version = storage.read_selected(Tool::Node)?.ok_or_else(|| {
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

fn selected_yarn_command(storage: &Storage) -> io::Result<Command> {
    let version = storage.read_selected(Tool::Yarn)?.ok_or_else(|| {
        io::Error::other(
            "no Yarn version selected; run `verslot install yarn@<version>` and `verslot use yarn@<version>`",
        )
    })?;
    let root = fs::canonicalize(storage.root())?;
    let installation = storage.installation_path(&Target {
        tool: Tool::Yarn,
        version,
    })?;
    let installation = fs::canonicalize(installation)?;
    real_directory(&root, &installation)?;
    let script = installation.join("bin/yarn.js");
    regular_file(&root, &script)?;

    #[cfg(windows)]
    let mut command = Command::new(windows_node_on_path()?);
    #[cfg(not(windows))]
    let mut command = Command::new("node");
    command.arg(script);
    Ok(command)
}

#[cfg(windows)]
fn windows_node_on_path() -> io::Result<std::path::PathBuf> {
    // A bare program name would search the Yarn shim's own directory before PATH.
    let path = std::env::var_os("PATH").unwrap_or_default();
    let executable = std::env::split_paths(&path)
        .filter(|directory| !directory.as_os_str().is_empty())
        .map(|directory| directory.join("node.exe"))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "node.exe not found on PATH"))?;
    std::path::absolute(executable)
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
