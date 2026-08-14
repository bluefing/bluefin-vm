//! Import a disk into Tart and run the VM — UI-agnostic.
//!
//! This shells out to the `tart` CLI (the brew formula depends on it). The argv
//! builders are kept separate from execution so they can be unit-tested without
//! tart installed.
//!
//! The single import path: validate the disk is a raw GPT image, recreate the
//! VM, clone the disk in, then set resources. Raw only — the `just build`
//! recipes produce raw and the tool imports the raw it extracts from a seed, so
//! there is no qcow2 conversion here.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

/// VM resources handed to Tart.
pub struct VmSpec {
    pub name: String,
    pub cpu: u32,
    pub memory_mib: u32,
    pub display: String,
    /// Whether to pass `--display-refit` (guest resolution follows the window).
    /// Off means a fixed `display`, which is what lets a chosen resolution and
    /// guest scale hold.
    pub refit: bool,
}

impl VmSpec {
    /// Resolve a spec for `name`, precedence saved profile > built-in default.
    /// The profile (set via `bluefin-vm tui`) is the only source; there is no
    /// env override, so what the config says is what Tart gets.
    pub fn resolve(name: impl Into<String>, saved: Option<&super::config::Resources>) -> Self {
        Self {
            name: name.into(),
            cpu: saved.and_then(|r| r.cpu).unwrap_or(4),
            memory_mib: saved.and_then(|r| r.memory_mib).unwrap_or(4096),
            display: saved
                .and_then(|r| r.display.clone())
                .unwrap_or_else(|| "1920x1200".into()),
            refit: saved.and_then(|r| r.refit).unwrap_or(true),
        }
    }
}

// --- argv builders (pure; unit-tested) --------------------------------------

fn create_args(name: &str) -> Vec<String> {
    vec!["create".into(), "--linux".into(), name.into()]
}

fn set_args(spec: &VmSpec) -> Vec<String> {
    let mut args = vec![
        "set".into(),
        spec.name.clone(),
        "--cpu".into(),
        spec.cpu.to_string(),
        "--memory".into(),
        spec.memory_mib.to_string(),
        "--display".into(),
        spec.display.clone(),
    ];
    // With refit on, Tart resizes the guest to follow the window; with it off,
    // the display stays fixed, which is what makes a chosen resolution and
    // guest scale (monitors.xml) hold.
    if spec.refit {
        args.push("--display-refit".into());
    }
    args
}

/// Args for `tart run`, sharing the durable host directory as `bluefin-share`.
/// `read_only` appends Tart's `:ro` so the guest can't write back; `graphics=false`
/// adds `--no-graphics` for a headless run.
pub fn run_args(name: &str, share: &Path, read_only: bool, graphics: bool) -> Vec<String> {
    let ro = if read_only { ":ro" } else { "" };
    let mut a = vec![
        "run".into(),
        name.into(),
        format!("--dir=bluefin-share:{}{ro}", share.display()),
    ];
    if !graphics {
        a.push("--no-graphics".into());
    }
    a
}

// --- paths ------------------------------------------------------------------

fn tart_home() -> PathBuf {
    if let Some(h) = std::env::var_os("TART_HOME") {
        return PathBuf::from(h);
    }
    let home = std::env::var_os("HOME").expect("HOME is set");
    PathBuf::from(home).join(".tart")
}

/// Where Tart keeps a VM's boot disk.
fn vm_disk_path(name: &str) -> PathBuf {
    tart_home().join("vms").join(name).join("disk.img")
}

/// Whether a local Tart VM of this name exists (its boot disk is on disk).
/// Checked before `up` runs the pipeline: an existing VM is booted, never
/// silently replaced.
pub fn exists(name: &str) -> bool {
    vm_disk_path(name).is_file()
}

// --- disk validation --------------------------------------------------------

/// Reject anything that isn't a raw GPT disk before we destroy the old VM — a
/// bad input must never cost a working VM. Checks the magic bytes: a qcow2
/// header at 0, and the GPT signature at LBA 1 (byte 512).
fn ensure_raw_disk(path: &Path) -> Result<()> {
    let mut f = File::open(path).with_context(|| format!("opening {}", path.display()))?;

    let mut magic = [0u8; 4];
    f.read_exact(&mut magic)
        .with_context(|| format!("reading {}", path.display()))?;
    if magic == [0x51, 0x46, 0x49, 0xfb] {
        bail!(
            "{} is a qcow2 image; this tool imports raw disks only",
            path.display()
        );
    }

    let mut sig = [0u8; 8];
    f.seek(SeekFrom::Start(512))
        .and_then(|_| f.read_exact(&mut sig))
        .with_context(|| format!("reading GPT header of {}", path.display()))?;
    if &sig != b"EFI PART" {
        bail!("{} is not a raw GPT disk image", path.display());
    }
    Ok(())
}

// --- execution --------------------------------------------------------------

fn run_tart(args: &[String]) -> Result<()> {
    let status = Command::new("tart")
        .args(args)
        .status()
        .context("running tart (install it: brew install cirruslabs/cli/tart)")?;
    if !status.success() {
        bail!("`tart {}` failed", args.join(" "));
    }
    Ok(())
}

/// Clone `src` to `dst`. On APFS `cp -c` is an instant, space-free clone; fall
/// back to a full copy where the filesystem can't clone.
fn clone_file(src: &Path, dst: &Path) -> Result<()> {
    let cloned = Command::new("cp")
        .arg("-c")
        .arg(src)
        .arg(dst)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if cloned {
        return Ok(());
    }
    std::fs::copy(src, dst)
        .with_context(|| format!("copying {} to {}", src.display(), dst.display()))?;
    Ok(())
}

/// Import `raw` into a Tart VM named `spec.name`. `replace` carries the
/// deletion policy to the destructive moment: without it an existing VM is an
/// error, so a VM created while the pipeline ran (its download takes minutes)
/// can never be deleted by a check that passed earlier.
pub fn import(raw: &Path, spec: &VmSpec, replace: bool) -> Result<()> {
    ensure_raw_disk(raw)?;

    if exists(&spec.name) {
        if !replace {
            bail!(
                "Tart VM '{}' already exists; replace it with --replace",
                spec.name
            );
        }
        // Delete then create fresh. A part-deleted VM makes delete fail
        // noisily on stderr, so silence it -- the create below is the real
        // guard (it errors if the VM somehow still exists).
        let _ = Command::new("tart")
            .arg("delete")
            .arg(&spec.name)
            .stderr(Stdio::null())
            .status();
    }
    run_tart(&create_args(&spec.name))?;

    let dst = vm_disk_path(&spec.name);
    let vmdir = dst.parent().expect("vm disk has a parent dir");
    if !vmdir.is_dir() {
        bail!("expected Tart VM dir not found: {}", vmdir.display());
    }
    clone_file(raw, &dst)?;

    run_tart(&set_args(spec))?;
    Ok(())
}

/// Start the VM detached: spawn `tart run` with its output to `log`, wait
/// `settle` for it to fail fast, then leave it running. Returns the log path.
pub fn run_detached(
    name: &str,
    share: &Path,
    read_only: bool,
    log: &Path,
    settle: std::time::Duration,
) -> Result<PathBuf> {
    std::fs::create_dir_all(share)
        .with_context(|| format!("creating share dir {}", share.display()))?;
    let out = File::create(log).with_context(|| format!("creating log {}", log.display()))?;
    let err = out.try_clone().context("cloning log handle")?;

    let mut child = Command::new("tart")
        .args(run_args(name, share, read_only, true))
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err))
        .spawn()
        .context("running tart (install it: brew install cirruslabs/cli/tart)")?;

    // A startup failure (bad VM, tart error) shows up as an early exit; surface
    // it instead of reporting a VM that isn't actually running.
    std::thread::sleep(settle);
    if let Some(status) = child.try_wait().context("checking tart process")? {
        let output = std::fs::read_to_string(log).unwrap_or_default();
        bail!("tart run exited at startup ({status}):\n{output}");
    }
    Ok(log.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_prefers_profile_then_default() {
        // No profile -> built-in defaults.
        let d = VmSpec::resolve("v", None);
        assert_eq!(
            (d.cpu, d.memory_mib, d.display.as_str()),
            (4, 4096, "1920x1200")
        );

        // The profile fills what it sets; unset fields fall to the defaults.
        let res = crate::core::config::Resources {
            cpu: Some(8),
            memory_mib: None,
            display: Some("2560x1600".into()),
            scale: None,
            refit: None,
        };
        let p = VmSpec::resolve("v", Some(&res));
        assert_eq!(
            (p.cpu, p.memory_mib, p.display.as_str()),
            (8, 4096, "2560x1600")
        );
        assert!(p.refit); // unset -> refit on (the default)
        assert!(VmSpec::resolve("v", None).refit);
    }

    #[test]
    fn create_args_are_minimal() {
        assert_eq!(create_args("Bluefin"), ["create", "--linux", "Bluefin"]);
    }

    #[test]
    fn set_args_carry_resources_and_refit() {
        let spec = VmSpec {
            name: "Bluefin".into(),
            cpu: 6,
            memory_mib: 8192,
            display: "2560x1600".into(),
            refit: true,
        };
        assert_eq!(
            set_args(&spec),
            [
                "set",
                "Bluefin",
                "--cpu",
                "6",
                "--memory",
                "8192",
                "--display",
                "2560x1600",
                "--display-refit",
            ]
        );
    }

    #[test]
    fn set_args_omit_refit_when_off() {
        let spec = VmSpec {
            name: "Bluefin".into(),
            cpu: 6,
            memory_mib: 8192,
            display: "2560x1600".into(),
            refit: false,
        };
        // A fixed display and no --display-refit -- what lets the resolution hold.
        assert!(!set_args(&spec).contains(&"--display-refit".to_string()));
        assert!(set_args(&spec).contains(&"2560x1600".to_string()));
    }

    #[test]
    fn run_args_attach_the_share_and_toggle_graphics() {
        let share = Path::new("/Users/x/bluefin-share");
        assert_eq!(
            run_args("Bluefin", share, false, true),
            [
                "run",
                "Bluefin",
                "--dir=bluefin-share:/Users/x/bluefin-share",
            ]
        );
        // Read-only appends Tart's :ro suffix.
        assert_eq!(
            run_args("Bluefin", share, true, true)[2],
            "--dir=bluefin-share:/Users/x/bluefin-share:ro"
        );
        // Headless adds --no-graphics.
        assert_eq!(
            run_args("Bluefin", share, false, false).last().unwrap(),
            "--no-graphics"
        );
    }

    #[test]
    fn ensure_raw_disk_accepts_gpt_and_rejects_others() {
        let dir = std::env::temp_dir().join("bluefin-vm-tart-disk");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // A raw GPT disk: "EFI PART" at byte 512 (LBA 1).
        let mut raw = vec![0u8; 520];
        raw[512..520].copy_from_slice(b"EFI PART");
        let raw_path = dir.join("disk.raw");
        std::fs::write(&raw_path, &raw).unwrap();
        assert!(ensure_raw_disk(&raw_path).is_ok());

        // qcow2 magic at byte 0.
        let qcow = dir.join("disk.qcow2");
        std::fs::write(&qcow, [0x51, 0x46, 0x49, 0xfb, 0, 0, 0, 0]).unwrap();
        assert!(ensure_raw_disk(&qcow).is_err());

        // Neither: no GPT signature.
        let blank = dir.join("blank.img");
        std::fs::write(&blank, vec![0u8; 1024]).unwrap();
        assert!(ensure_raw_disk(&blank).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vm_disk_path_honours_tart_home_and_import_refuses_to_replace() {
        // One test owns TART_HOME: vm_disk_path/exists read it, and setting
        // it from parallel tests would race.
        let prev = std::env::var_os("TART_HOME");
        let home = std::env::temp_dir().join("bluefin-vm-tart-home");
        let _ = std::fs::remove_dir_all(&home);
        std::env::set_var("TART_HOME", &home);

        assert_eq!(vm_disk_path("Bluefin"), home.join("vms/Bluefin/disk.img"));
        assert!(!exists("Bluefin"));

        // With a VM present, import without the replace policy must fail
        // before anything destructive -- the check lives at the point of
        // deletion, not at `up`'s entry (the pipeline runs for minutes).
        std::fs::create_dir_all(home.join("vms/Bluefin")).unwrap();
        std::fs::write(home.join("vms/Bluefin/disk.img"), b"").unwrap();
        assert!(exists("Bluefin"));

        let raw = home.join("disk.raw");
        let mut blob = vec![0u8; 520];
        blob[512..520].copy_from_slice(b"EFI PART");
        std::fs::write(&raw, &blob).unwrap();
        let spec = VmSpec::resolve("Bluefin", None);
        let err = import(&raw, &spec, false).unwrap_err();
        assert!(err.to_string().contains("already exists"), "{err}");
        // The VM's disk survived the refused import.
        assert!(exists("Bluefin"));

        let _ = std::fs::remove_dir_all(&home);
        match prev {
            Some(v) => std::env::set_var("TART_HOME", v),
            None => std::env::remove_var("TART_HOME"),
        }
    }
}
