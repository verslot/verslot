"""Exercise native Unix stderr PTYs and redirected output; retain raw evidence."""

import errno
import json
import os
from pathlib import Path
import platform
import re
import select
import subprocess
import time


EVIDENCE = Path("target/terminal-acceptance")
DOTS = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"
TESTS = [
    "progress::tests::stage_output_smoke",
    "install::tests::installation_progress_smoke",
]


def build_test_executable():
    result = subprocess.run(
        ["cargo", "test", "--locked", "--lib", "--no-run", "--message-format=json"],
        capture_output=True, text=True, timeout=600,
    )
    (EVIDENCE / "build.stdout").write_text(result.stdout, encoding="utf-8")
    (EVIDENCE / "build.stderr").write_text(result.stderr, encoding="utf-8")
    result.check_returncode()
    executables = [
        event["executable"]
        for line in result.stdout.splitlines()
        if (event := json.loads(line)).get("reason") == "compiler-artifact"
        and event.get("executable") and event["profile"]["test"]
        and event["target"]["kind"] == ["lib"]
    ]
    if len(executables) != 1:
        raise AssertionError(f"Expected one library test executable: {executables}")
    return executables[0]


def run_with_pty(command, stdout, stderr):
    import fcntl
    import pty
    import struct
    import termios

    master, slave = pty.openpty()
    process = None
    try:
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 120, 0, 0))
        process = subprocess.Popen(
            command, stdin=subprocess.DEVNULL, stdout=stdout, stderr=slave,
            env={**os.environ, "TERM": "xterm-256color"},
        )
    finally:
        os.close(slave)
        if process is None:
            os.close(master)
    try:
        deadline = time.monotonic() + 30
        while True:
            if time.monotonic() >= deadline:
                raise TimeoutError("Terminal smoke exceeded 30 seconds")
            if not select.select([master], [], [], 0.1)[0]:
                continue
            try:
                chunk = os.read(master, 65536)
            except OSError as error:
                if error.errno != errno.EIO:
                    raise
                break  # Linux signals the closed slave with EIO; macOS may return EOF.
            if not chunk:
                break
            stderr.write(chunk)
        return process.wait(timeout=5)
    finally:
        os.close(master)
        if process.poll() is None:
            process.kill()
            process.wait()


def check_output(test, mode, stdout, stderr):
    if "1 passed; 0 failed" not in stdout:
        raise AssertionError("The exact smoke test did not run and pass")
    for stage in ["Fetching checksums...", "Downloading archive...",
                  "Verifying SHA-256...", "Extracting and installing..."]:
        if stage not in stderr or stage in stdout:
            raise AssertionError(f"Missing or misplaced stderr stage: {stage}")
    if "Downloaded " in stdout or any(dot in stdout for dot in DOTS):
        raise AssertionError("Progress leaked to stdout")
    if mode == "redirected":
        if "\x1b" in stderr or "\r" in stderr or any(dot in stderr for dot in DOTS):
            raise AssertionError("Redirected stderr contains animation/terminal controls")
    elif "\x1b[" not in stderr or not any(dot in stderr for dot in DOTS):
        raise AssertionError("The PTY did not exercise animated terminal output")

    if test == TESTS[0]:
        for amount in ["1024 bytes", "2048/4096 bytes (50%)",
                       "4096/4096 bytes (100%)", "4097 bytes"]:
            if amount not in stderr:
                raise AssertionError(f"Missing reporter amount/fallback: {amount}")
        marker = "stage output smoke complete"
        before, separator, after = stderr.partition(marker)
        if not separator or any(dot in after for dot in DOTS) or "Downloading" in after:
            raise AssertionError("Animation did not stop before final output")
        if mode == "pty":
            frames = re.findall(rf"([{DOTS}]) Downloading archive\.\.\. 1024 bytes", before)
            if len(set(frames)) < 2:
                raise AssertionError("Spinner did not advance while bytes stayed unchanged")
            if "[============>-----------]" not in before or "[========================]" not in before:
                raise AssertionError("Terminal progress bar did not reach half/full states")
            last_stage = before.rsplit("Extracting and installing...", 1)[-1]
            if not re.search(r"\x1b\[[0-9;]*K", last_stage):
                raise AssertionError("Progress line was not cleared before the final marker")
    else:
        duplicate = "already installed node@22.0.0"
        if stdout.count(duplicate) != 2 or stdout.replace(duplicate, "").count("installed node@22.0.0") != 2:
            raise AssertionError("Known/unknown install and duplicate results are missing")
        if stderr.count("expected invalid archive failure:") != 2:
            raise AssertionError("Known/unknown installation failure paths did not run")
        if mode == "redirected" and ("(100%)" not in stderr or not re.search(r"Downloaded \d+ bytes\n", stderr)):
            raise AssertionError("Lifecycle did not exercise known and unknown totals")


def main():
    if os.name != "posix":
        raise SystemExit("Native PTY acceptance requires macOS or Linux")
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    metadata = {
        "platform": platform.platform(), "python": platform.python_version(),
        "checkout_sha": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "pr_head_sha": os.environ.get("PR_HEAD_SHA"), "results": [],
    }
    executable = build_test_executable()
    for test in TESTS:
        for mode in ["pty", "redirected"]:
            name = f"{test.rsplit('::', 1)[-1]}-{mode}"
            command = [executable, test, "--exact", "--ignored", "--nocapture"]
            with (EVIDENCE / f"{name}.stdout").open("wb") as stdout, (EVIDENCE / f"{name}.stderr").open("wb") as stderr:
                if mode == "pty":
                    returncode = run_with_pty(command, stdout, stderr)
                else:
                    returncode = subprocess.run(command, stdout=stdout, stderr=stderr, timeout=30).returncode
            stdout = (EVIDENCE / f"{name}.stdout").read_bytes().decode("utf-8")
            stderr = (EVIDENCE / f"{name}.stderr").read_bytes().decode("utf-8")
            entry = {"command": command, "mode": mode, "returncode": returncode, "passed": False}
            metadata["results"].append(entry)
            try:
                if returncode:
                    raise AssertionError(f"{name} exited with {returncode}")
                check_output(test, mode, stdout, stderr)
                entry["passed"] = True
                print(f"PASS {name}", flush=True)
            finally:
                (EVIDENCE / "results.json").write_text(json.dumps(metadata, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
