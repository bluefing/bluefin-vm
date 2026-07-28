//! `bluefin-vm` — download, import, and run a Bluefin VM on Apple Silicon.
//!
//! CLI-first, but the real work lives in `core` (UI-agnostic) so a ratatui
//! TUI can wrap the same operations later without rewriting them.

mod core;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};

/// Where the CI-built seed is published (Cloudflare R2).
const DEFAULT_SEED_URL: &str = "https://projectbluefin.dev/bluefin-vm-raw-arm64.zip";

#[derive(Parser)]
#[command(name = "bluefin-vm", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
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
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Download { out, url, sha256 } => download(&url, out, sha256.as_deref()),
    }
}

fn download(url: &str, out: PathBuf, expected: Option<&str>) -> Result<()> {
    eprintln!("Downloading {url}");
    let bar = ProgressBar::new(0);
    bar.set_style(
        ProgressStyle::with_template(
            "{bar:40.cyan/blue} {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
        )
        .expect("valid template"),
    );

    let outcome = core::download::download(url, &out, |done, total| {
        if total > 0 && bar.length() != Some(total) {
            bar.set_length(total);
        }
        bar.set_position(done);
    })?;
    bar.finish();

    if let Some(expected) = expected {
        eprint!("Verifying checksum… ");
        core::download::verify(&out, expected)?;
        eprintln!("ok");
    }

    match outcome {
        core::download::Outcome::AlreadyComplete => {
            eprintln!("Already downloaded: {}", out.display())
        }
        core::download::Outcome::Resumed => eprintln!("Resumed — saved to {}", out.display()),
        core::download::Outcome::Downloaded => eprintln!("Saved to {}", out.display()),
    }
    Ok(())
}
