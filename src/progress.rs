use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant};

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

pub(crate) enum Progress<'a> {
    Silent,
    Stderr,
    Plain {
        writer: Option<Box<dyn Write + 'a>>,
        last_update: Option<(u64, Instant)>,
    },
    Terminal(ProgressBar),
}

impl<'a> Progress<'a> {
    pub(crate) fn stderr() -> Self {
        Self::Stderr
    }

    fn stderr_output() -> Self {
        if io::stderr().is_terminal() {
            let bar = ProgressBar::with_draw_target(None, ProgressDrawTarget::stderr_with_hz(10));
            if !bar.is_hidden() {
                bar.set_style(
                    ProgressStyle::default_spinner().tick_strings(&["-", "\\", "|", "/", " "]),
                );
                return Self::Terminal(bar);
            }
        }
        Self::plain(io::stderr())
    }

    pub(crate) fn plain(writer: impl Write + 'a) -> Self {
        Self::Plain {
            writer: Some(Box::new(writer)),
            last_update: None,
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
            } => {
                Self::write_plain(writer, message);
                *last_update = None;
            }
            Self::Terminal(bar) => {
                bar.set_message(message);
                bar.force_draw();
                bar.enable_steady_tick(Duration::from_millis(100));
            }
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
            } => {
                if force
                    || last_update.is_none_or(|(previous, time)| {
                        bytes != previous && now.duration_since(time) >= Duration::from_secs(1)
                    })
                {
                    Self::write_plain(writer, &format!("Downloaded {bytes} bytes"));
                    // The initial snapshot does not delay the first data-bearing update.
                    if bytes != 0 {
                        *last_update = Some((bytes, now));
                    }
                }
            }
            Self::Terminal(bar) => {
                bar.set_message(format!("Downloading archive... {bytes} bytes"));
                if force {
                    bar.force_draw();
                }
            }
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
            progress.stage("Fetching checksums...");
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
        progress.stage("Verifying SHA-256...");
        progress.finish();
        drop(progress);
        assert_eq!(output, b"Downloading archive...\nDownloaded 0 bytes\nDownloaded 2 bytes\nDownloaded 4 bytes\nDownloaded 5 bytes\nVerifying SHA-256...\n");
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
            progress.stage("Verifying SHA-256...");
            progress.finish();
            drop(progress);
            assert!(output.failed);
        }
    }

    #[test]
    #[ignore = "manual terminal and redirected-output smoke; includes deliberate pauses"]
    fn stage_output_smoke() {
        let mut progress = Progress::stderr();
        for stage in [
            "Fetching checksums...",
            "Downloading archive...",
            "Verifying SHA-256...",
            "Extracting and installing...",
        ] {
            progress.stage(stage);
            if stage == "Downloading archive..." {
                progress.downloaded(0, true);
                progress.downloaded(1024, false);
                std::thread::sleep(Duration::from_millis(350));
                progress.downloaded(2048, false);
                std::thread::sleep(Duration::from_millis(350));
                progress.downloaded(2048, true);
            }
            std::thread::sleep(Duration::from_millis(350));
        }
        progress.finish();
        eprintln!("stage output smoke complete");
    }
}
