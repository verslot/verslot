use crate::logger::{self, Symbol};
use crate::target::{Target, Version};

pub(crate) fn print_result(message: &str) {
    let symbol = if message.starts_with("already ") {
        Symbol::None
    } else {
        Symbol::Success
    };
    logger::print_stdout(symbol, &format!("{}\n", format_result(message)));
}

fn format_result(message: &str) -> String {
    let mut message = message.to_owned();
    if let Some(first) = message.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    message
}

pub(crate) fn print_list(installations: &[Target], selected: Option<Version>, terminal: bool) {
    logger::print_stdout(
        Symbol::None,
        &format_list(
            installations,
            selected,
            terminal,
            logger::stdout_color_enabled(),
        ),
    );
}

fn format_list(
    installations: &[Target],
    selected: Option<Version>,
    terminal: bool,
    color: bool,
) -> String {
    if !terminal {
        return installations
            .iter()
            .map(|target| format!("{target}\n"))
            .collect();
    }
    if installations.is_empty() {
        return "No installed versions\n".to_owned();
    }
    let tool_width = installations
        .iter()
        .map(|target| target.tool.to_string().len())
        .max()
        .unwrap_or(0)
        .max("Tool".len());
    let header = format!("{:<tool_width$}  Version", "Tool");
    let mut output = if color {
        format!("\x1b[3;35m{header}\x1b[0m\n")
    } else {
        format!("{header}\n")
    };
    for target in installations {
        let tool = target.tool.to_string();
        let version = target.version;
        if color {
            let version_color = if selected == Some(version) { 32 } else { 90 };
            output.push_str(&format!(
                "\x1b[36m{tool:<tool_width$}\x1b[0m  \x1b[{version_color}m{version}\x1b[0m\n"
            ));
        } else {
            output.push_str(&format!("{tool:<tool_width$}  {version}\n"));
        }
    }
    output
}

pub(crate) fn format_current(selected: Option<Version>, terminal: bool) -> String {
    match selected {
        Some(version) => format!("node@{version}\n"),
        None if terminal => "No version selected\n".to_owned(),
        None => String::new(),
    }
}

pub(crate) fn print_error(error: &str) {
    logger::print_stderr(
        Symbol::Error,
        &format!(
            "{}\n",
            format_diagnostic(error, logger::stderr_color_enabled())
        ),
    );
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
    fn results_capitalize_without_color() {
        for (message, expected) in [
            ("installed node@22.0.0", "Installed node@22.0.0"),
            ("selected node@22.0.0", "Selected node@22.0.0"),
            ("uninstalled node@22.0.0", "Uninstalled node@22.0.0"),
            (
                "already installed node@22.0.0",
                "Already installed node@22.0.0",
            ),
            (
                "already selected node@22.0.0",
                "Already selected node@22.0.0",
            ),
        ] {
            assert_eq!(format_result(message), expected);
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

    #[test]
    fn list_shows_only_tool_and_version_columns() {
        let installations = [
            "node@18.18.0".parse::<Target>().unwrap(),
            "node@24.20.0".parse::<Target>().unwrap(),
        ];
        assert_eq!(
            format_list(&installations, Some(installations[1].version), true, false),
            "Tool  Version\nnode  18.18.0\nnode  24.20.0\n"
        );
    }

    #[test]
    fn list_colors_headers_tools_and_selected_version() {
        let installations = [
            "node@18.18.0".parse::<Target>().unwrap(),
            "node@24.20.0".parse::<Target>().unwrap(),
        ];
        assert_eq!(
            format_list(&installations, Some(installations[1].version), true, true),
            concat!(
                "\x1b[3;35mTool  Version\x1b[0m\n",
                "\x1b[36mnode\x1b[0m  \x1b[90m18.18.0\x1b[0m\n",
                "\x1b[36mnode\x1b[0m  \x1b[32m24.20.0\x1b[0m\n",
            )
        );
    }

    #[test]
    fn list_without_selection_keeps_versions_dim() {
        let target: Target = "node@22.0.0".parse().unwrap();
        assert_eq!(
            format_list(&[target], None, true, true),
            concat!(
                "\x1b[3;35mTool  Version\x1b[0m\n",
                "\x1b[36mnode\x1b[0m  \x1b[90m22.0.0\x1b[0m\n",
            )
        );
    }

    #[test]
    fn redirected_list_keeps_plain_targets_without_headers_or_colors() {
        let installations = [
            "node@18.18.0".parse::<Target>().unwrap(),
            "node@24.20.0".parse::<Target>().unwrap(),
        ];
        assert_eq!(
            format_list(&installations, Some(installations[1].version), false, true),
            "node@18.18.0\nnode@24.20.0\n"
        );
    }

    #[test]
    fn empty_list_preserves_terminal_and_redirected_messages() {
        for color in [false, true] {
            assert_eq!(
                format_list(&[], None, true, color),
                "No installed versions\n"
            );
            assert_eq!(format_list(&[], None, false, color), "");
        }
    }

    #[test]
    fn current_decorates_only_empty_terminal_output() {
        let target: Target = "node@22.0.0".parse().unwrap();
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
