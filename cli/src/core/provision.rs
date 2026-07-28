//! Write first-boot provisioning data into the host share — UI-agnostic.
//!
//! The guest oneshot (`image/provision.sh`) reads these files on first boot to
//! create the user's account, then deletes them. Credential model: public key
//! only, no password (see that script and README "First-boot provisioning").

use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};

/// Sub-directory of the share the guest reads (and clears) on first boot.
const DIR: &str = ".bluefin-vm";

/// What to provision into the VM.
pub struct Provision {
    pub username: String,
    /// One or more ssh public keys (the authorized_keys file's contents).
    pub authorized_keys: String,
    pub autologin: bool,
}

/// A valid Linux account name, matching the guest script's own check: lowercase
/// start, `[a-z0-9_-]`, ≤32 chars. Validated host-side too so a bad name fails
/// before boot with a clear message rather than in a boot log.
pub fn valid_username(name: &str) -> bool {
    let mut chars = name.chars();
    let first_ok = matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c == '_');
    first_ok
        && name.len() <= 32
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Write the provision files into `share/.bluefin-vm/`.
pub fn write(share: &Path, p: &Provision) -> Result<()> {
    if !valid_username(&p.username) {
        bail!(
            "invalid username '{}': use lowercase letters, digits, '-' or '_' \
             (start with a letter or '_'), max 32 chars",
            p.username
        );
    }

    let dir = share.join(DIR);
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    fs::write(dir.join("username"), format!("{}\n", p.username)).context("writing username")?;
    fs::write(dir.join("authorized_keys"), &p.authorized_keys)
        .context("writing authorized_keys")?;

    // Presence is the flag; clear a stale one when autologin is off.
    let autologin = dir.join("autologin");
    if p.autologin {
        fs::write(&autologin, "").context("writing autologin")?;
    } else {
        let _ = fs::remove_file(&autologin);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_and_rejects_usernames() {
        for ok in ["alice", "bob99", "_svc", "a-b_c"] {
            assert!(valid_username(ok), "{ok} should be valid");
        }
        for bad in ["", "1abc", "Alice", "a b", "root!", &"x".repeat(33)] {
            assert!(!valid_username(bad), "{bad} should be invalid");
        }
    }

    #[test]
    fn writes_the_expected_files() {
        let share = std::env::temp_dir().join("bluefin-vm-provision-on");
        let _ = fs::remove_dir_all(&share);
        write(
            &share,
            &Provision {
                username: "alice".into(),
                authorized_keys: "ssh-ed25519 AAAAKEY alice@mac\n".into(),
                autologin: true,
            },
        )
        .unwrap();
        let dir = share.join(DIR);
        assert_eq!(fs::read_to_string(dir.join("username")).unwrap(), "alice\n");
        assert_eq!(
            fs::read_to_string(dir.join("authorized_keys")).unwrap(),
            "ssh-ed25519 AAAAKEY alice@mac\n"
        );
        assert!(dir.join("autologin").exists());
        let _ = fs::remove_dir_all(&share);
    }

    #[test]
    fn autologin_off_clears_a_stale_flag() {
        let share = std::env::temp_dir().join("bluefin-vm-provision-off");
        let _ = fs::remove_dir_all(&share);
        let flag = share.join(DIR).join("autologin");
        // Pre-existing flag from an earlier provision...
        fs::create_dir_all(share.join(DIR)).unwrap();
        fs::write(&flag, "").unwrap();
        write(
            &share,
            &Provision {
                username: "bob".into(),
                authorized_keys: String::new(),
                autologin: false,
            },
        )
        .unwrap();
        // ...is removed when autologin is off.
        assert!(!flag.exists());
        let _ = fs::remove_dir_all(&share);
    }

    #[test]
    fn invalid_username_bails_before_writing() {
        let share = std::env::temp_dir().join("bluefin-vm-provision-bad");
        let _ = fs::remove_dir_all(&share);
        let r = write(
            &share,
            &Provision {
                username: "Root!".into(),
                authorized_keys: String::new(),
                autologin: true,
            },
        );
        assert!(r.is_err());
        let _ = fs::remove_dir_all(&share);
    }
}
