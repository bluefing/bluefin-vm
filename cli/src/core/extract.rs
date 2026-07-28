//! Extract the VM disk from the seed zip — UI-agnostic.
//!
//! Like `download`, this takes a `progress` closure rather than printing, so
//! the CLI and a future TUI drive the same code. The disk entry is larger than
//! 4 GiB, so this relies on the zip crate's zip64 support and streams the
//! entry through a fixed buffer — never holding the ~21 GiB image in memory.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use zip::ZipArchive;

const CHUNK: usize = 64 * 1024;

/// Where bootc-image-builder places the raw disk inside the seed zip
/// (alongside `manifest-raw.json`).
const DISK_ENTRY: &str = "image/disk.raw";

/// Extract the raw disk from `zip_path` into `dest_dir`, returning the written
/// path (`dest_dir/disk.raw`).
///
/// `progress(written, total)` is called as bytes are inflated; `total` is the
/// entry's uncompressed size.
pub fn extract_disk(
    zip_path: &Path,
    dest_dir: &Path,
    mut progress: impl FnMut(u64, u64),
) -> Result<PathBuf> {
    let file = File::open(zip_path).with_context(|| format!("opening {}", zip_path.display()))?;
    let mut archive =
        ZipArchive::new(file).with_context(|| format!("reading zip {}", zip_path.display()))?;
    let mut entry = archive
        .by_name(DISK_ENTRY)
        .with_context(|| format!("{DISK_ENTRY} not found in {}", zip_path.display()))?;
    let total = entry.size();

    std::fs::create_dir_all(dest_dir)
        .with_context(|| format!("creating {}", dest_dir.display()))?;
    let dest = dest_dir.join("disk.raw");
    let mut out = File::create(&dest).with_context(|| format!("creating {}", dest.display()))?;

    let mut written = 0u64;
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = entry.read(&mut buf).context("inflating disk from zip")?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).context("writing disk to disk")?;
        written += n as u64;
        progress(written, total);
    }
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    /// Build a seed-shaped zip (nested disk entry + a manifest) at `path`.
    fn write_seed(path: &Path, disk: &[u8]) {
        let file = File::create(path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file(DISK_ENTRY, opts).unwrap();
        zip.write_all(disk).unwrap();
        zip.start_file("manifest-raw.json", opts).unwrap();
        zip.write_all(b"{}").unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn extracts_the_disk_entry() {
        let dir = std::env::temp_dir().join("bluefin-vm-extract-ok");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let zip = dir.join("seed.zip");
        // Repeating bytes so deflate actually compresses (exercises inflate).
        let disk: Vec<u8> = (0u8..=255).cycle().take(50_000).collect();
        write_seed(&zip, &disk);

        let mut last = (0u64, 0u64);
        let out = extract_disk(&zip, &dir, |w, t| last = (w, t)).unwrap();

        assert_eq!(out, dir.join("disk.raw"));
        assert_eq!(std::fs::read(&out).unwrap(), disk);
        // Progress ran to completion against the real uncompressed size.
        assert_eq!(last, (disk.len() as u64, disk.len() as u64));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_disk_entry_errors() {
        let dir = std::env::temp_dir().join("bluefin-vm-extract-missing");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let zip = dir.join("seed.zip");
        // A zip with only the manifest, no disk.
        let file = File::create(&zip).unwrap();
        let mut w = ZipWriter::new(file);
        w.start_file("manifest-raw.json", SimpleFileOptions::default())
            .unwrap();
        w.write_all(b"{}").unwrap();
        w.finish().unwrap();

        assert!(extract_disk(&zip, &dir, |_, _| {}).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
