//! Import a disk into Tart and run the VM — UI-agnostic.
//!
//! This shells out to the `tart` CLI (the brew formula depends on it). The argv
//! builders are kept separate from execution so they can be unit-tested without
//! tart installed — the same split `create-vm.sh` gets from its `-n` dry run.
//!
//! Ports `bin/create-vm.sh`: validate the disk is a raw GPT image, recreate the
//! VM, clone the disk in, then set resources. Unlike the script it does not
//! convert qcow2 — the tool only ever imports the raw it extracts from a seed.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

/// VM resources. Defaults and env overrides mirror `create-vm.sh`.
pub struct VmSpec {
    pub name: String,
    pub cpu: u32,
    pub memory_mib: u32,
    pub display: String,
}

impl VmSpec {
    /// Build a spec for `name`, reading TART_CPU / TART_MEM / TART_DISPLAY.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            cpu: env_u32("TART_CPU", 4),
            memory_mib: env_u32("TART_MEM", 4096),
            display: std::env::var("TART_DISPLAY").unwrap_or_else(|_| "1920x1200".into()),
        }
    }
}

fn env_u32(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

// --- argv builders (pure; unit-tested) --------------------------------------

fn create_args(name: &str) -> Vec<String> {
    vec!["create".into(), "--linux".into(), name.into()]
}

fn set_args(spec: &VmSpec) -> Vec<String> {
    vec![
        "set".into(),
        spec.name.clone(),
        "--cpu".into(),
        spec.cpu.to_string(),
        "--memory".into(),
        spec.memory_mib.to_string(),
        // 16:10 default; --display-refit lets the guest resolution follow window
        // resizes/fullscreen (GNOME + virtio-gpu honours it).
        "--display".into(),
        spec.display.clone(),
        "--display-refit".into(),
    ]
}

/// Args for `tart run`, sharing the durable host folder as `bluefin-share`.
/// `graphics=false` adds `--no-graphics` for a headless run.
pub fn run_args(name: &str, share: &Path, graphics: bool) -> Vec<String> {
    let mut a = vec![
        "run".into(),
        name.into(),
        format!("--dir=bluefin-share:{}", share.display()),
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

// --- disk validation --------------------------------------------------------

/// Reject anything that isn't a raw GPT disk before we destroy the old VM — a
/// bad input must never cost a working VM. Mirrors create-vm.sh's magic-byte
/// checks (qcow2 header at 0, GPT signature at LBA 1 = byte 512).
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

/// Import `raw` into a Tart VM named `spec.name`, replacing any existing VM.
pub fn import(raw: &Path, spec: &VmSpec) -> Result<()> {
    ensure_raw_disk(raw)?;

    // Replace the VM: delete then create fresh. A missing VM makes delete fail
    // noisily on stderr, so silence it -- the create below is the real guard
    // (it errors if the VM somehow still exists).
    let _ = Command::new("tart")
        .arg("delete")
        .arg(&spec.name)
        .stderr(Stdio::null())
        .status();
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
    log: &Path,
    settle: std::time::Duration,
) -> Result<PathBuf> {
    std::fs::create_dir_all(share)
        .with_context(|| format!("creating share dir {}", share.display()))?;
    let out = File::create(log).with_context(|| format!("creating log {}", log.display()))?;
    let err = out.try_clone().context("cloning log handle")?;

    let mut child = Command::new("tart")
        .args(run_args(name, share, true))
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
    fn run_args_attach_the_share_and_toggle_graphics() {
        let share = Path::new("/Users/x/bluefin-share");
        assert_eq!(
            run_args("Bluefin", share, true),
            [
                "run",
                "Bluefin",
                "--dir=bluefin-share:/Users/x/bluefin-share",
            ]
        );
        // Headless adds --no-graphics.
        assert_eq!(
            run_args("Bluefin", share, false).last().unwrap(),
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
    fn vm_disk_path_honours_tart_home() {
        // vm_disk_path reads TART_HOME; the guard keeps it from leaking to
        // other tests that also touch process env.
        let prev = std::env::var_os("TART_HOME");
        std::env::set_var("TART_HOME", "/tmp/tart-test-home");
        assert_eq!(
            vm_disk_path("Bluefin"),
            PathBuf::from("/tmp/tart-test-home/vms/Bluefin/disk.img")
        );
        match prev {
            Some(v) => std::env::set_var("TART_HOME", v),
            None => std::env::remove_var("TART_HOME"),
        }
    }
}
