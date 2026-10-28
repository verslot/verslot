use std::io::{self, IsTerminal};

use crate::target::{Target, Version};

pub(crate) fn color_enabled(terminal: bool) -> bool {
    terminal
        && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
        && std::env::var_os("TERM").is_none_or(|value| value != "dumb")
}

pub(crate) fn print_result(message: &str) {
    let color = color_enabled(io::stdout().is_terminal());
    #[cfg(windows)]
    let color = color && enable_windows_ansi(winapi_util::console::Console::stdout());
    println!("{}", format_result(message, color));
}

fn format_result(message: &str, color: bool) -> String {
    let mut message = message.to_owned();
    if let Some(first) = message.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    if color
        && !message.starts_with("Already ")
        && let Some((verb, rest)) = message.split_once(' ')
    {
        return format!("\x1b[32m{verb}\x1b[0m {rest}");
    }
    message
}

#[cfg(windows)]
fn enable_windows_ansi(console: io::Result<winapi_util::console::Console>) -> bool {
    console
        .and_then(|mut console| console.set_virtual_terminal_processing(true))
        .is_ok()
}

pub(crate) fn format_list(
    installations: &[Target],
    selected: Option<Version>,
    terminal: bool,
) -> String {
    if installations.is_empty() && terminal {
        return "No installed versions\n".to_owned();
    }
    installations
        .iter()
        .map(|target| {
            let marker = if terminal && selected == Some(target.version) {
                "  (current)"
            } else {
                ""
            };
            format!("{target}{marker}\n")
        })
        .collect()
}

pub(crate) fn format_current(selected: Option<Version>, terminal: bool) -> String {
    match selected {
        Some(version) => format!("node@{version}\n"),
        None if terminal => "No version selected\n".to_owned(),
        None => String::new(),
    }
}

pub(crate) fn print_error(error: &str) {
    let color = color_enabled(io::stderr().is_terminal());
    #[cfg(windows)]
    let color = color && enable_windows_ansi(winapi_util::console::Console::stderr());
    eprintln!("{}", format_diagnostic(error, color));
}

fn format_diagnostic(error: &str, color: bool) -> String {
    let label = if color {
        "\x1b[31merror:\x1b[0m"
    } else {
        "error:"
    };
    format!("{label} {}", format_error(error))
}

fn format_error(error: &str) -> String {
    let Some((operation, reason)) = error.split_once(": ") else {
        return error.to_owned();
    };
    if let Some(target) = operation.strip_prefix("use ")
        && reason == format!("not installed: {target}")
    {
        return format!("{target} is not installed\n\nRun `verslot install {target}` first.");
    }
    if let Some(target) = operation.strip_prefix("uninstall ")
        && reason == format!("cannot uninstall current version: {target}")
    {
        return format!(
            "{target} is currently selected\n\nSelect another version before uninstalling."
        );
    }
    // Keep committed-operation and cleanup diagnostics intact: they describe actual state.
    if reason.contains("cleanup") {
        return format!("{operation}:\n  {reason}");
    }
    let action = operation
        .strip_prefix("use ")
        .map(|target| format!("select {target}"));
    format!(
        "failed to {}\n  {reason}",
        action.as_deref().unwrap_or(operation)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_capitalize_and_color_only_success_verbs() {
        for verb in ["installed", "selected", "uninstalled"] {
            let message = format!("{verb} node@22.0.0");
            let plain = format_result(&message, false);
            let (verb, rest) = plain.split_once(' ').unwrap();
            assert!(verb.starts_with(char::is_uppercase), "{plain}");
            assert_eq!(
                format_result(&message, true),
                format!("\x1b[32m{verb}\x1b[0m {rest}")
            );
        }
        for message in [
            "already installed node@22.0.0",
            "already selected node@22.0.0",
        ] {
            assert_eq!(format_result(message, true), format_result(message, false));
            assert!(format_result(message, true).starts_with("Already "));
        }
    }

    #[test]
    fn diagnostics_color_only_the_error_label() {
        let error = "install node@22.0.0: permission denied";
        assert_eq!(
            format_diagnostic(error, false),
            "error: failed to install node@22.0.0\n  permission denied"
        );
        assert_eq!(
            format_diagnostic(error, true),
            "\x1b[31merror:\x1b[0m failed to install node@22.0.0\n  permission denied"
        );
    }

    #[cfg(windows)]
    #[test]
    fn unavailable_windows_console_disables_ansi() {
        assert!(!enable_windows_ansi(Err(io::Error::other("no console"))));
    }

    #[test]
    fn queries_decorate_only_terminal_output() {
        let target: Target = "node@22.0.0".parse().unwrap();
        assert_eq!(
            format_list(&[target], Some(target.version), true),
            "node@22.0.0  (current)\n"
        );
        assert_eq!(
            format_list(&[target], Some(target.version), false),
            "node@22.0.0\n"
        );
        assert_eq!(format_list(&[], None, true), "No installed versions\n");
        assert_eq!(format_list(&[], None, false), "");
        assert_eq!(format_current(None, true), "No version selected\n");
        assert_eq!(format_current(None, false), "");
        assert_eq!(format_current(Some(target.version), true), "node@22.0.0\n");
    }

    #[test]
    fn errors_preserve_paths_and_offer_only_specific_recovery() {
        assert_eq!(
            format_error("use node@22.0.0: not installed: node@22.0.0"),
            "node@22.0.0 is not installed\n\nRun `verslot install node@22.0.0` first."
        );
        assert_eq!(
            format_error("uninstall node@22.0.0: cannot uninstall current version: node@22.0.0"),
            "node@22.0.0 is currently selected\n\nSelect another version before uninstalling."
        );
        assert_eq!(
            format_error("install node@22.0.0: open C:\\data: permission denied"),
            "failed to install node@22.0.0\n  open C:\\data: permission denied"
        );
        assert_eq!(
            format_error("install node@22.0.0: installed, but cleanup failed: C:\\tmp"),
            "install node@22.0.0:\n  installed, but cleanup failed: C:\\tmp"
        );
    }
}
