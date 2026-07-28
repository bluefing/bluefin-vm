//! `bluefin-vm` — download, import, and run a Bluefin VM on Apple Silicon.
//!
//! CLI-first, but the real work lives in `core` (UI-agnostic) so a ratatui
//! TUI can wrap the same operations later without rewriting them. `up` is the
//! front door — download → extract → import → run; the other subcommands expose
//! the individual steps for debugging.

mod core;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};

/// Where the CI-built seed is published (Cloudflare R2).
const DEFAULT_SEED_URL: &str = "https://projectbluefin.dev/bluefin-vm-raw-arm64.zip";
/// Default Tart VM label — matches the `just` recipes' `default_name`.
const DEFAULT_VM_NAME: &str = "Bluefin";

#[derive(Parser)]
#[command(name = "bluefin-vm", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Download, extract, import, and start the VM (the whole pipeline).
    Up {
        /// Tart VM name.
        #[arg(long, default_value = DEFAULT_VM_NAME)]
        name: String,
        /// Seed URL (defaults to the published seed).
        #[arg(long, default_value = DEFAULT_SEED_URL)]
        url: String,
        /// Expected hex SHA-256 of the seed zip; fails the run if it mismatches.
        #[arg(long)]
        sha256: Option<String>,
        /// Where to cache the seed zip and extracted disk.
        #[arg(long)]
        work_dir: Option<PathBuf>,
        /// Host folder shared into the VM (durable tier).
        #[arg(long)]
        share: Option<PathBuf>,
    },
    /// Download the VM seed (resumable), optionally verifying its checksum.
    Download {
        /// Destination file.
        #[arg(long, short)]
        out: PathBuf,
        /// Source URL (defaults to the published seed).
        #[arg(long, default_value = DEFAULT_SEED_URL)]
        url: String,
        /// Expected hex SHA-256; the download fails if it doesn't match.
        #[arg(long)]
        sha256: Option<String>,
    },
    /// Extract the raw disk from a seed zip.
    Extract {
        /// Seed zip to read.
        #[arg(long, short)]
        zip: PathBuf,
        /// Directory to write disk.raw into.
        #[arg(long)]
        out_dir: PathBuf,
    },
    /// Import a raw disk into a Tart VM (replaces the VM if it exists).
    Import {
        /// Raw disk to import.
        #[arg(long, short)]
        disk: PathBuf,
        /// Tart VM name.
        #[arg(long, default_value = DEFAULT_VM_NAME)]
        name: String,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Up {
            name,
            url,
            sha256,
            work_dir,
            share,
        } => up(&name, &url, sha256.as_deref(), work_dir, share),
        Command::Download { out, url, sha256 } => {
            let outcome = download(&url, &out, sha256.as_deref())?;
            report_download(&outcome, &out);
            Ok(())
        }
        Command::Extract { zip, out_dir } => {
            let disk = extract(&zip, &out_dir)?;
            eprintln!("Extracted {}", disk.display());
            Ok(())
        }
        Command::Import { disk, name } => {
            eprintln!("Importing {} into Tart VM '{name}'…", disk.display());
            core::tart::import(&disk, &core::tart::VmSpec::new(name))?;
            eprintln!("Imported.");
            Ok(())
        }
    }
}

/// The full pipeline: fetch the seed, unpack its disk, import it, start the VM.
fn up(
    name: &str,
    url: &str,
    expected: Option<&str>,
    work_dir: Option<PathBuf>,
    share: Option<PathBuf>,
) -> Result<()> {
    let work_dir = work_dir.unwrap_or_else(default_work_dir);
    let share = share.unwrap_or_else(default_share);
    std::fs::create_dir_all(&work_dir)
        .with_context(|| format!("creating work dir {}", work_dir.display()))?;

    let seed = work_dir.join(seed_filename(url));
    let outcome = download(url, &seed, expected)?;
    report_download(&outcome, &seed);

    let disk = extract(&seed, &work_dir)?;

    eprintln!("Importing into Tart VM '{name}'…");
    core::tart::import(&disk, &core::tart::VmSpec::new(name))?;

    let log = std::env::temp_dir().join(format!("tart-{name}.log"));
    core::tart::run_detached(name, &share, &log, Duration::from_secs(3))?;
    eprintln!(
        "VM '{name}' running detached (window open). Log: {}",
        log.display()
    );
    eprintln!("Share: {}", share.display());
    eprintln!("Stop it:  tart stop {name}");
    Ok(())
}

// --- step wrappers with progress UI -----------------------------------------

fn download(url: &str, out: &Path, expected: Option<&str>) -> Result<core::download::Outcome> {
    eprintln!("Downloading {url}");
    let bar = bytes_bar();
    let outcome = core::download::download(url, out, |done, total| {
        if total > 0 && bar.length() != Some(total) {
            bar.set_length(total);
        }
        bar.set_position(done);
    })?;
    bar.finish();

    if let Some(expected) = expected {
        eprint!("Verifying checksum… ");
        core::download::verify(out, expected)?;
        eprintln!("ok");
    }
    Ok(outcome)
}

fn extract(zip: &Path, out_dir: &Path) -> Result<PathBuf> {
    eprintln!("Extracting disk from {}", zip.display());
    let bar = bytes_bar();
    let disk = core::extract::extract_disk(zip, out_dir, |done, total| {
        if total > 0 && bar.length() != Some(total) {
            bar.set_length(total);
        }
        bar.set_position(done);
    })?;
    bar.finish();
    Ok(disk)
}

// --- helpers ----------------------------------------------------------------

fn bytes_bar() -> ProgressBar {
    let bar = ProgressBar::new(0);
    bar.set_style(
        ProgressStyle::with_template(
            "{bar:40.cyan/blue} {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
        )
        .expect("valid template"),
    );
    bar
}

fn report_download(outcome: &core::download::Outcome, path: &Path) {
    match outcome {
        core::download::Outcome::AlreadyComplete => {
            eprintln!("Already downloaded: {}", path.display())
        }
        core::download::Outcome::Resumed => eprintln!("Resumed — saved to {}", path.display()),
        core::download::Outcome::Downloaded => eprintln!("Saved to {}", path.display()),
    }
}

/// The seed's on-disk name, taken from the URL's last path segment (falling
/// back to a sensible default if the URL has no usable tail).
fn seed_filename(url: &str) -> String {
    url.rsplit('/')
        .find(|s| !s.is_empty())
        .filter(|s| s.ends_with(".zip"))
        .unwrap_or("bluefin-vm-seed.zip")
        .to_string()
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").expect("HOME is set"))
}

/// Cache for the seed zip and extracted disk. `$BLUEFIN_VM_WORK_DIR` overrides.
fn default_work_dir() -> PathBuf {
    std::env::var_os("BLUEFIN_VM_WORK_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".cache/bluefin-vm"))
}

/// The durable host share. Matches the `just` recipes' `default_share`.
fn default_share() -> PathBuf {
    std::env::var_os("TART_SHARE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join("bluefin-share"))
}

#[cfg(test)]
mod tests {
    use super::seed_filename;

    #[test]
    fn seed_filename_uses_the_url_tail() {
        assert_eq!(
            seed_filename("https://projectbluefin.dev/bluefin-vm-raw-arm64.zip"),
            "bluefin-vm-raw-arm64.zip"
        );
    }

    #[test]
    fn seed_filename_falls_back_when_no_zip_tail() {
        assert_eq!(seed_filename("https://example.com/"), "bluefin-vm-seed.zip");
        assert_eq!(seed_filename("no-slashes"), "bluefin-vm-seed.zip");
    }
}
