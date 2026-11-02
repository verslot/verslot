"""Regression checks for the native terminal acceptance parser."""

import unittest

import check_terminal_progress as checker


class ReporterOutputTests(unittest.TestCase):
    def output(self, mode):
        target = "node@18.20.8"
        stages = "Resolving node@18\n"
        stages += "\n".join(
            f"{stage} {target}" for stage in ["Fetching checksums", "Downloading", "Verifying"]
        )
        if mode == "pty":
            amounts = (
                f"\x1b[36m⠋\x1b[0m  Downloading {target} 1024 bytes\r"
                f"\x1b[36m⠙\x1b[0m  Downloading {target} 1024 bytes\r"
                f"Downloading {target} {'█' * 13}{'░' * 11} 50%  2048/4096 bytes\r"
                f"Downloading {target} {'█' * 24} 100%  4096/4096 bytes\r"
                f"⠹  Downloading {target} 4097 bytes\r"
            )
            clear = "\x1b[2K"
        else:
            amounts = (
                "Downloaded 1024 bytes\n"
                "Downloaded 2048/4096 bytes (50%)\n"
                "Downloaded 4096/4096 bytes (100%)\n"
                "Downloaded 4097 bytes\n"
            )
            clear = ""
        return f"{stages}\n{amounts}Installing {target}{clear}\nstage output smoke complete\n"

    def check(self, mode, output):
        checker.check_output(checker.TESTS[0], mode, "1 passed; 0 failed", output)

    def test_accepts_target_bearing_terminal_and_redirected_progress(self):
        for mode in ["pty", "redirected"]:
            with self.subTest(mode=mode):
                self.check(mode, self.output(mode))

    def test_rejects_missing_resolution_or_target(self):
        for mode in ["pty", "redirected"]:
            for removed in ["Resolving node@18", " node@18.20.8"]:
                with self.subTest(mode=mode, removed=removed):
                    with self.assertRaises(AssertionError):
                        self.check(mode, self.output(mode).replace(removed, ""))

    def test_rejects_a_spinner_that_does_not_advance_at_the_same_byte_count(self):
        output = self.output("pty").replace("⠙", "⠋")
        with self.assertRaisesRegex(AssertionError, "Spinner did not advance"):
            self.check("pty", output)


if __name__ == "__main__":
    unittest.main()
