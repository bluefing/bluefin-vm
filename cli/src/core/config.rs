//! Persisted per-VM settings, keyed by VM name -- UI-agnostic.
//!
//! Extensibility is the point. Options are grouped into category sub-structs
//! (`Account`, `Resources`, ...) rather than a flat bag, and every setting is an
//! `Option`:
//!
//! - a new *option* is one `Option` field on its category;
//! - a new *category* (flavour, advanced, ...) is one field on `Profile`.
//!
//! Old config files simply miss the new keys and fall back to defaults, and
//! unknown keys from a newer file are ignored rather than rejected -- so the
//! schema grows without breaking existing configs (tests below pin both).
//!
//! Front-ends resolve a final value as `flag > profile > built-in default`;
//! this module only stores and returns the profile layer.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Account identity applied by first-boot provisioning.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Account {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_key: Option<SshKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autologin: Option<bool>,
}

/// The account's authorised ssh key. `None` on `Account` means "not set --
/// fall back to auto-detecting `~/.ssh/*.pub`"; `Disabled` is the explicit
/// choice to skip key injection, which must survive as its own state rather
/// than collapsing back into "not set" (that would silently re-enable
/// auto-detection on the next resolve).
///
/// Serialised as a bare string -- the path itself, or the literal `disabled`
/// -- so it stays a drop-in read of the plain-path format older builds wrote
/// (`ssh_key = "/home/me/.ssh/id.pub"`) instead of forcing a tagged table.
#[derive(Debug, Clone, PartialEq)]
pub enum SshKey {
    Path(PathBuf),
    Disabled,
}

impl Serialize for SshKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            SshKey::Path(p) => p.serialize(serializer),
            SshKey::Disabled => serializer.serialize_str("disabled"),
        }
    }
}

impl<'de> Deserialize<'de> for SshKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let s = PathBuf::deserialize(deserializer)?;
        Ok(if s == Path::new("disabled") {
            SshKey::Disabled
        } else {
            SshKey::Path(s)
        })
    }
}

/// Host folder mapped into the guest over virtiofs (the `bluefin-share` mount).
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Share {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
}

/// Hardware sizing handed to Tart, plus the guest display scale. `scale` is
/// the odd one out: Linux scanout is raw pixels (no `tart set` equivalent of
/// macOS's HiDPI points -- see docs/content/just/tart.md), so it isn't handed to
/// Tart at all. It's applied by first-boot provisioning instead, like the
/// account fields.
///
/// `refit` and `display`/`scale` are mutually exclusive by design: with refit
/// on (the default) Tart continuously resizes the guest to the window, so a
/// fixed `display` can't hold and a baked `scale` can't match the live mode.
/// Turning refit off is what makes a chosen resolution and scale stick; the
/// `setup` TUI enforces this, only letting you set `display`/`scale` when
/// `refit` is off.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Resources {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_mib: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    /// Guest desktop scale as a percentage (100 or 200); `None` leaves
    /// GNOME's own default alone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<u32>,
    /// Whether Tart resizes the guest display to follow the window
    /// (`--display-refit`); `None` means the built-in default (on).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refit: Option<bool>,
}

/// Everything remembered for one named VM. Further option groups slot in here
/// as more `#[serde(default)]` category fields; nothing else needs to change.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default, skip_serializing_if = "is_default")]
    pub account: Account,
    #[serde(default, skip_serializing_if = "is_default")]
    pub share: Share,
    #[serde(default, skip_serializing_if = "is_default")]
    pub resources: Resources,
}

/// The whole config: VM name -> profile. `BTreeMap` keeps the written file
/// deterministic (sorted by name).
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub vms: BTreeMap<String, Profile>,
}

/// `skip_serializing_if` helper: omit a category that's entirely unset, so an
/// empty `[vms.NAME.account]` table never lands in the file.
fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set -- can't locate the config file")
}

/// If the path starts with `~`, expand it to the user's home directory.
/// Left as-is if `$HOME` can't be read; a stray `~` is a softer failure than
/// aborting a path-taking command over it.
pub fn expand_tilde(path: PathBuf) -> PathBuf {
    if !path.starts_with("~") {
        return path;
    }
    let Ok(home_dir) = home() else {
        return path;
    };
    if path == Path::new("~") {
        home_dir
    } else if let Ok(suffix) = path.strip_prefix("~") {
        home_dir.join(suffix)
    } else {
        path
    }
}

impl Config {
    /// `$XDG_CONFIG_HOME/bluefin-vm/config.toml`, else
    /// `~/.config/bluefin-vm/config.toml`.
    pub fn path() -> Result<PathBuf> {
        let base = match std::env::var_os("XDG_CONFIG_HOME") {
            Some(v) => PathBuf::from(v),
            None => home()?.join(".config"),
        };
        Ok(base.join("bluefin-vm").join("config.toml"))
    }

    /// Load from the default path, or an empty config if the file is absent.
    pub fn load() -> Result<Self> {
        Self::load_from(&Self::path()?)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    /// Write to the default path, creating parent directories.
    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path()?)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let text = toml::to_string_pretty(self).context("serialising config")?;
        std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))
    }

    /// The stored profile for `name`, if any.
    pub fn profile(&self, name: &str) -> Option<&Profile> {
        self.vms.get(name)
    }

    /// Insert or replace the profile for `name`.
    pub fn set_profile(&mut self, name: impl Into<String>, profile: Profile) {
        self.vms.insert(name.into(), profile);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Config {
        let mut cfg = Config::default();
        cfg.set_profile(
            "Bluefin",
            Profile {
                account: Account {
                    user: Some("alice".into()),
                    ssh_key: Some(SshKey::Path("/home/alice/.ssh/id_ed25519.pub".into())),
                    autologin: Some(true),
                },
                share: Share {
                    directory: Some("/Users/alice/bluefin-share".into()),
                    read_only: Some(false),
                },
                resources: Resources {
                    cpu: Some(4),
                    memory_mib: Some(4096),
                    display: Some("1920x1200".into()),
                    scale: Some(200),
                    refit: Some(false),
                },
            },
        );
        // A second VM with a *different* key and only some fields set.
        cfg.set_profile(
            "work",
            Profile {
                account: Account {
                    ssh_key: Some(SshKey::Path("/home/alice/.ssh/id_work.pub".into())),
                    ..Default::default()
                },
                share: Share {
                    directory: Some("/Users/alice/work-share".into()),
                    read_only: Some(true),
                },
                resources: Resources {
                    cpu: Some(8),
                    ..Default::default()
                },
            },
        );
        cfg
    }

    #[test]
    fn round_trips_named_profiles_with_distinct_keys() {
        let dir = std::env::temp_dir().join("bluefin-vm-config-roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("config.toml");

        let cfg = sample();
        cfg.save_to(&path).unwrap();
        assert_eq!(Config::load_from(&path).unwrap(), cfg);
        // Each VM keeps its own ssh key and share -- the point of naming profiles.
        assert_ne!(
            cfg.profile("Bluefin").unwrap().account.ssh_key,
            cfg.profile("work").unwrap().account.ssh_key
        );
        assert_ne!(
            cfg.profile("Bluefin").unwrap().share.directory,
            cfg.profile("work").unwrap().share.directory
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_loads_empty() {
        let path = std::env::temp_dir().join("bluefin-vm-config-absent/config.toml");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
        assert!(Config::load_from(&path).unwrap().vms.is_empty());
    }

    #[test]
    fn unset_fields_and_categories_are_omitted() {
        let mut cfg = Config::default();
        cfg.set_profile(
            "x",
            Profile {
                resources: Resources {
                    cpu: Some(2),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let text = toml::to_string_pretty(&cfg).unwrap();
        assert!(text.contains("cpu = 2"));
        assert!(!text.contains("memory_mib")); // None omitted
        assert!(!text.contains("account")); // empty category omitted
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.profile("x").unwrap().resources.cpu, Some(2));
        assert_eq!(back.profile("x").unwrap().account.user, None);
    }

    #[test]
    fn unknown_keys_from_a_newer_version_are_ignored() {
        // A file a future build wrote, with an option and a category we don't
        // know yet. Extensibility contract: parse, don't reject.
        let text = r#"
[vms.Bluefin.account]
user = "alice"
future_option = "ignored"

[vms.Bluefin.flavour]
name = "bluefin-dx"
"#;
        let cfg: Config = toml::from_str(text).unwrap();
        assert_eq!(
            cfg.profile("Bluefin").unwrap().account.user,
            Some("alice".into())
        );
    }

    #[test]
    fn ssh_key_reads_the_bare_path_format_written_before_disabled_existed() {
        let text = r#"
[vms.Bluefin.account]
ssh_key = "/home/alice/.ssh/id_ed25519.pub"
"#;
        let cfg: Config = toml::from_str(text).unwrap();
        assert_eq!(
            cfg.profile("Bluefin").unwrap().account.ssh_key,
            Some(SshKey::Path("/home/alice/.ssh/id_ed25519.pub".into()))
        );
    }

    #[test]
    fn ssh_key_disabled_round_trips_as_a_plain_string() {
        let mut cfg = Config::default();
        cfg.set_profile(
            "x",
            Profile {
                account: Account {
                    ssh_key: Some(SshKey::Disabled),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let text = toml::to_string_pretty(&cfg).unwrap();
        assert!(text.contains(r#"ssh_key = "disabled""#));
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(
            back.profile("x").unwrap().account.ssh_key,
            Some(SshKey::Disabled)
        );
    }

    #[test]
    fn path_honours_xdg_config_home() {
        let prev = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("XDG_CONFIG_HOME", "/tmp/xdg-test");
        assert_eq!(
            Config::path().unwrap(),
            PathBuf::from("/tmp/xdg-test/bluefin-vm/config.toml")
        );
        match prev {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }

    #[test]
    fn expand_tilde_resolves_home_correctly() {
        let home_dir = home().unwrap();
        assert_eq!(expand_tilde(PathBuf::from("~")), home_dir);
        assert_eq!(expand_tilde(PathBuf::from("~/foo")), home_dir.join("foo"));
        assert_eq!(
            expand_tilde(PathBuf::from("/absolute/path")),
            PathBuf::from("/absolute/path")
        );
    }
}
