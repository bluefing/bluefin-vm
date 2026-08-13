# Backlog

Internal follow-up tracker. Not part of the rendered docs site. Each item has
enough context to pick up cold: what, why, where to start.

The legacy `docs/BACKLOG.md` and `docs/ROADMAP.md` still need folding in here —
tracked in `migration.md`. What's below is the TUI/CLI work captured during
design review.

## Command surface

The `config` namespace is designed in `../design/command-surface.md`; this is
the implementation slice. (The `setup` → `tui` rename from the same design is
done.)

### Add `bv config init|path|show`

Add a `Config` subcommand:
`init` scaffolds `~/.config/bluefin-vm/config.toml` with a default profile and
**never clobbers** an existing file (print its path and exit; `--force` to
overwrite); `path` prints the resolved path; `show` prints the current config.
Start in `main.rs` (the `Command` enum) and `core/config.rs` (a
save-if-absent + a default-profile constructor).

### `just tart ssh` should use the profile's user and key

Today the recipe runs `ssh $USER@<ip>` with default key discovery, so it breaks
whenever the profile differs from the host: a provisioned `usera` isn't reached
by host `$USER` (`bluefing`), and a non-default key name (e.g. a FIDO
`id_ed25519_sk_*`) is never offered, so it falls to a password — which fails once
`ssh_password_auth` is off. Read the VM's profile: default the target to
`account.user` (fall back to `$USER`) and pass `-i account.ssh_key` when set.
Surfaced during drop-autologin boot testing.

### Lift the image URL into config

`DEFAULT_SEED_URL` is a hardcoded `const` in `main.rs`
(`https://disks.bluefing.net/bluefin-vm-raw-arm64.zip`), which is really config.
Make it a profile/global setting with the const as the fallback default, and
pair it with the published `.sha256` so `up`/`download` verify the checksum by
default rather than only on `--sha256`. Fits the `bv config` work.

### Rename the legacy `seed` terminology to `image`

"seed" is non-standard for the downloadable prebuilt disk and doesn't land with a
cloud-native audience (they read it as database/torrent/terraform seeding). New
content already uses `image`/`disk image`; the legacy term still spans ~54 spots
(`DEFAULT_SEED_URL`, `core/extract.rs`, `core/mod.rs`, tests,
comments). Rename them in one mechanical pass, keeping the artifact filename
(`bluefin-vm-raw-arm64.zip`) unchanged. Say "disk image" where it could be
confused with the bootc container image.

### Warn when the share is the home directory

The share is mounted into the VM as `bluefin-share`, so a profile with
`share.directory = "~/"` exposes the user's entire `$HOME` to the guest. The
built-in default is already `~/bluefin-share`, but nothing stops the TUI/config
being set to `~/` (or another broad path), and provisioning then writes its
flags to `~/.bluefin-vm` rather than the dedicated share. Warn (or refuse) when
the resolved share is `$HOME` or an ancestor of it, and keep the TUI default at
`~/bluefin-share`. Surfaced during content-addressed-cache testing, where `up`
reported `Share: /Users/<user>`.

## TUI as a control surface

The TUI graduates from a one-shot profile editor into a front-end that also
drives the VM. The core ops are already UI-agnostic (`core/tart.rs`,
`core/provision.rs`), so the bulk of the work is running long operations inside
the ratatui event loop without freezing it (a worker thread + status channel),
not new core logic.

### Run the pipeline inside the TUI

The Up button exists: it saves the profile, closes the TUI, and hands off to
the same `up` the CLI runs, with the pipeline's output on the plain terminal.
The next step is keeping the TUI open through the run -- the worker-thread +
status-channel plumbing above, with the pipeline's progress rendered in-form.
This unlocks progress reporting below and further verbs (stop, replace).

### Cycle through machine configs

A picker in the TUI to switch between the named profiles in `config.toml`.
`config.rs` already keys profiles by name (what `tui --name` edits); this is the
selection UI over them.

### Progress bars for build / provision

Show progress for the long steps. Depends on the core ops emitting progress:
download reports bytes cleanly and extract can too; the bootc disk build only
gives coarse stages, so expect a mix of a real bar and stage labels. Pairs with
the worker-thread work above.

## Idempotency

### Config-hash to skip re-provisioning

Hash the resolved profile + provisioning inputs, store the hash, and only
re-arm first-boot provisioning when it changes — "no config change ⇒ reuse the
existing VM." Extends the idempotency already in `extract.rs` (its
`Extracted` / `AlreadyExtracted` size-skip). Note the interaction with the guest
gate: provisioning is gated on `ConditionPathExists=.../username` and only runs
at boot, so the hash decides *whether to re-arm* that gate, and taking effect
still needs a reboot.

The motivating case: edit the profile in the TUI (say a new username), press
Up, and the existing VM boots with the old account — the change silently does
nothing. `up` prints a blanket warning on the boot-existing path today; the
hash turns that into a real drift check that warns only when the profile
actually differs from what the VM was provisioned with.

## Image

### Remove or hide the baked `bluefin` account from the greeter

The `bluefin` test account comes from `config.toml` (the unpatched build layer),
so it sits on the greeter alongside the provisioned user on every disk. Options,
each with a trade-off: drop it from `config.toml` (but a `--no-provision` disk
then has no login at all); have provisioning lock/remove it once the real account
exists; or keep it but hide it from the greeter (`AccountsService`
`SystemAccount=true` / a `NotShowIn`). Surfaced during drop-autologin boot
testing.

## Testing

### Integration test framework for config-variation behaviour

Unit tests cover the host-side writer and the TUI; the guest-side `provision.sh`
(account, password, sudoers/sshd drop-ins, scale) has no automated coverage, and
as the toggles multiply the config → behaviour matrix gets hard to check by hand.
Add a layered harness:

- **Tier 1 — fast, every PR, no VM.** `provision.sh` runs cleanly in a plain
  Linux container (its VM-only steps are already guarded: `restorecon` on
  `/sys/fs/selinux/enforce`, the sshd reload with `|| true`, the scale block
  degrades when DRM debugfs is absent). Drop synthetic `…/.bluefin-vm/` files
  into a container, run the script as root, and assert the account/filesystem
  result — user in `wheel`, `authorized_keys` perms, `password == username` in
  `/etc/shadow`, the sudoers drop-in present *iff* `passwordless-sudo`, the sshd
  drop-in present *iff* `disable-ssh-password`, share cleared. Matrix over the
  flag combinations. Harness: bats + podman/docker, gated behind a
  `just test-integration` recipe so plain `just test` stays offline.
- **Tier 2 — slow, on-demand / nightly, Apple Silicon only.** Full
  `up-provisioned` boot plus assertions over ssh (extend
  `tests/e2e/guest-checks.sh`) for what a container can't verify: systemd
  ordering (`Before=gdm`), SELinux *enforcing*, sshd honouring the drop-in live,
  GUI/polkit. A few representative configs, not the full matrix — each is a real
  boot.

One enabler: make `provision.sh`'s `pdir` overridable
(`pdir="${BLUEFIN_VM_PDIR:-/var/mnt/…}"`) so a test needn't write under
`/var/mnt`. Start Tier 1 on a `fedora` container; add a Bluefin-base variant for
the nightly.

## Host download cache

### Prune old content-addressed builds

`up` caches each build's disk at `~/.cache/bluefin-vm/<zip-sha256>/disk.raw`, so
every new upstream build adds ~21 GiB and older ones linger. `just
cli::purge-cache` clears the whole cache, but nothing drops only the stale
builds. Add a prune that keeps the current build (the sha the published
`.sha256` resolves to) and removes the rest -- a `--prune` on `up`, or a
`cli::prune-cache` recipe. Needs the CLI to expose the current key, so it pairs
with the cache-path / `bv config` work.

## Disk size

### Support resizing the disk

The disk is built at a deliberately modest default (`minsize = "20 GiB"` in
`config.toml`) — small enough not to waste space, but needs vary widely: a few
text files ask for almost nothing, local AI models ask for a lot. So the disk
must be growable after the fact; a fixed 20 GiB doesn't fit everyone.

Mechanism: `tart set <vm> --disk-size <GB>` grows the disk on the host (grow-only
— it can't shrink), then the guest grows its partition and filesystem
(`growpart` + `resize2fs`/`xfs_growfs`). Expose target size as a TUI/CLI field;
the 20 GiB build default stays.

Where to start: add a host-side resize step (`tart set --disk-size`), then
confirm the guest-side grow — check whether the bootc disk already auto-grows
root at boot; if not, add a growfs oneshot. See `tart.run/faq/#disk-resizing`.
