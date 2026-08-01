//! `bluefin-vm` — download, import, and run a Bluefin VM on Apple Silicon.
//!
//! CLI-first, but the real work lives in `core` (UI-agnostic) so a ratatui
//! TUI can wrap the same operations later without rewriting them. `up` is the
//! front door — download → extract → import → provision → run; the other
//! subcommands expose the individual steps for debugging.

mod core;
mod tui;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
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

/// Flags shared by `up` and `provision` for the first-boot account.
#[derive(Args)]
struct ProvisionArgs {
    /// Account to create in the VM (default: your macOS username, $USER).
    #[arg(long)]
    user: Option<String>,
    /// SSH public key to authorise (default: auto-detected ~/.ssh/*.pub).
    #[arg(long)]
    ssh_key: Option<PathBuf>,
    /// Boot to the greeter instead of autologging into the provisioned user.
    #[arg(long)]
    no_autologin: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Download, extract, import, provision, and start the VM (the whole pipeline).
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
        /// Skip provisioning; boot the baked test login instead.
        #[arg(long)]
        no_provision: bool,
        #[command(flatten)]
        provision: ProvisionArgs,
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
    /// Write first-boot provisioning into the share (usually done by `up`).
    Provision {
        /// Tart VM name -- keys the saved profile.
        #[arg(long, default_value = DEFAULT_VM_NAME)]
        name: String,
        #[command(flatten)]
        provision: ProvisionArgs,
        /// Host folder shared into the VM (durable tier).
        #[arg(long)]
        share: Option<PathBuf>,
    },
    /// Interactively edit a VM's saved profile (account, share, resources).
    Setup {
        /// Tart VM name -- the profile to edit.
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
            no_provision,
            provision,
        } => up(
            &name,
            &url,
            sha256.as_deref(),
            work_dir,
            share,
            provision,
            no_provision,
        ),
        Command::Download { out, url, sha256 } => {
            let out = core::config::expand_tilde(out);
            let outcome = download(&url, &out, sha256.as_deref())?;
            report_download(&outcome, &out);
            Ok(())
        }
        Command::Extract { zip, out_dir } => {
            let zip = core::config::expand_tilde(zip);
            let out_dir = core::config::expand_tilde(out_dir);
            let (disk, outcome) = extract(&zip, &out_dir)?;
            report_extract(&outcome, &disk);
            Ok(())
        }
        Command::Import { disk, name } => {
            let disk = core::config::expand_tilde(disk);
            eprintln!("Importing {} into Tart VM '{name}'…", disk.display());
            let config = core::config::Config::load()?;
            let spec =
                core::tart::VmSpec::resolve(&name, config.profile(&name).map(|p| &p.resources));
            core::tart::import(&disk, &spec)?;
            eprintln!("Imported.");
            Ok(())
        }
        Command::Provision {
            name,
            provision,
            share,
        } => {
            let mut config = core::config::Config::load()?;
            let share = core::config::expand_tilde(
                share
                    .or_else(|| {
                        config
                            .profile(&name)
                            .and_then(|p| p.share.directory.clone())
                    })
                    .unwrap_or_else(default_share),
            );
            let prov = resolve_account(&mut config, &name, provision)?;
            core::provision::write(&share, &prov)?;
            config.save()?;
            eprintln!(
                "Wrote provisioning for '{}' to {} (saved to config)",
                prov.username,
                share.join(".bluefin-vm").display()
            );
            Ok(())
        }
        Command::Setup { name } => tui::run(&name),
    }
}

/// The full pipeline: fetch the seed, unpack its disk, import it, provision the
/// first-boot account, and start the VM.
#[allow(clippy::too_many_arguments)]
fn up(
    name: &str,
    url: &str,
    expected: Option<&str>,
    work_dir: Option<PathBuf>,
    share: Option<PathBuf>,
    provision: ProvisionArgs,
    no_provision: bool,
) -> Result<()> {
    let work_dir = core::config::expand_tilde(work_dir.unwrap_or_else(default_work_dir));
    std::fs::create_dir_all(&work_dir)
        .with_context(|| format!("creating work dir {}", work_dir.display()))?;

    let seed = work_dir.join(seed_filename(url));
    let outcome = download(url, &seed, expected)?;
    report_download(&outcome, &seed);

    let (disk, extract_outcome) = extract(&seed, &work_dir)?;
    report_extract(&extract_outcome, &disk);

    let mut config = core::config::Config::load()?;

    // Share precedence mirrors resources: --share flag > profile > default.
    // Read-only is profile-only (no flag yet).
    let read_only = config
        .profile(name)
        .and_then(|p| p.share.read_only)
        .unwrap_or(false);
    let share = core::config::expand_tilde(
        share
            .or_else(|| config.profile(name).and_then(|p| p.share.directory.clone()))
            .unwrap_or_else(default_share),
    );

    eprintln!("Importing into Tart VM '{name}'…");
    let spec = core::tart::VmSpec::resolve(name, config.profile(name).map(|p| &p.resources));
    core::tart::import(&disk, &spec)?;

    // Provision before boot: the guest oneshot reads the share on first boot.
    let provisioned = if no_provision {
        None
    } else {
        let prov = resolve_account(&mut config, name, provision)?;
        core::provision::write(&share, &prov)?;
        config.save()?;
        eprintln!(
            "Provisioned first-boot account '{}' (saved to config).",
            prov.username
        );
        Some(prov.username)
    };

    let log = std::env::temp_dir().join(format!("tart-{name}.log"));
    core::tart::run_detached(name, &share, read_only, &log, Duration::from_secs(3))?;
    eprintln!(
        "VM '{name}' running detached (window open). Log: {}",
        log.display()
    );
    match &provisioned {
        Some(user) => {
            eprintln!("First boot creates '{user}' — log in there (autologin / ssh key).");
            eprintln!("Lock it down anytime: run `bluefin-vm-harden` in the VM.");
        }
        None => eprintln!("No provisioning — the baked test login (bluefin/bluefin) applies."),
    }
    eprintln!(
        "Share: {}{}",
        share.display(),
        if read_only { " (read-only)" } else { "" }
    );
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

fn extract(zip: &Path, out_dir: &Path) -> Result<(PathBuf, core::extract::Outcome)> {
    eprintln!("Extracting disk from {}", zip.display());
    let bar = bytes_bar();
    let (disk, outcome) = core::extract::extract_disk(zip, out_dir, |done, total| {
        if total > 0 && bar.length() != Some(total) {
            bar.set_length(total);
        }
        bar.set_position(done);
    })?;
    bar.finish();
    Ok((disk, outcome))
}

fn report_extract(outcome: &core::extract::Outcome, path: &Path) {
    match outcome {
        core::extract::Outcome::AlreadyExtracted => {
            eprintln!("Already extracted: {}", path.display())
        }
        core::extract::Outcome::Extracted => eprintln!("Extracted to {}", path.display()),
    }
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

/// Default account name: the host's `$USER`, so the VM feels like the user's.
fn default_username() -> Option<String> {
    std::env::var("USER").ok().filter(|s| !s.is_empty())
}

/// Auto-detect the host's ssh public key, preferring ed25519.
fn default_ssh_key() -> Option<PathBuf> {
    let ssh = home().join(".ssh");
    ["id_ed25519.pub", "id_ecdsa.pub", "id_rsa.pub"]
        .into_iter()
        .map(|f| ssh.join(f))
        .find(|p| p.exists())
}

impl ProvisionArgs {
    /// Resolve flags, the VM's saved profile, and host defaults into an account
    /// -- precedence flag > profile > default. Returns the persistable form
    /// (an ssh key *path*, not its contents); `provision_from` materialises it.
    fn resolve(self, saved: Option<&core::config::Account>) -> Result<core::config::Account> {
        let user = self
            .user
            .or_else(|| saved.and_then(|a| a.user.clone()))
            .or_else(default_username)
            .context("no --user given and $USER is unset")?;
        let ssh_key = self
            .ssh_key
            .map(core::config::expand_tilde)
            .map(core::config::SshKey::Path)
            .or_else(|| saved.and_then(|a| a.ssh_key.clone()))
            .or_else(|| default_ssh_key().map(core::config::SshKey::Path));
        // --no-autologin only ever disables; otherwise fall to the profile, then on.
        let autologin = if self.no_autologin {
            false
        } else {
            saved.and_then(|a| a.autologin).unwrap_or(true)
        };
        Ok(core::config::Account {
            user: Some(user),
            ssh_key,
            autologin: Some(autologin),
        })
    }
}

/// Turn a resolved account into first-boot provisioning data, reading the ssh
/// key file into `authorized_keys`.
fn provision_from(
    account: &core::config::Account,
    scale: Option<u32>,
) -> Result<core::provision::Provision> {
    let username = account
        .user
        .clone()
        .context("resolved account has no username")?;
    let authorized_keys = match &account.ssh_key {
        Some(core::config::SshKey::Path(p)) => std::fs::read_to_string(p)
            .with_context(|| format!("reading ssh key {}", p.display()))?,
        Some(core::config::SshKey::Disabled) => String::new(),
        None => {
            eprintln!("Warning: no ssh public key found (~/.ssh/*.pub); pass --ssh-key to set one");
            String::new()
        }
    };
    Ok(core::provision::Provision {
        username,
        authorized_keys,
        autologin: account.autologin.unwrap_or(true),
        scale,
    })
}

/// Resolve the account for `name` (flag > profile > default) and store it back
/// into `config`, returning the provisioning data. The caller writes it to the
/// share and persists `config`. Scale has no CLI flag -- it's profile-only,
/// set via `bluefin-vm setup`.
fn resolve_account(
    config: &mut core::config::Config,
    name: &str,
    args: ProvisionArgs,
) -> Result<core::provision::Provision> {
    let scale = config.profile(name).and_then(|p| p.resources.scale);
    let account = args.resolve(config.profile(name).map(|p| &p.account))?;
    let prov = provision_from(&account, scale)?;
    // Merge into any existing profile so other categories (resources) survive.
    let mut profile = config.profile(name).cloned().unwrap_or_default();
    profile.account = account;
    config.set_profile(name, profile);
    Ok(prov)
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
