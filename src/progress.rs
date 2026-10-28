use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant};

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

const DOT_TICKS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", " "];

pub(crate) enum Progress<'a> {
    Silent,
    Stderr,
    Plain {
        writer: Option<Box<dyn Write + 'a>>,
        last_update: Option<(u64, Instant)>,
        total: Option<u64>,
    },
    Terminal(ProgressBar),
}

impl<'a> Progress<'a> {
    fn spinner_style() -> ProgressStyle {
        let template = if crate::output::color_enabled(true) {
            "{spinner:.cyan} {msg}"
        } else {
            "{spinner} {msg}"
        };
        ProgressStyle::with_template(template)
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_strings(DOT_TICKS)
    }

    pub(crate) fn stderr() -> Self {
        Self::Stderr
    }

    fn stderr_output() -> Self {
        if io::stderr().is_terminal() {
            let bar = ProgressBar::with_draw_target(None, ProgressDrawTarget::stderr_with_hz(10));
            if !bar.is_hidden() {
                bar.set_style(Self::spinner_style());
                return Self::Terminal(bar);
            }
        }
        Self::plain(io::stderr())
    }

    pub(crate) fn plain(writer: impl Write + 'a) -> Self {
        Self::Plain {
            writer: Some(Box::new(writer)),
            last_update: None,
            total: None,
        }
    }

    pub(crate) fn stage(&mut self, message: &'static str) {
        if matches!(self, Self::Stderr) {
            *self = Self::stderr_output();
        }
        match self {
            Self::Silent | Self::Stderr => {}
            Self::Plain {
                writer,
                last_update,
                total,
            } => {
                Self::write_plain(writer, message);
                *last_update = None;
                *total = None;
            }
            Self::Terminal(bar) => {
                bar.set_style(Self::spinner_style());
                bar.set_message(message);
                bar.unset_length();
                bar.force_draw();
                bar.enable_steady_tick(Duration::from_millis(100));
            }
        }
    }

    pub(crate) fn set_download_total(&mut self, total: Option<u64>) {
        let total = total.filter(|total| *total > 0);
        match self {
            Self::Plain { total: current, .. } => *current = total,
            Self::Terminal(bar) => {
                if let Some(total) = total {
                    bar.set_length(total);
                    let template = if crate::output::color_enabled(true) {
                        "Downloading  {bar:24.cyan}  {msg}"
                    } else {
                        "Downloading  {bar:24}  {msg}"
                    };
                    if let Ok(style) = ProgressStyle::with_template(template) {
                        bar.set_style(style.progress_chars("██░"));
                    }
                } else {
                    bar.unset_length();
                    bar.set_style(Self::spinner_style());
                }
            }
            Self::Silent | Self::Stderr => {}
        }
    }

    pub(crate) fn downloaded(&mut self, bytes: u64, force: bool) {
        self.downloaded_at(bytes, force, Instant::now());
    }

    fn downloaded_at(&mut self, bytes: u64, force: bool, now: Instant) {
        match self {
            Self::Silent | Self::Stderr => {}
            Self::Plain {
                writer,
                last_update,
                total,
            } => {
                if total.is_some_and(|total| bytes > total) {
                    *total = None;
                }
                if force
                    || last_update.is_none_or(|(previous, time)| {
                        bytes != previous && now.duration_since(time) >= Duration::from_secs(1)
                    })
                {
                    let amount = Self::byte_amount(bytes, *total);
                    Self::write_plain(writer, &format!("Downloaded {amount}"));
                    // The initial snapshot does not delay the first data-bearing update.
                    if bytes != 0 {
                        *last_update = Some((bytes, now));
                    }
                }
            }
            Self::Terminal(bar) => {
                let total = bar.length().filter(|total| bytes <= *total);
                if total != bar.length() {
                    bar.set_style(Self::spinner_style());
                }
                if let Some(total) = total {
                    let percentage = u128::from(bytes) * 100 / u128::from(total);
                    bar.set_message(format!("{percentage}%  {bytes}/{total} bytes"));
                } else {
                    bar.set_message(format!("Downloading {bytes} bytes"));
                }
                if total.is_none() {
                    bar.unset_length();
                }
                bar.set_position(bytes);
                if force {
                    bar.force_draw();
                }
            }
        }
    }

    fn byte_amount(bytes: u64, total: Option<u64>) -> String {
        match total {
            Some(total) => {
                let percentage = u128::from(bytes) * 100 / u128::from(total);
                format!("{bytes}/{total} bytes ({percentage}%)")
            }
            None => format!("{bytes} bytes"),
        }
    }

    fn write_plain(writer: &mut Option<Box<dyn Write + 'a>>, message: &str) {
        if let Some(output) = writer
            && writeln!(output, "{message}")
                .and_then(|()| output.flush())
                .is_err()
        {
            *writer = None;
        }
    }

    pub(crate) fn finish(&mut self) {
        if let Self::Terminal(bar) = self {
            bar.disable_steady_tick();
            bar.finish_and_clear();
        }
        *self = Self::Silent;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Debug)]
    struct RecordingTerminal(Arc<Mutex<String>>);

    impl indicatif::TermLike for RecordingTerminal {
        fn width(&self) -> u16 {
            120
        }

        fn move_cursor_up(&self, _n: usize) -> io::Result<()> {
            Ok(())
        }

        fn move_cursor_down(&self, _n: usize) -> io::Result<()> {
            Ok(())
        }

        fn move_cursor_right(&self, _n: usize) -> io::Result<()> {
            Ok(())
        }

        fn move_cursor_left(&self, _n: usize) -> io::Result<()> {
            Ok(())
        }

        fn write_line(&self, line: &str) -> io::Result<()> {
            self.write_str(&format!("{line}\n"))
        }

        fn write_str(&self, text: &str) -> io::Result<()> {
            self.0.lock().unwrap().push_str(text);
            Ok(())
        }

        fn clear_line(&self) -> io::Result<()> {
            Ok(())
        }

        fn flush(&self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn terminal_colors_respect_output_and_environment() {
        const CASE_VARIABLE: &str = "VERSLOT_TEST_PROGRESS_COLOR";
        if let Ok(expected) = std::env::var(CASE_VARIABLE) {
            let expected = expected == "true";
            assert!(!crate::output::color_enabled(false));
            assert_eq!(crate::output::color_enabled(true), expected);
            let output = Arc::new(Mutex::new(String::new()));
            let bar = ProgressBar::with_draw_target(
                None,
                ProgressDrawTarget::term_like(Box::new(RecordingTerminal(output.clone()))),
            );
            let mut progress = Progress::Terminal(bar);
            progress.stage("Fetching checksums");
            progress.set_download_total(Some(4096));
            progress.downloaded(2048, true);
            progress.downloaded(4097, true);
            progress.finish();
            let output = output.lock().unwrap();
            for text in [
                "Fetching checksums",
                "50%",
                "2048/4096 bytes",
                "Downloading 4097 bytes",
            ] {
                assert!(output.contains(text), "{output}");
            }
            assert_eq!(output.contains("\x1b["), expected, "{output}");
            return;
        }

        for (no_color, term, expected) in [
            (None, None, true),
            (Some(""), Some("xterm-256color"), true),
            (Some("1"), None, false),
            (None, Some("dumb"), false),
            (Some(""), Some("dumb"), false),
        ] {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "progress::tests::terminal_colors_respect_output_and_environment",
                    "--nocapture",
                ])
                .env(CASE_VARIABLE, expected.to_string())
                .env("CLICOLOR_FORCE", "1")
                .env_remove("NO_COLOR")
                .env_remove("TERM");
            if let Some(no_color) = no_color {
                command.env("NO_COLOR", no_color);
            }
            if let Some(term) = term {
                command.env("TERM", term);
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "NO_COLOR={no_color:?} TERM={term:?}: {output:?}"
            );
        }
    }

    struct FailingOutput {
        fail_flush: bool,
        failed: bool,
    }

    impl Write for FailingOutput {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            assert!(!self.failed, "progress wrote again after output failure");
            if self.fail_flush {
                Ok(bytes.len())
            } else {
                self.failed = true;
                Err(io::Error::other("injected write failure"))
            }
        }

        fn flush(&mut self) -> io::Result<()> {
            assert!(!self.failed, "progress flushed again after output failure");
            self.failed = true;
            Err(io::Error::other("injected flush failure"))
        }
    }

    #[test]
    fn plain_progress_stops_after_write_or_flush_failure() {
        for fail_flush in [false, true] {
            let mut output = FailingOutput {
                fail_flush,
                failed: false,
            };
            let mut progress = Progress::plain(&mut output);
            progress.stage("Fetching checksums");
            progress.stage("Downloading archive...");
            progress.finish();
            drop(progress);
            assert!(output.failed);
        }
    }

    #[test]
    fn byte_progress_throttles_intermediate_updates_but_not_snapshots_or_stages() {
        let mut output = Vec::new();
        let mut progress = Progress::plain(&mut output);
        let start = Instant::now();
        progress.stage("Downloading archive...");
        progress.downloaded_at(0, true, start);
        progress.downloaded_at(2, false, start);
        progress.downloaded_at(3, false, start + Duration::from_millis(999));
        progress.downloaded_at(4, false, start + Duration::from_secs(1));
        progress.downloaded_at(4, false, start + Duration::from_secs(2));
        progress.downloaded_at(5, true, start + Duration::from_secs(2));
        progress.stage("Verifying");
        progress.finish();
        drop(progress);
        assert_eq!(output, b"Downloading archive...\nDownloaded 0 bytes\nDownloaded 2 bytes\nDownloaded 4 bytes\nDownloaded 5 bytes\nVerifying\n");
    }

    #[test]
    fn byte_progress_failure_disables_later_bytes_and_stages() {
        for fail_flush in [false, true] {
            let mut output = FailingOutput {
                fail_flush,
                failed: false,
            };
            let mut progress = Progress::plain(&mut output);
            progress.downloaded(0, true);
            progress.downloaded(2, false);
            progress.downloaded(3, true);
            progress.stage("Verifying");
            progress.finish();
            drop(progress);
            assert!(output.failed);
        }
    }

    #[test]
    fn progress_drops_inconsistent_totals_and_keeps_snapshot_throttling() {
        let mut output = Vec::new();
        let mut progress = Progress::plain(&mut output);
        let start = Instant::now();
        progress.stage("Downloading archive...");
        progress.set_download_total(Some(4));
        progress.downloaded_at(0, true, start);
        progress.downloaded_at(2, false, start);
        progress.downloaded_at(3, false, start + Duration::from_millis(999));
        progress.downloaded_at(4, false, start + Duration::from_secs(1));
        progress.downloaded_at(5, false, start + Duration::from_secs(2));
        progress.downloaded_at(6, true, start + Duration::from_secs(2));
        progress.stage("Verifying");
        progress.finish();
        drop(progress);
        assert_eq!(output, b"Downloading archive...\nDownloaded 0/4 bytes (0%)\nDownloaded 2/4 bytes (50%)\nDownloaded 4/4 bytes (100%)\nDownloaded 5 bytes\nDownloaded 6 bytes\nVerifying\n");
    }

    #[test]
    fn terminal_download_shows_percentage_and_bytes_and_recovers_from_exceeded_total() {
        let bar = ProgressBar::hidden();
        let mut progress = Progress::Terminal(bar.clone());
        progress.set_download_total(Some(1000));
        progress.downloaded(999, true);
        assert_eq!(bar.message(), "99%  999/1000 bytes");
        assert_eq!(bar.position(), 999);
        assert_eq!(bar.length(), Some(1000));

        progress.downloaded(1001, true);
        assert_eq!(bar.message(), "Downloading 1001 bytes");
        assert_eq!(bar.length(), None);
        progress.finish();
    }

    #[test]
    fn progress_rounds_percentages_down_without_premature_completion() {
        let mut output = Vec::new();
        let mut progress = Progress::plain(&mut output);
        progress.set_download_total(Some(1000));
        progress.downloaded(999, true);
        progress.set_download_total(Some(u64::MAX));
        progress.downloaded(u64::MAX - 1, true);
        progress.finish();
        drop(progress);
        assert_eq!(output, b"Downloaded 999/1000 bytes (99%)\nDownloaded 18446744073709551614/18446744073709551615 bytes (99%)\n");
    }

    #[test]
    #[ignore = "manual terminal and redirected-output smoke; includes deliberate pauses"]
    fn stage_output_smoke() {
        let mut progress = Progress::stderr();
        for stage in [
            "Fetching checksums",
            "Downloading",
            "Verifying",
            "Installing",
        ] {
            progress.stage(stage);
            if stage == "Downloading" {
                progress.downloaded(0, true);
                progress.downloaded(1024, false);
                std::thread::sleep(Duration::from_millis(350));
                progress.downloaded(2048, false);
                std::thread::sleep(Duration::from_millis(350));
                progress.downloaded(2048, true);
                progress.set_download_total(Some(4096));
                progress.downloaded(2048, true);
                std::thread::sleep(Duration::from_millis(350));
                progress.downloaded(4096, true);
                std::thread::sleep(Duration::from_millis(350));
                // Demonstrate immediate return to byte-only display on inconsistency.
                progress.downloaded(4097, true);
            }
            std::thread::sleep(Duration::from_millis(350));
        }
        progress.finish();
        eprintln!("stage output smoke complete");
    }
}
