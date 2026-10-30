use std::io::{self, IsTerminal, Write};

pub(crate) const SYMBOL_WIDTH: usize = 2;

#[derive(Clone, Copy)]
pub(crate) enum Symbol {
    None,
    Success,
    Error,
}

pub(crate) fn color_enabled(terminal: bool) -> bool {
    terminal
        && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
        && std::env::var_os("TERM").is_none_or(|value| value != "dumb")
}

#[cfg(windows)]
fn enable_windows_ansi(console: io::Result<winapi_util::console::Console>) -> bool {
    console
        .and_then(|mut console| console.set_virtual_terminal_processing(true))
        .is_ok()
}

pub(crate) fn stdout_color_enabled() -> bool {
    let color = color_enabled(io::stdout().is_terminal());
    #[cfg(windows)]
    let color = color && enable_windows_ansi(winapi_util::console::Console::stdout());
    color
}

pub(crate) fn stderr_color_enabled() -> bool {
    let color = color_enabled(io::stderr().is_terminal());
    #[cfg(windows)]
    let color = color && enable_windows_ansi(winapi_util::console::Console::stderr());
    color
}

fn format_prefix(symbol: Symbol, color: bool) -> String {
    // All symbols used here occupy one terminal column; the empty field occupies two.
    let (glyph, code) = match symbol {
        Symbol::None => ("", 0),
        Symbol::Success => ("✔", 32),
        Symbol::Error => ("✘", 31),
    };
    if color && !glyph.is_empty() {
        format!("\x1b[{code}m{glyph:<SYMBOL_WIDTH$}\x1b[0m ")
    } else {
        format!("{glyph:<SYMBOL_WIDTH$} ")
    }
}

pub(crate) fn format_lines(symbol: Symbol, message: &str, color: bool) -> String {
    let mut output = String::new();
    for (index, line) in message.split_inclusive('\n').enumerate() {
        let symbol = if index == 0 { symbol } else { Symbol::None };
        output.push_str(&format_prefix(symbol, color));
        output.push_str(line);
    }
    output
}

pub(crate) fn print_stdout(symbol: Symbol, message: &str) {
    print!("{}", format_lines(symbol, message, stdout_color_enabled()));
}

pub(crate) fn print_stderr(symbol: Symbol, message: &str) {
    eprint!("{}", format_lines(symbol, message, stderr_color_enabled()));
}

pub(crate) fn write_message(
    writer: &mut (impl Write + ?Sized),
    symbol: Symbol,
    message: &str,
    color: bool,
) -> io::Result<()> {
    writer.write_all(format_lines(symbol, message, color).as_bytes())
}

pub(crate) fn write_plain_line(
    writer: &mut (impl Write + ?Sized),
    message: &str,
) -> io::Result<()> {
    write_message(writer, Symbol::None, &format!("{message}\n"), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_and_empty_fields_reserve_the_same_columns() {
        for (symbol, expected) in [
            (Symbol::None, "   message\n"),
            (Symbol::Success, "✔  message\n"),
            (Symbol::Error, "✘  message\n"),
        ] {
            assert_eq!(format_lines(symbol, "message\n", false), expected);
        }
    }

    #[test]
    fn continuation_and_blank_lines_keep_their_original_content() {
        assert_eq!(
            format_lines(
                Symbol::Error,
                "error: failed\n  cause\n\nRun next.\n",
                false
            ),
            "✘  error: failed\n     cause\n   \n   Run next.\n"
        );
    }

    #[test]
    fn empty_output_stays_empty() {
        assert_eq!(format_lines(Symbol::None, "", false), "");
    }

    #[test]
    fn explicitly_written_blank_lines_reserve_the_empty_symbol_field() {
        let mut output = Vec::new();
        write_plain_line(&mut output, "").unwrap();
        assert_eq!(output, b"   \n");
    }

    #[test]
    fn formatting_preserves_unicode_ansi_and_missing_final_newlines() {
        assert_eq!(
            format_lines(Symbol::None, "\x1b[36mnode\x1b[0m  版本", false),
            "   \x1b[36mnode\x1b[0m  版本"
        );
    }

    #[test]
    fn colors_do_not_change_prefix_padding() {
        assert_eq!(
            format_lines(Symbol::Success, "Installed node@22.0.0\n", true),
            "\x1b[32m✔ \x1b[0m Installed node@22.0.0\n"
        );
        assert_eq!(format_lines(Symbol::None, "plain\n", true), "   plain\n");
    }

    #[cfg(windows)]
    #[test]
    fn unavailable_windows_console_disables_ansi() {
        assert!(!enable_windows_ansi(Err(io::Error::other("no console"))));
    }
}
