use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::download::tests::{local_agent, response, serve};

use super::*;

fn remote_query(input: &str) -> RemoteQuery {
    use clap::Parser;
    let cli = crate::cli::Cli::try_parse_from(["verslot", "list-remote", input]).unwrap();
    match cli.command {
        crate::cli::Command::ListRemote { query } => query,
        _ => panic!("expected remote query"),
    }
}

#[derive(Clone)]
struct ProgressLog(Arc<Mutex<Vec<u8>>>);

impl Write for ProgressLog {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn loading_is_visible_before_the_remote_index_response() {
    let log = ProgressLog(Arc::new(Mutex::new(Vec::new())));
    let observed = log.clone();
    let (url, server) = crate::download::tests::serve_observed(
        vec![response(b"version\tfiles\tlts\nv18.1.0\twin-x64-zip\t-\n")],
        move |_| assert_eq!(*observed.0.lock().unwrap(), b"   Fetching versions\n"),
    );
    let mut output = Vec::new();
    let mut progress = Progress::plain(log.clone());
    list_with_progress(
        &remote_query("node"),
        "win-x64-zip",
        &local_agent(),
        &url,
        Instant::now() + Duration::from_secs(5),
        &mut output,
        &mut progress,
    )
    .unwrap();
    server.join().unwrap();
    assert_eq!(output, b"   v18.1.0\n");
    assert_eq!(*log.0.lock().unwrap(), b"   Fetching versions\n");
}

#[derive(Clone, Debug)]
struct LoadingScreen(Arc<Mutex<String>>);

impl indicatif::TermLike for LoadingScreen {
    fn width(&self) -> u16 {
        120
    }
    fn move_cursor_up(&self, _: usize) -> std::io::Result<()> {
        Ok(())
    }
    fn move_cursor_down(&self, _: usize) -> std::io::Result<()> {
        Ok(())
    }
    fn move_cursor_right(&self, _: usize) -> std::io::Result<()> {
        Ok(())
    }
    fn move_cursor_left(&self, _: usize) -> std::io::Result<()> {
        Ok(())
    }
    fn write_line(&self, line: &str) -> std::io::Result<()> {
        self.write_str(&format!("{line}\n"))
    }
    fn write_str(&self, text: &str) -> std::io::Result<()> {
        self.0.lock().unwrap().push_str(text);
        Ok(())
    }
    fn clear_line(&self) -> std::io::Result<()> {
        self.0.lock().unwrap().clear();
        Ok(())
    }
    fn flush(&self) -> std::io::Result<()> {
        Ok(())
    }
}

struct AfterLoadingOutput {
    screen: LoadingScreen,
    bytes: Vec<u8>,
}

impl Write for AfterLoadingOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        assert!(
            self.screen.0.lock().unwrap().is_empty(),
            "loading must clear before result output"
        );
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn loading_clears_before_results_and_on_empty_results_or_errors() {
    use indicatif::{ProgressBar, ProgressDrawTarget};
    let cases: &[(Vec<u8>, Option<&[u8]>)] = &[
        (
            response(b"version\tfiles\tlts\nv18.1.0\twin-x64-zip\t-\n"),
            Some(b"   v18.1.0\n"),
        ),
        (
            response(b"version\tfiles\tlts\nv18.1.0\tsrc\t-\n"),
            Some(b""),
        ),
        (response(b"invalid index"), None),
        (
            b"HTTP/1.1 503 unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
            None,
        ),
    ];
    for (fixture, expected) in cases {
        let screen = LoadingScreen(Arc::new(Mutex::new(String::new())));
        let observed = screen.clone();
        let (url, server) =
            crate::download::tests::serve_observed(vec![fixture.clone()], move |_| {
                assert!(observed.0.lock().unwrap().contains("Fetching versions"))
            });
        let bar = ProgressBar::with_draw_target(
            None,
            ProgressDrawTarget::term_like(Box::new(screen.clone())),
        );
        let mut progress = Progress::Terminal(bar);
        let mut output = AfterLoadingOutput {
            screen: screen.clone(),
            bytes: Vec::new(),
        };
        let result = list_with_progress(
            &remote_query("node"),
            "win-x64-zip",
            &local_agent(),
            &url,
            Instant::now() + Duration::from_secs(5),
            &mut output,
            &mut progress,
        );
        server.join().unwrap();
        match expected {
            Some(expected) => {
                result.unwrap();
                assert_eq!(output.bytes, *expected);
            }
            None => {
                assert!(result.is_err());
                assert!(output.bytes.is_empty());
            }
        }
        assert!(screen.0.lock().unwrap().is_empty());
    }
}

struct FailingLoadingOutput(bool);

impl Write for FailingLoadingOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.0 {
            Err(std::io::Error::other("loading write failed"))
        } else {
            Ok(bytes.len())
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::other("loading flush failed"))
    }
}

#[test]
fn failed_loading_output_does_not_fail_a_successful_query() {
    for fail_write in [false, true] {
        let (url, server) = serve(vec![response(
            b"version\tfiles\tlts\nv18.1.0\twin-x64-zip\t-\n",
        )]);
        let mut output = Vec::new();
        let mut progress = Progress::plain(FailingLoadingOutput(fail_write));
        list_with_progress(
            &remote_query("node"),
            "win-x64-zip",
            &local_agent(),
            &url,
            Instant::now() + Duration::from_secs(5),
            &mut output,
            &mut progress,
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(output, b"   v18.1.0\n");
    }
}

#[test]
fn all_releases_are_printed_in_numeric_order_through_the_logger() {
    let index = b"version\tfiles\tlts\nv18.10.0\twin-x64-zip\t-\nv18.2.0\twin-x64-zip\t-\nv0.12.0\twin-x64-zip\t-\n";
    let (url, server) = serve(vec![response(index)]);
    let mut output = Vec::new();
    let result = list_with(
        &remote_query("node"),
        "win-x64-zip",
        &local_agent(),
        &url,
        Instant::now() + Duration::from_secs(5),
        &mut output,
    );
    result.unwrap();
    server.join().unwrap();
    assert_eq!(output, b"   v0.12.0\n   v18.2.0\n   v18.10.0\n");
}

#[test]
fn numeric_queries_match_literal_prefixes_across_version_components() {
    let index = b"version\tfiles\tlts\nv18.10.0\twin-x64-zip\t-\nv18.1.10\twin-x64-zip\t-\nv18.1.0\twin-x64-zip\t-\nv10.0.0\twin-x64-zip\t-\n";
    for (query, expected) in [
        ("node@18.1", "   v18.1.0\n   v18.1.10\n   v18.10.0\n"),
        ("node@18.1.1", "   v18.1.10\n"),
        ("node@18.1.0", "   v18.1.0\n"),
        (
            "node@1",
            "   v10.0.0\n   v18.1.0\n   v18.1.10\n   v18.10.0\n",
        ),
        ("node@9", ""),
    ] {
        let (url, server) = serve(vec![response(index)]);
        let mut output = Vec::new();
        list_with(
            &remote_query(query),
            "win-x64-zip",
            &local_agent(),
            &url,
            Instant::now() + Duration::from_secs(5),
            &mut output,
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), expected, "{query}");
    }
}

#[test]
fn lts_queries_include_historical_marked_releases_only() {
    let index = b"version\tfiles\tlts\nv18.20.8\twin-x64-zip\tHydrogen\nv18.0.0\twin-x64-zip\t-\nv18.12.0\twin-x64-zip\tHydrogen\nv4.9.1\twin-x64-zip\tArgon\n";
    let (url, server) = serve(vec![response(index)]);
    let mut output = Vec::new();
    list_with(
        &remote_query("node@lts"),
        "win-x64-zip",
        &local_agent(),
        &url,
        Instant::now() + Duration::from_secs(5),
        &mut output,
    )
    .unwrap();
    server.join().unwrap();
    assert_eq!(output, b"   v4.9.1\n   v18.12.0\n   v18.20.8\n");
}

#[test]
fn each_platform_lists_only_its_exact_archive_type() {
    let index = b"version\tfiles\tlts\nv18.0.0\twin-x64-zip,linux-x64,osx-arm64-tar\t-\nv18.1.0\twin-arm64-zip,linux-arm64,osx-x64-tar\t-\nv18.2.0\twin-x64-msi,win-arm64-7z,linux-x64-musl,osx-x64-pkg,osx-arm64-pkg\t-\n";
    for (os, architecture, environment, expected) in [
        ("windows", "x86_64", "msvc", "   v18.0.0\n"),
        ("windows", "aarch64", "msvc", "   v18.1.0\n"),
        ("macos", "x86_64", "", "   v18.1.0\n"),
        ("macos", "aarch64", "", "   v18.0.0\n"),
        ("linux", "x86_64", "gnu", "   v18.0.0\n"),
        ("linux", "aarch64", "gnu", "   v18.1.0\n"),
    ] {
        let platform = NodePlatform::for_platform(os, architecture, environment).unwrap();
        let (url, server) = serve(vec![response(index)]);
        let mut output = Vec::new();
        list_with(
            &remote_query("node"),
            platform.index_file,
            &local_agent(),
            &url,
            Instant::now() + Duration::from_secs(5),
            &mut output,
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            expected,
            "{os}/{architecture}"
        );
    }
}

#[test]
fn only_unique_canonical_official_releases_are_printed() {
    let index = b"version\tfiles\tlts\nv18.1.0\twin-x64-zip\t-\nv18.1.0\twin-x64-zip\tHydrogen\nv27.0.0-alpha.0\twin-x64-zip\t-\nv18.2.0-nightly\twin-x64-zip\t-\nv18.2.0+build\twin-x64-zip\t-\niojs-v3.3.1\twin-x64-zip\t-\nv018.2.0\twin-x64-zip\t-\nv18.02.0\twin-x64-zip\t-\nv18.2\twin-x64-zip\t-\nv4294967296.0.0\twin-x64-zip\t-\nv18.2.0\tsrc\t-\n";
    let (url, server) = serve(vec![response(index)]);
    let mut output = Vec::new();
    list_with(
        &remote_query("node"),
        "win-x64-zip",
        &local_agent(),
        &url,
        Instant::now() + Duration::from_secs(5),
        &mut output,
    )
    .unwrap();
    server.join().unwrap();
    assert_eq!(output, b"   v18.1.0\n");
}

#[test]
fn malformed_indexes_fail_without_printing_partial_results() {
    let cases: &[&[u8]] = &[
        b"version\tfiles\tlts\nv18.1.0\twin-x64-zip\tHydrogen\nv18.2.0\twin-x64-zip\t\n",
        b"version\tfiles\tlts\nv18.1.0\twin-x64-zip\t-\nv18.2.0\t\t-\n",
        b"version\tfiles\tlts\nv18.1.0\twin-x64-zip\t-\nv18.2.0\twin-x64-zip,\t-\n",
        b"version\tfiles\tlts\nv18.1.0\twin-x64-zip\t-\nv18.2.0\twin-x64-zip\t\x1b[31m\n",
        b"version\tfiles\tlts\nv18.1.0\twin-x64-zip\t-\nv18.2.0\twin-x64-zip\n",
        b"version\tfiles\tlts\nv18.1.0\twin-x64-zip\t-\n\xff",
        b"",
        b"<html>not an index</html>",
        b"version\tfiles\nv18.1.0\twin-x64-zip\n",
        b"version\tfiles\tfiles\tlts\nv18.1.0\twin-x64-zip\tsrc\t-\n",
    ];
    for index in cases {
        let (url, server) = serve(vec![response(index)]);
        let mut output = Vec::new();
        let error = list_with(
            &remote_query("node"),
            "win-x64-zip",
            &local_agent(),
            &url,
            Instant::now() + Duration::from_secs(5),
            &mut output,
        )
        .unwrap_err();
        server.join().unwrap();
        assert!(error.contains("release index"), "{error}");
        assert!(!error.contains('\x1b'), "{error}");
        assert!(output.is_empty(), "{output:?}");
    }
}

#[test]
fn transport_errors_leave_stdout_empty_and_explain_the_failure() {
    let mut fixtures = Vec::new();
    for status in [204, 302, 404, 500] {
        fixtures.push((format!("HTTP/1.1 {status} fixture\r\nLocation: http://127.0.0.1:1/never\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").into_bytes(), format!("{status}")));
    }
    fixtures.push((b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\nConnection: close\r\n\r\nversion\tfiles\tlts\n".to_vec(), "read response".to_owned()));
    fixtures.push((
        response(&vec![b'x'; 4_194_305]),
        "4194304-byte limit".to_owned(),
    ));
    for (fixture, reason) in fixtures {
        let (url, server) = serve(vec![fixture]);
        let mut output = Vec::new();
        let error = list_with(
            &remote_query("node"),
            "win-x64-zip",
            &local_agent(),
            &url,
            Instant::now() + Duration::from_secs(5),
            &mut output,
        )
        .unwrap_err();
        server.join().unwrap();
        assert!(error.contains(&reason), "{error}");
        assert!(output.is_empty());
    }
}

#[test]
fn a_stalled_index_is_interrupted_by_the_query_deadline() {
    let (url, server) =
        crate::download::tests::serve_observed(vec![response(b"version\tfiles\tlts\n")], |_| {
            std::thread::sleep(Duration::from_secs(1))
        });
    let mut output = Vec::new();
    let error = list_with(
        &remote_query("node"),
        "win-x64-zip",
        &local_agent(),
        &url,
        Instant::now() + Duration::from_millis(500),
        &mut output,
    )
    .unwrap_err();
    server.join().unwrap();
    assert!(
        error.contains("timeout") || error.contains("deadline"),
        "{error}"
    );
    assert!(output.is_empty());
}

#[test]
fn repeated_queries_fetch_fresh_results_instead_of_caching() {
    let (url, server) = serve(vec![
        response(b"version\tfiles\tlts\nv18.1.0\twin-x64-zip\t-\n"),
        response(b"version\tfiles\tlts\nv18.2.0\twin-x64-zip\t-\n"),
    ]);
    let agent = local_agent();
    for expected in [b"   v18.1.0\n", b"   v18.2.0\n"] {
        let mut output = Vec::new();
        list_with(
            &remote_query("node"),
            "win-x64-zip",
            &agent,
            &url,
            Instant::now() + Duration::from_secs(5),
            &mut output,
        )
        .unwrap();
        assert_eq!(output, expected);
    }
    server.join().unwrap();
}

#[test]
fn full_results_are_not_truncated_to_a_recent_release_limit() {
    let mut index = "version\tfiles\tlts\n".to_owned();
    for minor in (0..30).rev() {
        index.push_str(&format!("v18.{minor}.0\twin-x64-zip\t-\n"));
    }
    let (url, server) = serve(vec![response(index.as_bytes())]);
    let mut output = Vec::new();
    list_with(
        &remote_query("node"),
        "win-x64-zip",
        &local_agent(),
        &url,
        Instant::now() + Duration::from_secs(5),
        &mut output,
    )
    .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(output).unwrap();
    assert_eq!(output.lines().count(), 30);
    assert!(output.starts_with("   v18.0.0\n"));
    assert!(output.ends_with("   v18.29.0\n"));
}

#[test]
fn listing_does_not_require_a_storage_environment() {
    let (url, server) = serve(vec![response(
        b"version\tfiles\tlts\nv18.1.0\twin-x64-zip\t-\n",
    )]);
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "remote::tests::storage_free_query_child"])
        .env("VERSLOT_REMOTE_TEST_INDEX_URL", url)
        .env_remove("HOME")
        .env_remove("LOCALAPPDATA")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    server.join().unwrap();
}

#[test]
fn storage_free_query_child() {
    let Ok(url) = std::env::var("VERSLOT_REMOTE_TEST_INDEX_URL") else {
        return;
    };
    let mut output = Vec::new();
    list_with(
        &remote_query("node"),
        "win-x64-zip",
        &local_agent(),
        &url,
        Instant::now() + Duration::from_secs(5),
        &mut output,
    )
    .unwrap();
    assert_eq!(output, b"   v18.1.0\n");
}

#[test]
fn both_command_names_execute_the_same_queries_and_failures() {
    use clap::Parser;
    let index = b"version\tfiles\tlts\nv18.10.0\twin-x64-zip\tHydrogen\nv18.1.0\twin-x64-zip\t-\n";
    for command in ["list-remote", "ls-remote"] {
        for (query, expected) in [
            ("node", "   v18.1.0\n   v18.10.0\n"),
            ("node@18.1", "   v18.1.0\n   v18.10.0\n"),
            ("node@18.1.0", "   v18.1.0\n"),
            ("node@lts", "   v18.10.0\n"),
            ("node@0", ""),
            ("node@4294967295.4294967295.4294967295", ""),
        ] {
            let cli = crate::cli::Cli::try_parse_from(["verslot", command, query]).unwrap();
            let crate::cli::Command::ListRemote { query } = cli.command else {
                panic!("expected remote query");
            };
            let (url, server) = serve(vec![response(index), response(b"invalid index")]);
            let agent = local_agent();
            let mut output = Vec::new();
            list_with(
                &query,
                "win-x64-zip",
                &agent,
                &url,
                Instant::now() + Duration::from_secs(5),
                &mut output,
            )
            .unwrap();
            assert_eq!(String::from_utf8(output).unwrap(), expected);
            let mut failed_output = Vec::new();
            let error = list_with(
                &query,
                "win-x64-zip",
                &agent,
                &url,
                Instant::now() + Duration::from_secs(5),
                &mut failed_output,
            )
            .unwrap_err();
            server.join().unwrap();
            assert!(error.contains("release index"));
            assert!(failed_output.is_empty());
        }
    }
}
