use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::installation::regular_file;
use crate::logger;
use crate::output::format_path_for_display as display_path;
use crate::shim;
use crate::storage::Storage;
use crate::target::{Target, Tool, Version};

mod paths;

struct Report<'a> {
    output: &'a mut dyn Write,
    verbose: bool,
    has_findings: bool,
    failed: bool,
}

impl Report<'_> {
    fn line(&mut self, message: fmt::Arguments<'_>) -> io::Result<()> {
        logger::write_plain_line(self.output, &message.to_string())
    }

    fn detail(&mut self, message: fmt::Arguments<'_>) -> io::Result<()> {
        if self.verbose {
            self.line(message)?;
        }
        Ok(())
    }

    fn finding(&mut self, message: fmt::Arguments<'_>) -> io::Result<()> {
        self.has_findings = true;
        self.line(message)
    }

    fn error(&mut self, message: fmt::Arguments<'_>) -> io::Result<()> {
        self.failed = true;
        self.finding(format_args!("Error: {message}"))
    }
}

enum ShimFinding {
    Missing,
    NotOnPath,
    Shadowed,
}

#[derive(PartialEq, Eq)]
enum ShimDirectory {
    Missing,
    OnPath,
    Unavailable,
}

/// Writes the report and returns whether any check failed.
pub(crate) fn write_report(output: &mut impl Write, verbose: bool) -> io::Result<bool> {
    let mut report = Report {
        output,
        verbose,
        has_findings: false,
        failed: false,
    };
    let path = std::env::var_os("PATH");
    let path = path.as_deref();
    let storage = Storage::from_env();
    let (shims, shim_directory, [node, yarn]) = match &storage {
        Ok(storage) => {
            let shims = storage.root().join("shims");
            report.detail(format_args!("Storage: {}", display_path(storage.root())))?;
            report.detail(format_args!("Shims: {}", display_path(&shims)))?;
            let directory = report_shim_directory(&mut report, &shims, path)?;
            let selections =
                report_selections(&mut report, storage, directory == ShimDirectory::OnPath)?;
            (Some(shims), directory, selections)
        }
        Err(error) => {
            report.detail(format_args!("Storage: unavailable\nShims: unavailable"))?;
            report.error(format_args!(
                "Cannot locate storage ({error}). Set the storage environment variable to an absolute directory."
            ))?;
            report_unavailable_selections(&mut report)?;
            (None, ShimDirectory::Unavailable, [None, None])
        }
    };

    report.detail(format_args!(
        "\nPATH matches (static lookup, excluding shell aliases, functions and caches):"
    ))?;
    let mut missing_shims = Vec::new();
    let mut off_path_shims = Vec::new();
    let mut shadowed = false;
    for (name, tool) in shim::COMMANDS {
        let found = paths::first_match(path, name);
        match &found {
            Ok(Some(found)) => report.detail(format_args!("{name}: {}", display_path(found)))?,
            Ok(None) => report.detail(format_args!("{name}: not found"))?,
            Err(error) => report.error(format_args!(
                "{name}: cannot check PATH ({error}). Check access to the reported directory."
            ))?,
        }
        let required = match tool {
            Some(Tool::Node) => node.is_some(),
            Some(Tool::Yarn) => yarn.is_some(),
            None => false,
        };
        if required && let Some(shims) = &shims {
            match report_required_shim(
                &mut report,
                shims,
                name,
                &found,
                shim_directory == ShimDirectory::OnPath,
            )? {
                Some(ShimFinding::Missing) => missing_shims.push(name),
                Some(ShimFinding::NotOnPath) => off_path_shims.push(name),
                Some(ShimFinding::Shadowed) => shadowed = true,
                None => {}
            }
        }
    }
    if !missing_shims.is_empty() {
        report.error(format_args!(
            "Missing or unusable shims: {}.",
            missing_shims.join(", ")
        ))?;
        if shim_directory != ShimDirectory::Missing {
            report.line(format_args!(
                "Run `verslot setup` from the installed Verslot executable.\nIf setup rejects the existing shim directory, move it aside and retry."
            ))?;
        }
    }
    if !off_path_shims.is_empty() {
        report.error(format_args!(
            "Shims not found on PATH: {}.",
            off_path_shims.join(", ")
        ))?;
    }
    if (shadowed || !off_path_shims.is_empty())
        && let Some(shims) = &shims
    {
        report.line(format_args!(
            "Put {} first on PATH{}.",
            display_path(shims),
            if cfg!(windows) {
                " and check the extension order in PATHEXT"
            } else {
                ""
            }
        ))?;
    }
    if let (Ok(storage), Some(version)) = (&storage, node) {
        report_bundled_scripts(&mut report, storage, version)?;
    }
    if yarn.is_some()
        && let Some(shims) = &shims
    {
        report_yarn_node(&mut report, path, shims, node.is_some())?;
    }
    if !verbose && !report.has_findings {
        report.line(format_args!("No issues found."))?;
    }
    report.output.flush()?;
    Ok(report.failed)
}

fn report_shim_directory(
    report: &mut Report<'_>,
    shims: &Path,
    path: Option<&OsStr>,
) -> io::Result<ShimDirectory> {
    match fs::metadata(shims) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => {
            report.error(format_args!(
                "Expected a shim directory at {}. Inspect the existing entry before running `verslot setup`.",
                display_path(shims)
            ))?;
            return Ok(ShimDirectory::Unavailable);
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            report.finding(format_args!(
                "Verslot is not configured yet.\nRun `verslot setup` and follow its PATH instructions."
            ))?;
            return Ok(ShimDirectory::Missing);
        }
        Err(error) => {
            report.error(format_args!(
                "Cannot check the shim directory ({error}). Check access to {}.",
                display_path(shims)
            ))?;
            return Ok(ShimDirectory::Unavailable);
        }
    }
    match paths::contains_directory(path, shims) {
        Ok(true) => {
            report.detail(format_args!("Shim directory is on PATH."))?;
            return Ok(ShimDirectory::OnPath);
        }
        Ok(false) => {
            let instructions = if cfg!(windows) {
                "Add the folder below to your user Path, then open a new terminal."
            } else {
                "Add the folder below to PATH in your shell's startup file, then open a new terminal."
            };
            report.error(format_args!(
                "Verslot's shims are missing from this terminal's PATH.\n{instructions}\n{}",
                display_path(shims)
            ))?;
        }
        Err(error) => report.error(format_args!(
            "Cannot check the shim directory on PATH ({error}). Check access to {} and the directories on PATH.",
            display_path(shims)
        ))?,
    }
    Ok(ShimDirectory::Unavailable)
}

fn report_selections(
    report: &mut Report<'_>,
    storage: &Storage,
    shims_on_path: bool,
) -> io::Result<[Option<Version>; 2]> {
    let selections = match storage.inspect_selections() {
        Ok(selections) => selections,
        Err(error) => {
            report.error(format_args!(
                "Cannot check selections ({error}). Inspect storage and retry `verslot doctor`."
            ))?;
            report_unavailable_selections(report)?;
            return Ok([None, None]);
        }
    };
    if shims_on_path && selections.iter().all(|state| matches!(state, Ok(None))) {
        report.finding(format_args!(concat!(
            "No tools selected.\n",
            "Run `verslot use <tool>@<version>` to select an installed version.\n",
            "Run `verslot install <tool>@<version>` first if needed."
        )))?;
    }
    report.detail(format_args!("Selections:"))?;
    let mut versions = [None; 2];
    for ((tool, state), version) in Tool::ALL.into_iter().zip(selections).zip(&mut versions) {
        match state {
            Ok(Some(selected)) => {
                *version = Some(selected);
                report.detail(format_args!("{tool}: selected {tool}@{selected}"))?;
            }
            Ok(None) => report.detail(format_args!("{tool}: unselected"))?,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::InvalidInput
                        | io::ErrorKind::InvalidData
                        | io::ErrorKind::NotFound
                        | io::ErrorKind::NotADirectory
                        | io::ErrorKind::Other
                ) =>
            {
                report.error(format_args!(
                    "{tool}: invalid selection ({error}). Try `verslot unuse {tool}`, then select a complete installation with `verslot use {tool}@<version>`."
                ))?;
            }
            Err(error) => report.error(format_args!(
                "{tool}: cannot check selection ({error}). Check read access to storage."
            ))?,
        }
    }
    Ok(versions)
}

fn report_unavailable_selections(report: &mut Report<'_>) -> io::Result<()> {
    report.detail(format_args!("Selections:"))?;
    for tool in Tool::ALL {
        report.detail(format_args!("{tool}: cannot check selection"))?;
    }
    Ok(())
}

fn report_bundled_scripts(
    report: &mut Report<'_>,
    storage: &Storage,
    version: Version,
) -> io::Result<()> {
    let target = Target {
        tool: Tool::Node,
        version,
    };
    let (root, installation) = match fs::canonicalize(storage.root()).and_then(|root| {
        let installation = storage.installation_path(&target)?.canonicalize()?;
        Ok((root, installation))
    }) {
        Ok(paths) => paths,
        Err(error) => {
            return report.error(format_args!(
                "Cannot check bundled commands for {target} ({error}). Inspect the installation and retry."
            ));
        }
    };
    for name in ["npm", "npx"] {
        let script = shim::bundled_node_script(&installation, name);
        match regular_file(&root, &script) {
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound
                        | io::ErrorKind::NotADirectory
                        | io::ErrorKind::InvalidData
                ) =>
            {
                report.finding(format_args!(
                    "Info: bundled {name} unavailable for {target}."
                ))?;
                report.detail(format_args!("{} ({error})", display_path(&script)))?;
            }
            Err(error) => report.error(format_args!(
                "Cannot check bundled {name} for {target} ({error}). Check access to {}.",
                display_path(&script)
            ))?,
        }
    }
    Ok(())
}

fn report_required_shim(
    report: &mut Report<'_>,
    shims: &Path,
    name: &str,
    found: &io::Result<Option<PathBuf>>,
    shims_on_path: bool,
) -> io::Result<Option<ShimFinding>> {
    let expected = shims.join(shim::executable_name(name));
    let location = display_path(&expected);
    match paths::is_executable_file(&expected) {
        Ok(true) => {}
        Ok(false) => {
            report.detail(format_args!(
                "{name}: missing or unusable shim at {location}."
            ))?;
            return Ok(Some(ShimFinding::Missing));
        }
        Err(error) => {
            report.error(format_args!(
                "{name}: cannot check shim ({error}). Check access to {location}."
            ))?;
            return Ok(None);
        }
    }
    if !shims_on_path {
        return Ok(None);
    }
    let found = match found {
        Ok(Some(found)) => found,
        Ok(None) => return Ok(Some(ShimFinding::NotOnPath)),
        Err(_) => return Ok(None),
    };
    match paths::matches_shim(found, shims, name) {
        Ok(true) => Ok(None),
        Ok(false) => {
            report.error(format_args!("{name}: shadowed by {}.", display_path(found)))?;
            report.detail(format_args!("Expected {location}."))?;
            Ok(Some(ShimFinding::Shadowed))
        }
        Err(error) => {
            report.error(format_args!(
                "{name}: cannot compare the PATH match with {location} ({error}). Check directory access."
            ))?;
            Ok(None)
        }
    }
}

fn report_yarn_node(
    report: &mut Report<'_>,
    path: Option<&OsStr>,
    shims: &Path,
    node_selected: bool,
) -> io::Result<()> {
    let node = match paths::yarn_node(path) {
        Ok(Some(node)) => node,
        Ok(None) => {
            return report.error(format_args!(
                "Yarn Node.js: no Node.js found on PATH. Add a Node.js directory to PATH, or select Node.js and run `verslot setup`."
            ));
        }
        Err(error) => {
            return report.error(format_args!(
                "Yarn Node.js: cannot check PATH ({error}). Check PATH and directory access."
            ));
        }
    };
    let location = display_path(&node);
    match paths::matches_shim(&node, shims, "node") {
        Ok(true) if !node_selected => {
            report.detail(format_args!("Yarn Node.js: {location} (Verslot shim)"))?;
            report.error(format_args!(
                "Yarn found a Node.js shim without a valid selection.\nRun `verslot use node@<version>` or put a system Node.js directory first on PATH."
            ))
        }
        Ok(true) => report.detail(format_args!("Yarn Node.js: {location} (Verslot shim)")),
        Ok(false) => report.detail(format_args!("Yarn Node.js: {location} (external Node.js)")),
        Err(error) => report.error(format_args!(
            "Yarn Node.js: cannot identify {location} ({error}). Check directory access."
        )),
    }
}
