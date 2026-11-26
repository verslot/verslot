use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use serde_json::{Value, json};

use crate::installation::{real_directory, regular_file};
use crate::mutation::{
    StorageLock, acquire_exclusive_lock, allocate_operation, is_link, remove_operation,
};
use crate::shim::executable_name;
use crate::{download, logger, output, setup, storage::Storage};

mod release;

const MARKER: &str = ".verslot-self-update";
const MARKER_CONTENT: &str = "verslot-self-update-v1\n";
const MANUAL_UPGRADE: &str = "self update requires an intact Verslot-owned shims installation; run its installed `verslot self update`, or upgrade manually or through your package manager. External downloaded copies are preserved";
const HANDOFF: &[u8] = b"verslot-self-update-v1\n";

pub(crate) fn update() -> Result<(), String> {
    update_installed().map_err(|error| format!("self update: {error}"))
}

fn update_installed() -> Result<(), String> {
    let storage = Storage::from_env().map_err(|error| error.to_string())?;
    let root = fs::canonicalize(storage.root()).map_err(|_| MANUAL_UPGRADE.to_owned())?;
    let source = fs::canonicalize(std::env::current_exe().map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    // Reject links and external copies before creating locks or downloading anything.
    owned_receipt(&root, &source).map_err(|error| format!("{MANUAL_UPGRADE}: {error}"))?;
    let lock = acquire_exclusive_lock(&root).map_err(|error| error.to_string())?;
    let receipt = owned_receipt(&root, &source).map_err(|error| error.to_string())?;
    let target = release::current_target()?;
    let current = env!("CARGO_PKG_VERSION");
    let agent = download::download_agent();
    let release = match release::latest(&agent, current, target)? {
        release::Selection::Current(latest) => {
            output::print_result(&format!(
                "current v{current}; latest stable v{latest}; no update needed (no automatic downgrade)"
            ));
            return Ok(());
        }
        release::Selection::Update(release) => release,
    };
    logger::print_stdout(
        logger::Symbol::None,
        &format!("Verslot v{current} -> v{}\n", release.version),
    );
    if let Some(warning) = cleanup_pending(&root).map_err(|error| error.to_string())? {
        print_warning(&warning);
    }
    let operation = allocate_operation(&root).map_err(|error| error.to_string())?;
    let preparation = (|| {
        write_new(&operation.join(MARKER), MARKER_CONTENT.as_bytes())?;
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(operation.join("active.lock"))?;
        lease.try_lock_shared().map_err(io::Error::other)?;
        release::prepare(&agent, &release, &root, &operation).map_err(io::Error::other)?;
        let executable = candidate_executable(&operation);
        let state = json!({
            "format": 1,
            "current": current,
            "target": release.version,
            "platform": target,
            "receipt": receipt,
            "digest": setup::file_digest(&executable)?,
        });
        write_new(&operation.join("state.json"), state.to_string().as_bytes())?;
        Ok::<_, io::Error>((lease, executable))
    })();
    let (lease, executable) = match preparation {
        Ok(prepared) => prepared,
        Err(error) => return Err(preparation_error(&root, &operation, error)),
    };
    let mut command = Command::new(&executable);
    command.arg("__self-update").arg(&operation);
    let status = wait_for_installer(&root, &operation, lock, lease, &mut command)?;
    if !status.success() {
        output::print_error(&format!(
            "self update: installation did not complete ({status}); see the recovery diagnostic above"
        ));
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

fn wait_for_installer(
    root: &Path,
    operation: &Path,
    lock: StorageLock,
    lease: File,
    command: &mut Command,
) -> Result<ExitStatus, String> {
    let mut child = match command.stdin(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(error) => {
            drop(lease);
            return Err(preparation_error(root, operation, error));
        }
    };
    // The original invocation remains only as a status observer. In particular,
    // it must not keep the mutation lock while the new executable refreshes shims.
    drop(lock);
    let signal = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("handoff pipe is missing"))
        .and_then(|mut input| input.write_all(HANDOFF));
    let status = child.wait();
    drop(lease);
    cleanup_after_child(root, operation);
    let status = status.map_err(|error| {
        format!("wait for update: {error}; inspect `verslot --version` before retrying")
    })?;
    if status.success() {
        signal.map_err(|error| {
            format!("cannot hand off update: {error}; existing installation preserved")
        })?;
    }
    Ok(status)
}

fn candidate_executable(operation: &Path) -> PathBuf {
    operation.join("candidate").join(executable_name("verslot"))
}

fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create_new(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn preparation_error(root: &Path, operation: &Path, error: io::Error) -> String {
    match remove_operation(root, operation) {
        Ok(()) => format!("{error}; existing installation preserved"),
        Err(cleanup) => format!(
            "{error}; existing installation preserved; cleanup pending at {}: {cleanup}; rerun `verslot setup`",
            operation.display()
        ),
    }
}

pub(crate) fn owned_receipt(root: &Path, executable: &Path) -> io::Result<String> {
    let directory = root.join("shims");
    if executable != directory.join(executable_name("verslot")) {
        return Err(io::Error::other(MANUAL_UPGRADE));
    }
    regular_file(root, executable)?;
    setup::validate_owned_directory(root, &directory, false)
}

// This mode is deliberately absent from help and never accepts a download URL.
pub(crate) fn complete(operation: &Path) -> Result<(), String> {
    complete_prepared(operation).map_err(|error| format!("self update: {error}"))
}

fn complete_prepared(operation: &Path) -> Result<(), String> {
    let storage = Storage::from_env().map_err(|error| error.to_string())?;
    let root = fs::canonicalize(storage.root()).map_err(|error| error.to_string())?;
    validate_operation(&root, operation).map_err(|error| error.to_string())?;
    let lease = OpenOptions::new()
        .read(true)
        .write(true)
        .open(operation.join("active.lock"))
        .map_err(|error| error.to_string())?;
    lease.try_lock_shared().map_err(|error| error.to_string())?;
    let state = read_state(&root, operation)?;
    let current = state["current"].as_str().ok_or("missing current version")?;
    let target = state["target"].as_str().ok_or("missing target version")?;
    let executable = candidate_executable(operation);
    if fs::canonicalize(std::env::current_exe().map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?
        != executable
    {
        return Err("internal update must run from its prepared executable".to_owned());
    }
    if target != env!("CARGO_PKG_VERSION")
        || state["platform"].as_str() != Some(release::current_target()?)
        || release::version_parts(target)? <= release::version_parts(current)?
    {
        return Err(
            "prepared release version or platform does not match; existing installation preserved"
                .to_owned(),
        );
    }
    verify_candidate(&root, &executable, &state)?;
    let mut signal = [0; HANDOFF.len()];
    io::stdin()
        .read_exact(&mut signal)
        .map_err(|error| format!("update handoff was cancelled: {error}"))?;
    if signal != HANDOFF {
        return Err("invalid update handoff; existing installation preserved".to_owned());
    }
    let _lock = acquire_exclusive_lock(&root).map_err(|error| error.to_string())?;
    let installed = root.join("shims").join(executable_name("verslot"));
    let receipt = owned_receipt(&root, &installed).map_err(|error| error.to_string())?;
    if state["receipt"].as_str() != Some(receipt.as_str()) {
        return Err(
            "installation changed during update handoff; existing installation preserved"
                .to_owned(),
        );
    }
    verify_candidate(&root, &executable, &state)?;
    let outcome = setup::setup_locked(&root, &executable).map_err(|error| format!(
        "installation failed: {error}; inspect {} and {} before retrying; retained backups must not be deleted until recovery is confirmed",
        root.join("shims").display(), root.join(".shims-previous").display(),
    ))?;
    output::print_result(&format!("updated Verslot v{current} -> v{target}"));
    if let Some(warning) = outcome.cleanup_warning {
        print_warning(&warning);
    }
    Ok(())
}

fn read_state(root: &Path, operation: &Path) -> Result<Value, String> {
    let path = operation.join("state.json");
    regular_file(root, &path).map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|error| error.to_string())?
        .take(8193)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let state: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if bytes.len() > 8192
        || state["format"].as_u64() != Some(1)
        || !["current", "target", "platform", "receipt", "digest"]
            .iter()
            .all(|field| state[field].is_string())
    {
        return Err("invalid prepared update state".to_owned());
    }
    Ok(state)
}

fn verify_candidate(root: &Path, executable: &Path, state: &Value) -> Result<(), String> {
    regular_file(root, executable).map_err(|error| error.to_string())?;
    if state["digest"].as_str()
        != Some(
            setup::file_digest(executable)
                .map_err(|error| error.to_string())?
                .as_str(),
        )
    {
        return Err(
            "prepared executable SHA-256 mismatch; existing installation preserved".to_owned(),
        );
    }
    release::validate_binary(executable, release::current_target()?)
}

fn print_warning(message: &str) {
    logger::print_stderr(logger::Symbol::None, &format!("warning: {message}\n"));
}

fn cleanup_after_child(root: &Path, operation: &Path) {
    let result = acquire_exclusive_lock(root).and_then(|_lock| cleanup_operation(root, operation));
    if let Err(error) = result {
        print_warning(&format!(
            "update cleanup pending at {}: {error}; rerun `verslot setup`",
            operation.display()
        ));
    }
}

pub(crate) fn cleanup_pending(root: &Path) -> io::Result<Option<String>> {
    let temporary = root.join("tmp");
    match fs::symlink_metadata(&temporary) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
        Ok(_) => real_directory(root, &temporary)?,
    }
    let mut warnings = Vec::new();
    for entry in fs::read_dir(&temporary)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if is_link(&metadata) || !metadata.is_dir() {
            continue;
        }
        let operation = entry.path();
        match fs::symlink_metadata(operation.join(MARKER)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
            Ok(_) => {}
        }
        if let Err(error) = cleanup_operation(root, &operation) {
            warnings.push(format!("update cleanup pending at {}: {error}; close related processes and rerun `verslot setup`", operation.display()));
        }
    }
    Ok((!warnings.is_empty()).then(|| warnings.join("\n")))
}

fn validate_operation(root: &Path, operation: &Path) -> io::Result<()> {
    if operation.parent() != Some(root.join("tmp").as_path()) {
        return Err(io::Error::other(
            "update is outside its direct temporary directory",
        ));
    }
    real_directory(root, operation)?;
    let marker = operation.join(MARKER);
    if regular_file(root, &marker)?.len() != MARKER_CONTENT.len() as u64
        || fs::read(marker)? != MARKER_CONTENT.as_bytes()
    {
        return Err(io::Error::other(
            "invalid update ownership marker; preserved files",
        ));
    }
    for entry in fs::read_dir(operation)? {
        let entry = entry?;
        if entry.file_name() == "candidate" {
            real_directory(root, &entry.path())?;
            for candidate in fs::read_dir(entry.path())? {
                let candidate = candidate?;
                if candidate.file_name() != "LICENSE"
                    && candidate.file_name() != executable_name("verslot").as_str()
                {
                    return Err(io::Error::other(
                        "unknown entry in update candidate; preserved files",
                    ));
                }
                regular_file(root, &candidate.path())?;
            }
        } else if [MARKER, "active.lock", "state.json", "release.archive"]
            .iter()
            .any(|name| entry.file_name() == *name)
        {
            regular_file(root, &entry.path())?;
        } else {
            return Err(io::Error::other(
                "unknown entry in update directory; preserved files",
            ));
        }
    }
    Ok(())
}

fn cleanup_operation(root: &Path, operation: &Path) -> io::Result<()> {
    validate_operation(root, operation)?;
    let lease_path = operation.join("active.lock");
    let lease = match OpenOptions::new().read(true).write(true).open(&lease_path) {
        Ok(file) => Some(file),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    if let Some(lease) = &lease {
        match lease.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => return Ok(()),
            Err(std::fs::TryLockError::Error(error)) => return Err(error),
        }
    }
    let candidate = operation.join("candidate");
    if candidate.try_exists()? {
        for entry in fs::read_dir(&candidate)? {
            fs::remove_file(entry?.path())?;
        }
        fs::remove_dir(&candidate)?;
    }
    for name in ["release.archive", "state.json"] {
        match fs::remove_file(operation.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    drop(lease);
    if lease_path.try_exists()? {
        fs::remove_file(lease_path)?;
    }
    fs::remove_file(operation.join(MARKER))?;
    fs::remove_dir(operation)
}

#[cfg(test)]
mod tests;
