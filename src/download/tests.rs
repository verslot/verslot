use std::fs;
use std::io::{self, Cursor, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::*;

const ABC_DIGEST: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
const FILENAME: &str = "node-v22.0.0-win-x64.zip";

// Compose the actual downloader with installation tests without production URL overrides.
pub(crate) fn download_fixture(
    version: Version,
    bytes: &[u8],
    archive: &Path,
    progress: &mut Progress<'_>,
) -> Result<String, String> {
    let mut distribution = NodeDistribution::for_current_build(version).unwrap();
    let digest: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let checksums = format!("{digest} {}\n", distribution.archive_filename);
    let (url, task) = serve(vec![response(checksums.as_bytes()), response(bytes)]);
    distribution.archive_url = format!("{url}/{}", distribution.archive_filename);
    distribution.checksum_url = format!("{url}/SHASUMS256.txt");
    let result =
        download_distribution_with_progress(&local_agent(), &distribution, archive, progress);
    task.join().unwrap();
    result
}

#[test]
fn download_failure_does_not_report_later_stages() {
    for (responses, expected_error, expected_output) in [
        (
            vec![response(b"invalid")],
            "missing entry",
            "Fetching checksums...\n",
        ),
        (
            vec![
                response(format!("{ABC_DIGEST} {FILENAME}\n").as_bytes()),
                b"HTTP/1.1 500 Error\r\nContent-Length: 0\r\n\r\n".to_vec(),
            ],
            "received 500",
            "Fetching checksums...\nDownloading archive...\n",
        ),
        (
            vec![
                response(format!("{ABC_DIGEST} {FILENAME}\n").as_bytes()),
                response(b"bad"),
            ],
            "SHA-256 mismatch",
            "Fetching checksums...\nDownloading archive...\nVerifying SHA-256...\n",
        ),
    ] {
        let (url, task) = serve(responses);
        let directory = OperationDirectory::new();
        let mut output = Vec::new();
        let result = download_distribution_with_progress(
            &local_agent(),
            &local_distribution(&url),
            &directory.archive_path(),
            &mut Progress::plain(&mut output),
        );
        task.join().unwrap();
        let error = result.unwrap_err();
        assert!(error.contains(expected_error), "{error}");
        assert_eq!(output, expected_output.as_bytes());
    }
}

#[test]
fn download_reports_stages_in_order_without_changing_the_digest() {
    let checksums = format!("{ABC_DIGEST} {FILENAME}\n");
    let output = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&output);
    let (url, task) = serve_observed(
        vec![response(checksums.as_bytes()), response(b"abc")],
        move |request| {
            let expected = if request == 0 {
                "Fetching checksums...\n"
            } else {
                "Fetching checksums...\nDownloading archive...\n"
            };
            assert_eq!(
                *observed.lock().unwrap(),
                expected.as_bytes(),
                "stage must be visible before the response"
            );
        },
    );
    let directory = OperationDirectory::new();
    let mut progress = crate::progress::Progress::plain(SharedOutput(Arc::clone(&output)));
    let digest = download_distribution_with_progress(
        &local_agent(),
        &local_distribution(&url),
        &directory.archive_path(),
        &mut progress,
    )
    .unwrap();
    drop(progress);
    task.join().unwrap();
    assert_eq!(digest, ABC_DIGEST);
    assert_eq!(
        *output.lock().unwrap(),
        b"Fetching checksums...\nDownloading archive...\nVerifying SHA-256...\n"
    );
}

struct SharedOutput(Arc<Mutex<Vec<u8>>>);

impl Write for SharedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

#[test]
fn checksum_matching_is_exact_and_accepts_lf_crlf_and_ascii_whitespace() {
    for text in [
        format!("{ABC_DIGEST}  {FILENAME}\n"),
        format!("\t{}\t{FILENAME} \r\n", ABC_DIGEST.to_ascii_uppercase()),
        format!("ignored unrelated.zip\n{ABC_DIGEST} {FILENAME}\n"),
    ] {
        assert_eq!(parse_checksum(&text, FILENAME).unwrap(), ABC_DIGEST);
    }
    for text in [
        String::new(),
        format!("{ABC_DIGEST} {FILENAME}.other\n"),
        format!("{ABC_DIGEST} prefix-{FILENAME}\n"),
    ] {
        assert!(
            parse_checksum(&text, FILENAME)
                .unwrap_err()
                .contains("missing")
        );
    }
}

#[test]
fn duplicate_and_malformed_matching_checksums_are_rejected() {
    let valid = format!("{ABC_DIGEST} {FILENAME}\n");
    assert!(
        parse_checksum(&valid.repeat(2), FILENAME)
            .unwrap_err()
            .contains("duplicate")
    );
    for text in [
        format!("short {FILENAME}\n"),
        format!("{} {FILENAME}\n", "g".repeat(64)),
        format!("{} {FILENAME}\n", "a".repeat(65)),
        format!("{ABC_DIGEST} {FILENAME} extra\n"),
        format!("{FILENAME}\n"),
        format!("{FILENAME} {ABC_DIGEST}\n"),
    ] {
        assert!(
            parse_checksum(&text, FILENAME)
                .unwrap_err()
                .contains("malformed")
        );
    }
}

#[test]
fn streaming_verifies_known_sha256_and_enforces_the_actual_byte_limit() {
    let mut output = Vec::new();
    assert_eq!(
        copy_bounded(&mut Cursor::new(b"abc"), &mut output, 3, deadline()).unwrap(),
        ABC_DIGEST
    );
    assert_eq!(output, b"abc");
    let mut oversized = Vec::new();
    assert!(
        copy_bounded(&mut Cursor::new(b"abcd"), &mut oversized, 3, deadline())
            .unwrap_err()
            .contains("3-byte limit")
    );
    assert!(oversized.is_empty());
}

#[test]
fn sha256_matches_the_million_a_vector_across_multiple_buffers() {
    let bytes = vec![b'a'; 1_000_000];
    assert_eq!(
        copy_bounded(
            &mut Cursor::new(bytes),
            &mut io::sink(),
            1_000_000,
            deadline(),
        )
        .unwrap(),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
}

struct FailingReader(io::ErrorKind);

impl Read for FailingReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(self.0, "injected read failure"))
    }
}

#[test]
fn read_failures_and_timeouts_do_not_return_a_verified_digest() {
    for kind in [io::ErrorKind::UnexpectedEof, io::ErrorKind::TimedOut] {
        let error =
            copy_bounded(&mut FailingReader(kind), &mut Vec::new(), 3, deadline()).unwrap_err();
        assert!(error.contains("read response"));
        assert!(error.contains("injected read failure"));
    }
}

struct FailingWriter {
    fail_flush: bool,
}

impl Write for FailingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.fail_flush {
            Ok(buffer.len())
        } else {
            Err(io::Error::other("injected write failure"))
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("injected flush failure"))
    }
}

#[test]
fn write_and_flush_failures_do_not_return_a_verified_digest() {
    for (fail_flush, expected) in [(false, "write download"), (true, "flush download")] {
        assert!(
            copy_bounded(
                &mut Cursor::new(b"abc"),
                &mut FailingWriter { fail_flush },
                3,
                deadline(),
            )
            .unwrap_err()
            .contains(expected)
        );
    }
}

#[test]
fn expired_deadline_prevents_reads_and_writes() {
    let mut output = Vec::new();
    let error = copy_bounded(&mut Cursor::new(b"abc"), &mut output, 3, Instant::now()).unwrap_err();
    assert!(error.contains("deadline"));
    assert!(output.is_empty());
}

// Serves generated fixtures on loopback only, never nodejs.org. Bounds every accept/read.
fn serve(responses: Vec<Vec<u8>>) -> (String, JoinHandle<()>) {
    serve_observed(responses, |_| {})
}

fn serve_observed(
    responses: Vec<Vec<u8>>,
    observe: impl Fn(usize) + Send + 'static,
) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let task = thread::spawn(move || {
        for (index, response) in responses.into_iter().enumerate() {
            let until = deadline();
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < until, "fixture connection not received");
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => panic!("fixture accept: {error}"),
                }
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                assert_eq!(socket.read(&mut byte).unwrap(), 1);
                request.push(byte[0]);
                assert!(request.len() < 8192);
            }
            assert!(
                String::from_utf8(request)
                    .unwrap()
                    .to_ascii_lowercase()
                    .contains("accept-encoding: identity")
            );
            observe(index);
            // The client may close early when rejecting a response or byte limit.
            let _ = socket.write_all(&response);
        }
    });
    (url, task)
}

fn response(body: &[u8]) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}

fn local_agent() -> Agent {
    let config = Agent::config_builder()
        .https_only(false)
        .proxy(None)
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .build();
    Agent::with_parts(config, ReadTimeoutConnector, DefaultResolver::default())
}

struct OperationDirectory(PathBuf);

impl OperationDirectory {
    fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "verslot-download-test-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn archive_path(&self) -> PathBuf {
        self.0.join("archive")
    }
}

impl Drop for OperationDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.archive_path());
        let _ = fs::remove_dir(&self.0);
    }
}

fn local_distribution(url: &str) -> NodeDistribution {
    NodeDistribution {
        archive_format: crate::distribution::ArchiveFormat::Zip,
        archive_filename: FILENAME.to_owned(),
        archive_url: format!("{url}/{FILENAME}"),
        checksum_url: format!("{url}/SHASUMS256.txt"),
    }
}

#[test]
fn valid_download_creates_an_exclusive_file_and_returns_the_verified_digest() {
    for archive in [
        response(b"abc"),
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3\r\nabc\r\n0\r\n\r\n".to_vec(),
        b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nContent-Encoding: gzip\r\nConnection: close\r\n\r\nabc".to_vec(),
    ] {
        let checksums = format!("{ABC_DIGEST} {FILENAME}\n");
        let (url, task) = serve(vec![response(checksums.as_bytes()), archive]);
        let directory = OperationDirectory::new();
        let path = directory.archive_path();
        assert_eq!(
            download_distribution(&local_agent(), &local_distribution(&url), &path).unwrap(),
            ABC_DIGEST
        );
        assert_eq!(fs::read(path).unwrap(), b"abc");
        task.join().unwrap();
    }
}

#[test]
fn invalid_checksums_stop_before_archive_download_or_file_creation() {
    for checksums in [
        format!("{ABC_DIGEST} other.zip\n").into_bytes(),
        format!("{ABC_DIGEST} {FILENAME}\n").repeat(2).into_bytes(),
        format!("bad {FILENAME}\n").into_bytes(),
        vec![0xff],
        vec![b'a'; CHECKSUM_LIMIT as usize + 1],
    ] {
        let (url, task) = serve(vec![response(&checksums)]);
        let directory = OperationDirectory::new();
        assert!(
            download_distribution(
                &local_agent(),
                &local_distribution(&url),
                &directory.archive_path(),
            )
            .is_err()
        );
        assert!(!directory.archive_path().exists());
        task.join().unwrap();
    }
}

#[test]
fn mismatched_and_truncated_archives_are_not_verified() {
    for (archive, expected) in [
        (response(b"abd"), "SHA-256 mismatch"),
        (
            b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nabc".to_vec(),
            "read response",
        ),
        (
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n4\r\nabc"
                .to_vec(),
            "read response",
        ),
    ] {
        let checksums = format!("{ABC_DIGEST} {FILENAME}\n");
        let (url, task) = serve(vec![response(checksums.as_bytes()), archive]);
        let directory = OperationDirectory::new();
        assert!(
            download_distribution(
                &local_agent(),
                &local_distribution(&url),
                &directory.archive_path(),
            )
            .unwrap_err()
            .contains(expected)
        );
        task.join().unwrap();
    }
}

#[test]
fn existing_archive_is_preserved() {
    let checksums = format!("{ABC_DIGEST} {FILENAME}\n");
    let (url, task) = serve(vec![response(checksums.as_bytes()), response(b"")]);
    let directory = OperationDirectory::new();
    fs::write(directory.archive_path(), b"existing").unwrap();
    assert!(
        download_distribution(
            &local_agent(),
            &local_distribution(&url),
            &directory.archive_path(),
        )
        .unwrap_err()
        .contains("create archive")
    );
    assert_eq!(fs::read(directory.archive_path()).unwrap(), b"existing");
    task.join().unwrap();
}

#[test]
fn non_200_responses_are_rejected_and_redirects_are_not_followed() {
    for (status, expected) in [
        (204, "expected HTTP 200"),
        (302, "expected HTTP 200"),
        (404, "unavailable"),
        (410, "unavailable"),
        (500, "expected HTTP 200"),
    ] {
        let fixture = format!(
            "HTTP/1.1 {status} fixture\r\nLocation: http://127.0.0.1:1/never\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
        .into_bytes();
        let (url, task) = serve(vec![fixture]);
        assert!(
            fetch_response(&local_agent(), &url, deadline())
                .unwrap_err()
                .contains(expected)
        );
        task.join().unwrap();
    }
}

#[test]
fn production_transport_requires_https_and_certificate_verification() {
    let agent = download_agent();
    assert!(agent.config().https_only());
    assert!(!agent.config().tls_config().disable_verification());
    assert!(fetch_response(&agent, "http://127.0.0.1:1/", deadline()).is_err());
    assert_eq!(agent.config().max_redirects(), 0);
}

#[test]
fn transport_failure_and_expired_request_deadline_are_reported() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    assert!(
        fetch_response(&local_agent(), &url, deadline())
            .unwrap_err()
            .contains("transport failure")
    );
    assert!(
        fetch_response(&local_agent(), &url, Instant::now())
            .unwrap_err()
            .contains("deadline")
    );
}

#[test]
fn global_deadline_interrupts_a_stalled_http_body() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let task = thread::spawn(move || {
        let until = deadline();
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < until);
                    thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("fixture accept: {error}"),
            }
        };
        socket.set_nonblocking(false).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = [0; 8192];
        assert!(socket.read(&mut request).unwrap() > 0);
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\n")
            .unwrap();
        thread::sleep(Duration::from_secs(1));
    });
    let until = Instant::now() + Duration::from_millis(500);
    let mut response = fetch_response(&local_agent(), &url, until).unwrap();
    let error = copy_bounded(
        &mut response.body_mut().as_reader(),
        &mut Vec::new(),
        3,
        until,
    )
    .unwrap_err();
    assert!(error.contains("read response") || error.contains("deadline"));
    task.join().unwrap();
}

#[derive(Debug)]
struct TimeoutRecorder(std::sync::Arc<std::sync::Mutex<Option<NextTimeout>>>);

impl Transport for TimeoutRecorder {
    fn buffers(&mut self) -> &mut dyn Buffers {
        panic!("only the read timeout is exercised by this fixture")
    }

    fn transmit_output(&mut self, _: usize, _: NextTimeout) -> Result<(), Error> {
        panic!("only the read timeout is exercised by this fixture")
    }

    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, Error> {
        *self.0.lock().unwrap() = Some(timeout);
        Ok(true)
    }

    fn is_open(&mut self) -> bool {
        true
    }
}

#[test]
fn transport_reads_are_capped_without_extending_shorter_deadlines() {
    for (budget, expected, reason) in [
        (DOWNLOAD_TIMEOUT, READ_TIMEOUT, Timeout::RecvBody),
        (
            Duration::from_secs(1),
            Duration::from_secs(1),
            Timeout::Global,
        ),
    ] {
        let recorded = std::sync::Arc::new(std::sync::Mutex::new(None));
        let mut transport = ReadTimeoutTransport(Box::new(TimeoutRecorder(recorded.clone())));
        transport
            .await_input(NextTimeout {
                after: budget.into(),
                reason: Timeout::Global,
            })
            .unwrap();
        let timeout = recorded.lock().unwrap().unwrap();
        assert_eq!(*timeout.after, expected);
        assert_eq!(timeout.reason, reason);
    }
}
