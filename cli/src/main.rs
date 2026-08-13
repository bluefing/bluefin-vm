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

/// Where the CI-built disk image is published (Cloudflare R2, via the
/// `disks.bluefing.net` custom domain). Hardcoded for now -- lifting it into
/// config, and renaming the legacy `seed` terms to `image`, are backlog items.
const DEFAULT_SEED_URL: &str = "https://disks.bluefing.net/bluefin-vm-raw-arm64.zip";
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
        /// Hex SHA-256 of the disk zip; also the cache key. Defaults to the
        /// published `<url>.sha256`; a download mismatch fails the run.
        #[arg(long)]
        sha256: Option<String>,
        /// Where to cache disk images (each build under its own checksum).
        #[arg(long)]
        work_dir: Option<PathBuf>,
        /// Host folder shared into the VM (durable tier).
        #[arg(long)]
        share: Option<PathBuf>,
        /// Skip provisioning; boot the baked test login instead.
        #[arg(long)]
        no_provision: bool,
        /// Replace an existing VM (destroys its state) instead of booting it.
        #[arg(long)]
        replace: bool,
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
    Tui {
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
            replace,
            provision,
        } => up(
            &name,
            &url,
            sha256.as_deref(),
            work_dir,
            share,
            provision,
            no_provision,
            replace,
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
        Command::Tui { name } => match tui::run(&name)? {
            // The Up button runs the same pipeline as `bluefin-vm up`, with
            // the saved profile supplying every choice -- one verb, two front
            // ends. Non-destructive: an existing VM is booted, not replaced.
            tui::Outcome::Up => up(
                &name,
                DEFAULT_SEED_URL,
                None,
                None,
                None,
                ProvisionArgs {
                    user: None,
                    ssh_key: None,
                },
                false,
                false,
            ),
            tui::Outcome::Saved | tui::Outcome::Cancelled => Ok(()),
        },
    }
}

/// The full pipeline: fetch the seed, unpack its disk, import it, provision the
/// first-boot account, and start the VM. An existing VM is booted, not
/// replaced -- it holds the user's state -- unless `--replace` asks for a
/// fresh one by name.
#[allow(clippy::too_many_arguments)]
fn up(
    name: &str,
    url: &str,
    expected: Option<&str>,
    work_dir: Option<PathBuf>,
    share: Option<PathBuf>,
    provision: ProvisionArgs,
    no_provision: bool,
    replace: bool,
) -> Result<()> {
    if core::tart::exists(name) && !replace {
        let config = core::config::Config::load()?;
        let (share, read_only) = resolve_share(&config, name, share);
        eprintln!(
            "VM '{name}' already exists — booting it. Profile changes do not apply to \
             an existing VM (apply them to a fresh one: up --replace)."
        );
        let log = std::env::temp_dir().join(format!("tart-{name}.log"));
        core::tart::run_detached(name, &share, read_only, &log, Duration::from_secs(3))?;
        eprintln!(
            "VM '{name}' running detached (window open). Log: {}",
            log.display()
        );
        eprintln!("Stop it:  tart stop {name}");
        return Ok(());
    }

    let work_dir = core::config::expand_tilde(work_dir.unwrap_or_else(default_work_dir));
    std::fs::create_dir_all(&work_dir)
        .with_context(|| format!("creating work dir {}", work_dir.display()))?;

    // Content-address the extracted disk by the published zip checksum: each
    // build has a distinct hash and lands in its own folder, so `up` can never
    // reuse a stale disk (the size-only extract skip once served a six-week-old
    // image). A cache hit is a single stat; the 65-byte `.sha256` sidecar tells
    // us which build is current, and `--sha256` pins the key without the fetch.
    let key = resolve_disk_key(url, expected)?;
    let disk = disk_cache_path(&work_dir, &key);

    if disk.is_file() {
        eprintln!("Using cached disk {} ({})", &key[..12], disk.display());
    } else {
        let zip = work_dir.join(format!("{key}.zip"));
        let outcome = download(url, &zip, Some(key.as_str()))?;
        report_download(&outcome, &zip);
        let (extracted, extract_outcome) =
            extract(&zip, disk.parent().expect("cache path has a parent"))?;
        report_extract(&extract_outcome, &extracted);
        // The content-addressed disk is all Tart needs; drop the 2.9 GB zip so
        // the cache holds one artifact per build rather than the zip and disk both.
        let _ = std::fs::remove_file(&zip);
    }

    let mut config = core::config::Config::load()?;
    let (share, read_only) = resolve_share(&config, name, share);

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
            eprintln!(
                "First boot creates '{user}' — log in at the greeter (password = '{user}') or ssh in with your key."
            );
            eprintln!("Set a password of your own: run `bluefin-vm-harden` in the VM.");
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

/// The cache key for the disk: the SHA-256 of the published zip. Prefer an
/// explicit `--sha256`; otherwise read the `<url>.sha256` sidecar published
/// alongside the image. Keying the cache on this is what stops a new build from
/// reusing an old extracted disk.
fn resolve_disk_key(url: &str, flag: Option<&str>) -> Result<String> {
    let raw = match flag {
        Some(h) => h.to_string(),
        None => {
            let sidecar = format!("{url}.sha256");
            core::download::fetch_text(&sidecar)
                .with_context(|| format!("fetching checksum {sidecar}"))?
        }
    };
    parse_sha256(&raw).with_context(|| format!("resolving disk checksum for {url}"))
}

/// Parse a hex SHA-256 from a `--sha256` value or a `.sha256` file body,
/// tolerating the `<hash>  <filename>` form and surrounding whitespace, and
/// normalising to lowercase. Rejecting anything that isn't exactly 64 hex
/// digits also guarantees the result is safe as a path segment (no separators
/// or `..`), which `disk_cache_path` relies on.
fn parse_sha256(raw: &str) -> Result<String> {
    let hash = raw
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(hash)
    } else {
        anyhow::bail!("expected a 64-hex-digit sha256, got {raw:?}");
    }
}

/// The cache location for a build's disk, content-addressed by the zip's
/// SHA-256 (`work_dir/<hash>/disk.raw`). A different build has a different hash
/// and thus a different path, so a stale disk is structurally impossible to
/// reuse. `key` is a validated 64-hex digest (see `parse_sha256`).
fn disk_cache_path(work_dir: &Path, key: &str) -> PathBuf {
    work_dir.join(key).join("disk.raw")
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

/// Resolve the share directory and read-only flag for `name` -- flag >
/// profile > default; read-only is profile-only (no flag yet).
fn resolve_share(
    config: &core::config::Config,
    name: &str,
    flag: Option<PathBuf>,
) -> (PathBuf, bool) {
    let read_only = config
        .profile(name)
        .and_then(|p| p.share.read_only)
        .unwrap_or(false);
    let share = core::config::expand_tilde(
        flag.or_else(|| config.profile(name).and_then(|p| p.share.directory.clone()))
            .unwrap_or_else(default_share),
    );
    (share, read_only)
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
        // The sudo and ssh-password postures are profile-only (set via `tui`),
        // like scale -- carry the saved values through so a CLI run preserves them.
        Ok(core::config::Account {
            user: Some(user),
            ssh_key,
            sudo_password: saved.and_then(|a| a.sudo_password),
            ssh_password_auth: saved.and_then(|a| a.ssh_password_auth),
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
        sudo_password: account.sudo_password.unwrap_or(true),
        ssh_password_auth: account.ssh_password_auth.unwrap_or(true),
        scale,
    })
}

/// Resolve the account for `name` (flag > profile > default) and store it back
/// into `config`, returning the provisioning data. The caller writes it to the
/// share and persists `config`. Scale has no CLI flag -- it's profile-only,
/// set via `bluefin-vm tui`.
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
    use super::{disk_cache_path, parse_sha256};
    use std::path::Path;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[test]
    fn parse_sha256_accepts_bare_and_sidecar_forms() {
        // A bare hash, upper-cased and padded with whitespace, normalises down.
        assert_eq!(
            parse_sha256(&format!("  {}\n", A.to_uppercase())).unwrap(),
            A
        );
        // The `sha256sum` two-column form: take the first token, drop the name.
        assert_eq!(parse_sha256(&format!("{A}  disk.zip\n")).unwrap(), A);
    }

    #[test]
    fn parse_sha256_rejects_non_hex_and_wrong_length() {
        for bad in [
            "",
            "deadbeef",
            &"z".repeat(64),
            &"a".repeat(63),
            &"a".repeat(65),
        ] {
            assert!(parse_sha256(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn a_changed_hash_yields_a_different_cache_path() {
        // The regression guard: a new build (new hash) can never resolve to the
        // same disk as an old one, so a stale disk is impossible to reuse.
        let work = Path::new("/cache");
        assert_ne!(disk_cache_path(work, A), disk_cache_path(work, B));
        assert_eq!(disk_cache_path(work, A), disk_cache_path(work, A));
        assert_eq!(
            disk_cache_path(work, A),
            Path::new("/cache").join(A).join("disk.raw")
        );
    }
}
