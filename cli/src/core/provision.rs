//! Write first-boot provisioning data into the host share — UI-agnostic.
//!
//! The guest oneshot (`image/provision.sh`) reads these files on first boot to
//! create the user's account, then deletes them. Credential model: public key
//! plus a login password of `password == username` (a public convention, not a
//! secret); `sudo` prompts and ssh password login is on unless a flag file says
//! otherwise (see that script and `docs/PROVISIONING.md`).

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
    /// Whether `sudo` prompts for a password; default `true`. `false` writes a
    /// `NOPASSWD` sudoers rule (passwordless).
    pub sudo_password: bool,
    /// Allow ssh password login; default `true` (as the base image ships).
    /// `false` writes an sshd drop-in disabling it.
    pub ssh_password_auth: bool,
    /// Guest desktop scale target as a percentage; the guest snaps it to the
    /// nearest scale the display supports. `None` leaves GNOME's own default
    /// alone.
    pub scale: Option<u32>,
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

    // One rule for both postures: a `true` (on) field is the default and writes
    // nothing; a `false` writes the non-default flag file the guest acts on
    // (and a stale one is cleared, so it never lingers into a later provision).
    let passwordless_sudo = dir.join("passwordless-sudo");
    if !p.sudo_password {
        fs::write(&passwordless_sudo, "").context("writing passwordless-sudo")?;
    } else {
        let _ = fs::remove_file(&passwordless_sudo);
    }
    let disable_ssh_password = dir.join("disable-ssh-password");
    if !p.ssh_password_auth {
        fs::write(&disable_ssh_password, "").context("writing disable-ssh-password")?;
    } else {
        let _ = fs::remove_file(&disable_ssh_password);
    }

    // Same pattern as the flags above, but with content: the value is the file
    // body, cleared when unset so a stale scale from an earlier provision
    // doesn't linger.
    let scale = dir.join("scale");
    match p.scale {
        Some(pct) => fs::write(&scale, format!("{pct}\n")).context("writing scale")?,
        None => {
            let _ = fs::remove_file(&scale);
        }
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
                sudo_password: false,
                ssh_password_auth: false,
                scale: Some(200),
            },
        )
        .unwrap();
        let dir = share.join(DIR);
        assert_eq!(fs::read_to_string(dir.join("username")).unwrap(), "alice\n");
        assert_eq!(
            fs::read_to_string(dir.join("authorized_keys")).unwrap(),
            "ssh-ed25519 AAAAKEY alice@mac\n"
        );
        assert!(dir.join("passwordless-sudo").exists());
        assert!(dir.join("disable-ssh-password").exists());
        assert_eq!(fs::read_to_string(dir.join("scale")).unwrap(), "200\n");
        let _ = fs::remove_dir_all(&share);
    }

    #[test]
    fn default_flags_clear_stale_files() {
        let share = std::env::temp_dir().join("bluefin-vm-provision-off");
        let _ = fs::remove_dir_all(&share);
        let dir = share.join(DIR);
        fs::create_dir_all(&dir).unwrap();
        // Pre-existing flags from an earlier provision...
        fs::write(dir.join("passwordless-sudo"), "").unwrap();
        fs::write(dir.join("disable-ssh-password"), "").unwrap();
        write(
            &share,
            &Provision {
                username: "bob".into(),
                authorized_keys: String::new(),
                sudo_password: true,
                ssh_password_auth: true,
                scale: None,
            },
        )
        .unwrap();
        // ...are cleared for a default account (sudo prompts, ssh password on).
        assert!(!dir.join("passwordless-sudo").exists());
        assert!(!dir.join("disable-ssh-password").exists());
        let _ = fs::remove_dir_all(&share);
    }

    #[test]
    fn unset_scale_clears_a_stale_file() {
        let share = std::env::temp_dir().join("bluefin-vm-provision-scale-off");
        let _ = fs::remove_dir_all(&share);
        let scale = share.join(DIR).join("scale");
        // Pre-existing scale from an earlier provision...
        fs::create_dir_all(share.join(DIR)).unwrap();
        fs::write(&scale, "200\n").unwrap();
        write(
            &share,
            &Provision {
                username: "bob".into(),
                authorized_keys: String::new(),
                sudo_password: true,
                ssh_password_auth: true,
                scale: None,
            },
        )
        .unwrap();
        // ...is removed once the profile no longer sets a scale.
        assert!(!scale.exists());
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
                sudo_password: true,
                ssh_password_auth: true,
                scale: None,
            },
        );
        assert!(r.is_err());
        let _ = fs::remove_dir_all(&share);
    }
}
