//! Resumable HTTP download and checksum — UI-agnostic.
//!
//! No printing or progress UI lives here: callers pass a `progress` closure
//! that receives `(downloaded, total)` byte counts, so the CLI can drive an
//! indicatif bar and a future TUI can drive a ratatui gauge from the same
//! code.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

const CHUNK: usize = 64 * 1024;

/// What a `download` call actually did, so the caller can report it clearly.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Fetched fresh (no prior file, or the server ignored our range).
    Downloaded,
    /// Resumed from a partial file.
    Resumed,
    /// The file was already complete; nothing to fetch.
    AlreadyComplete,
}

/// Download `url` to `dest`, resuming if a partial file already exists.
///
/// `progress(downloaded, total)` is called as bytes arrive; `total` is 0 when
/// the server doesn't report a length.
pub fn download(url: &str, dest: &Path, mut progress: impl FnMut(u64, u64)) -> Result<Outcome> {
    let existing = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);

    let mut req = ureq::get(url);
    if existing > 0 {
        req = req.header("Range", format!("bytes={existing}-"));
    }
    let resp = match req.call() {
        Ok(resp) => resp,
        // 416 Range Not Satisfiable: the local file is already at or beyond the
        // remote size, i.e. a finished download re-run. Treat it as a no-op
        // success rather than an error.
        Err(ureq::Error::StatusCode(416)) => {
            progress(existing, existing);
            return Ok(Outcome::AlreadyComplete);
        }
        Err(e) => return Err(anyhow::Error::new(e).context("HTTP request failed")),
    };

    let status = resp.status();
    if !status.is_success() {
        bail!("server returned {status}");
    }
    // 206 => server honoured the range, append; anything else => start fresh.
    let resuming = status.as_u16() == 206;
    let remaining: u64 = resp
        .headers()
        .get("Content-Length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let total = if resuming {
        existing + remaining
    } else {
        remaining
    };

    let mut reader = resp.into_body().into_reader();
    let mut file = if resuming {
        OpenOptions::new()
            .append(true)
            .open(dest)
            .with_context(|| format!("opening {} to append", dest.display()))?
    } else {
        File::create(dest).with_context(|| format!("creating {}", dest.display()))?
    };

    let mut done = if resuming { existing } else { 0 };
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = reader.read(&mut buf).context("reading response body")?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).context("writing to disk")?;
        done += n as u64;
        progress(done, total);
    }
    Ok(if resuming {
        Outcome::Resumed
    } else {
        Outcome::Downloaded
    })
}

/// Hex-encoded SHA-256 of a file.
pub fn sha256(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = file.read(&mut buf).context("reading for checksum")?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Verify a file's SHA-256 against an expected hex digest (case-insensitive).
pub fn verify(path: &Path, expected: &str) -> Result<()> {
    let actual = sha256(path)?;
    if !actual.eq_ignore_ascii_case(expected) {
        bail!("checksum mismatch:\n  expected {expected}\n  actual   {actual}");
    }
    Ok(())
}

/// Fetch a small text resource in one GET -- e.g. the published `.sha256`
/// sidecar. No resume or progress: the whole (capped) body is read into a
/// String. For metadata only, never the multi-GiB disk.
pub fn fetch_text(url: &str) -> Result<String> {
    let resp = ureq::get(url)
        .call()
        .map_err(|e| anyhow::Error::new(e).context("HTTP request failed"))?;
    let status = resp.status();
    if !status.is_success() {
        bail!("server returned {status}");
    }
    let mut body = String::new();
    // Cap the read: a checksum sidecar is tens of bytes; a huge body means a
    // wrong URL, not a checksum.
    resp.into_body()
        .into_reader()
        .take(64 * 1024)
        .read_to_string(&mut body)
        .context("reading response body")?;
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;

    /// A one-blob HTTP/1.1 server that honours `Range: bytes=N-` (206) and
    /// otherwise serves the whole blob (200). Returns the URL and a shared log
    /// of each request's Range start (None when absent), so a test can assert
    /// that a resume genuinely took the 206 path rather than restarting.
    fn serve_blob(blob: Vec<u8>) -> (String, Arc<Mutex<Vec<Option<u64>>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let log = Arc::new(Mutex::new(Vec::<Option<u64>>::new()));
        let log_writer = Arc::clone(&log);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = match stream {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut range_start: Option<u64> = None;
                let mut line = String::new();
                reader.read_line(&mut line).unwrap(); // request line
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).unwrap() == 0 {
                        break;
                    }
                    if header == "\r\n" || header == "\n" {
                        break;
                    }
                    let lower = header.to_ascii_lowercase();
                    if let Some(rest) = lower.strip_prefix("range:") {
                        if let Some(pos) = rest.find("bytes=") {
                            let digits: String = rest[pos + 6..]
                                .chars()
                                .take_while(|c| c.is_ascii_digit())
                                .collect();
                            range_start = digits.parse().ok();
                        }
                    }
                }
                log_writer.lock().unwrap().push(range_start);
                let (status, body): (&str, &[u8]) = match range_start {
                    // Range past the end -> 416, like a real server.
                    Some(n) if (n as usize) >= blob.len() => ("416 Range Not Satisfiable", &[]),
                    Some(n) => ("206 Partial Content", &blob[n as usize..]),
                    None => ("200 OK", &blob[..]),
                };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(head.as_bytes()).unwrap();
                stream.write_all(body).unwrap();
                stream.flush().unwrap();
            }
        });
        (format!("http://127.0.0.1:{port}/blob"), log)
    }

    fn sample_blob() -> Vec<u8> {
        (0u8..=255).cycle().take(100_000).collect()
    }

    #[test]
    fn sha256_matches_known_vector() {
        let path = std::env::temp_dir().join("bluefin-vm-sha256-test");
        std::fs::write(&path, b"abc").unwrap();
        // Canonical NIST test vector for SHA-256("abc").
        assert_eq!(
            sha256(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn downloads_the_whole_blob() {
        let blob = sample_blob();
        let (url, _log) = serve_blob(blob.clone());
        let dest = std::env::temp_dir().join("bluefin-vm-dl-full");
        let _ = std::fs::remove_file(&dest);
        assert_eq!(
            download(&url, &dest, |_, _| {}).unwrap(),
            Outcome::Downloaded
        );
        assert_eq!(std::fs::read(&dest).unwrap(), blob);
        let _ = std::fs::remove_file(&dest);
    }

    #[test]
    fn resumes_from_a_partial_file() {
        let blob = sample_blob();
        let (url, log) = serve_blob(blob.clone());
        let dest = std::env::temp_dir().join("bluefin-vm-dl-resume");
        // Simulate an interrupted download: the first 40_000 bytes are on disk.
        std::fs::write(&dest, &blob[..40_000]).unwrap();
        assert_eq!(download(&url, &dest, |_, _| {}).unwrap(), Outcome::Resumed);
        // The completed file is correct...
        assert_eq!(std::fs::read(&dest).unwrap(), blob);
        // ...and it genuinely resumed: the server saw a Range starting at 40_000.
        assert_eq!(log.lock().unwrap().last().copied().flatten(), Some(40_000));
        let _ = std::fs::remove_file(&dest);
    }

    #[test]
    fn already_complete_is_a_noop() {
        let blob = sample_blob();
        let (url, _log) = serve_blob(blob.clone());
        let dest = std::env::temp_dir().join("bluefin-vm-dl-complete");
        std::fs::write(&dest, &blob).unwrap(); // already fully downloaded
                                               // Re-running must not error (the 416 is handled) and reports completion.
        assert_eq!(
            download(&url, &dest, |_, _| {}).unwrap(),
            Outcome::AlreadyComplete
        );
        assert_eq!(std::fs::read(&dest).unwrap(), blob);
        let _ = std::fs::remove_file(&dest);
    }

    #[test]
    fn verify_accepts_correct_and_rejects_wrong() {
        let path = std::env::temp_dir().join("bluefin-vm-verify-test");
        std::fs::write(&path, b"abc").unwrap();
        let good = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        verify(&path, good).unwrap();
        verify(&path, &good.to_uppercase()).unwrap(); // case-insensitive
        assert!(verify(&path, "deadbeef").is_err());
        let _ = std::fs::remove_file(&path);
    }
}
