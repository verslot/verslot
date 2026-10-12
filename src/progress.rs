use std::io::{self, IsTerminal, Write};
use std::time::Duration;

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

pub(crate) enum Progress<'a> {
    Silent,
    Stderr,
    Plain(Option<Box<dyn Write + 'a>>),
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
        Self::Plain(Some(Box::new(writer)))
    }

    pub(crate) fn stage(&mut self, message: &'static str) {
        if matches!(self, Self::Stderr) {
            *self = Self::stderr_output();
        }
        match self {
            Self::Silent | Self::Stderr => {}
            Self::Plain(writer) => {
                if let Some(output) = writer
                    && writeln!(output, "{message}")
                        .and_then(|()| output.flush())
                        .is_err()
                {
                    *writer = None;
                }
            }
            Self::Terminal(bar) => {
                bar.set_message(message);
                bar.force_draw();
                bar.enable_steady_tick(Duration::from_millis(100));
            }
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
            std::thread::sleep(Duration::from_millis(350));
        }
        progress.finish();
        eprintln!("stage output smoke complete");
    }
}
