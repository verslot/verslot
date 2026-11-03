use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant};

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

use crate::logger;
use crate::target::Target;

const DOT_TICKS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", " "];
const RESOLUTION_DELAY: Duration = Duration::from_millis(200);

pub(crate) enum Progress<'a> {
    Silent,
    Stderr,
    Plain {
        writer: Option<Box<dyn Write + 'a>>,
        last_update: Option<(u64, Instant)>,
        total: Option<u64>,
        target: Option<Target>,
    },
    Terminal(ProgressBar),
}

impl<'a> Progress<'a> {
    fn spinner_style() -> ProgressStyle {
        let template = if logger::color_enabled(true) {
            format!("{{spinner:<{}.cyan}} {{msg}}", logger::SYMBOL_WIDTH)
        } else {
            format!("{{spinner:<{}}} {{msg}}", logger::SYMBOL_WIDTH)
        };
        ProgressStyle::with_template(&template)
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
            target: None,
        }
    }

    pub(crate) fn delayed_stage<T>(&mut self, message: &str, operation: impl FnOnce() -> T) -> T {
        if matches!(self, Self::Stderr) {
            *self = Self::stderr_output();
        }
        let Self::Terminal(bar) = self else {
            self.stage(message);
            return operation();
        };
        std::thread::scope(|scope| {
            let (cancel, cancelled) = std::sync::mpsc::channel::<()>();
            let timer = std::thread::Builder::new().spawn_scoped(scope, move || {
                if matches!(
                    cancelled.recv_timeout(RESOLUTION_DELAY),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                ) {
                    Self::terminal_stage(bar, message);
                }
            });
            let result = operation();
            drop(cancel);
            if let Ok(timer) = timer {
                // Join before the next stage can draw, so a late timer cannot replace it.
                let _ = timer.join();
            }
            result
        })
    }

    pub(crate) fn set_target(&mut self, target: &Target) {
        if matches!(self, Self::Stderr) {
            *self = Self::stderr_output();
        }
        match self {
            Self::Plain {
                target: current, ..
            } => *current = Some(*target),
            Self::Terminal(bar) => bar.set_prefix(target.to_string()),
            Self::Silent | Self::Stderr => {}
        }
    }

    pub(crate) fn stage(&mut self, message: &str) {
        if matches!(self, Self::Stderr) {
            *self = Self::stderr_output();
        }
        match self {
            Self::Silent | Self::Stderr => {}
            Self::Plain {
                writer,
                last_update,
                total,
                target,
            } => {
                let message = match target {
                    Some(target) => format!("{message} {target}"),
                    None => message.to_owned(),
                };
                Self::write_plain(writer, &message);
                *last_update = None;
                *total = None;
            }
            Self::Terminal(bar) => Self::terminal_stage(bar, message),
        }
    }

    fn terminal_stage(bar: &ProgressBar, message: &str) {
        bar.set_style(Self::spinner_style());
        bar.set_message(Self::terminal_message(bar, message));
        bar.unset_length();
        bar.force_draw();
        bar.enable_steady_tick(Duration::from_millis(100));
    }

    fn terminal_message(bar: &ProgressBar, message: &str) -> String {
        let target = bar.prefix();
        if target.is_empty() {
            message.to_owned()
        } else {
            format!("{message} {target}")
        }
    }

    pub(crate) fn set_download_total(&mut self, total: Option<u64>) {
        let total = total.filter(|total| *total > 0);
        match self {
            Self::Plain { total: current, .. } => *current = total,
            Self::Terminal(bar) => {
                if let Some(total) = total {
                    bar.set_length(total);
                    let label = if bar.prefix().is_empty() {
                        "Downloading"
                    } else {
                        "Downloading {prefix}"
                    };
                    let template = if logger::color_enabled(true) {
                        format!("{label}  {{bar:24.cyan}}  {{msg}}")
                    } else {
                        format!("{label}  {{bar:24}}  {{msg}}")
                    };
                    let template = logger::format_lines(logger::Symbol::None, &template, false);
                    if let Ok(style) = ProgressStyle::with_template(&template) {
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
                ..
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
                    let label = Self::terminal_message(bar, "Downloading");
                    bar.set_message(format!("{label} {bytes} bytes"));
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
            && logger::write_plain_line(output, message)
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
    struct RecordingTerminal(Arc<Mutex<String>>, Option<std::sync::mpsc::Sender<Instant>>);

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
            if let Some(sender) = &self.1 {
                let _ = sender.send(Instant::now());
            }
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
    fn fast_resolution_does_not_draw_or_replace_the_following_stage() {
        for failed in [false, true] {
            let output = Arc::new(Mutex::new(String::new()));
            let bar = ProgressBar::with_draw_target(
                None,
                ProgressDrawTarget::term_like(Box::new(RecordingTerminal(output.clone(), None))),
            );
            let mut progress = Progress::Terminal(bar.clone());
            let result = progress.delayed_stage("Resolving node@18", || {
                if failed { Err("unavailable") } else { Ok(42) }
            });
            assert_eq!(result.is_err(), failed);
            if !failed {
                progress.set_target(&"node@18.20.8".parse().unwrap());
                progress.stage("Installing");
                assert_eq!(bar.message(), "Installing node@18.20.8");
            }
            progress.finish();
            assert!(bar.is_finished());
            assert!(!output.lock().unwrap().contains("Resolving"));
        }
    }

    #[test]
    fn slow_resolution_draws_after_the_delay_and_cleans_up_before_the_next_stage() {
        let output = Arc::new(Mutex::new(String::new()));
        let (drawn, observed) = std::sync::mpsc::channel();
        let bar = ProgressBar::with_draw_target(
            None,
            ProgressDrawTarget::term_like(Box::new(RecordingTerminal(output.clone(), Some(drawn)))),
        );
        let mut progress = Progress::Terminal(bar.clone());
        let started = Instant::now();
        progress.delayed_stage("Resolving node@18", || {
            loop {
                let time = observed.recv_timeout(Duration::from_secs(5)).unwrap();
                if output.lock().unwrap().contains("Resolving node@18") {
                    assert!(time.duration_since(started) >= Duration::from_millis(200));
                    break;
                }
            }
        });
        assert_eq!(bar.message(), "Resolving node@18");
        progress.set_target(&"node@18.20.8".parse().unwrap());
        progress.stage("Downloading");
        assert_eq!(bar.message(), "Downloading node@18.20.8");
        progress.finish();
        assert!(bar.is_finished());
    }

    #[test]
    fn redirected_resolution_is_immediate_and_installation_stages_name_the_target() {
        struct RecordingWriter(Arc<Mutex<Vec<u8>>>);
        impl Write for RecordingWriter {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let output = Arc::new(Mutex::new(Vec::new()));
        let mut progress = Progress::plain(RecordingWriter(output.clone()));
        let result = progress.delayed_stage("Resolving node@18", || {
            assert_eq!(*output.lock().unwrap(), b"   Resolving node@18\n");
            42
        });
        assert_eq!(result, 42);
        progress.set_target(&"node@18.20.8".parse().unwrap());
        progress.stage("Downloading");
        progress.set_download_total(Some(100));
        progress.downloaded(50, true);
        progress.stage("Installing");
        progress.finish();
        assert_eq!(*output.lock().unwrap(), b"   Resolving node@18\n   Downloading node@18.20.8\n   Downloaded 50/100 bytes (50%)\n   Installing node@18.20.8\n");
    }

    #[test]
    fn terminal_download_keeps_the_target_with_known_unknown_and_exceeded_totals() {
        for total in [Some(100), None] {
            let output = Arc::new(Mutex::new(String::new()));
            let bar = ProgressBar::with_draw_target(
                None,
                ProgressDrawTarget::term_like(Box::new(RecordingTerminal(output.clone(), None))),
            );
            let mut progress = Progress::Terminal(bar.clone());
            progress.set_target(&"node@18.20.8".parse().unwrap());
            progress.stage("Downloading");
            progress.set_download_total(total);
            output.lock().unwrap().clear();
            progress.downloaded(50, true);
            let first = output.lock().unwrap().clone();
            assert!(first.contains("Downloading node@18.20.8"), "{first}");
            assert!(first.contains("50"), "{first}");
            output.lock().unwrap().clear();
            progress.downloaded(101, true);
            assert_eq!(bar.message(), "Downloading node@18.20.8 101 bytes");
            assert!(output.lock().unwrap().contains("Downloading node@18.20.8"));
            assert_eq!(bar.length(), None);
            progress.stage("Installing");
            assert_eq!(bar.message(), "Installing node@18.20.8");
            progress.finish();
        }
    }

    #[test]
    fn terminal_colors_respect_output_and_environment() {
        const CASE_VARIABLE: &str = "VERSLOT_TEST_PROGRESS_COLOR";
        if let Ok(expected) = std::env::var(CASE_VARIABLE) {
            let expected = expected == "true";
            assert!(!logger::color_enabled(false));
            assert_eq!(logger::color_enabled(true), expected);
            let output = Arc::new(Mutex::new(String::new()));
            let bar = ProgressBar::with_draw_target(
                None,
                ProgressDrawTarget::term_like(Box::new(RecordingTerminal(output.clone(), None))),
            );
            let mut progress = Progress::Terminal(bar);
            progress.stage("Fetching checksums");
            progress.set_download_total(Some(4096));
            progress.downloaded(2048, true);
            progress.downloaded(4097, true);
            progress.finish();
            let output = output.lock().unwrap();
            let plain = output.replace("\x1b[36m", "").replace("\x1b[0m", "");
            assert!(
                DOT_TICKS[..DOT_TICKS.len() - 1]
                    .iter()
                    .any(|dot| plain.contains(&format!("{dot}  Fetching checksums"))),
                "{plain}"
            );
            assert!(plain.contains("   Downloading"), "{plain}");
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
        assert_eq!(output, b"   Downloading archive...\n   Downloaded 0 bytes\n   Downloaded 2 bytes\n   Downloaded 4 bytes\n   Downloaded 5 bytes\n   Verifying\n");
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
        assert_eq!(output, b"   Downloading archive...\n   Downloaded 0/4 bytes (0%)\n   Downloaded 2/4 bytes (50%)\n   Downloaded 4/4 bytes (100%)\n   Downloaded 5 bytes\n   Downloaded 6 bytes\n   Verifying\n");
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
        assert_eq!(output, b"   Downloaded 999/1000 bytes (99%)\n   Downloaded 18446744073709551614/18446744073709551615 bytes (99%)\n");
    }

    #[test]
    #[ignore = "manual terminal and redirected-output smoke; includes deliberate pauses"]
    fn stage_output_smoke() {
        let mut progress = Progress::stderr();
        progress.delayed_stage("Resolving node@18", || {
            std::thread::sleep(Duration::from_millis(350));
        });
        progress.set_target(&"node@18.20.8".parse().unwrap());
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
