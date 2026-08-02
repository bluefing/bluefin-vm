# Backlog

Internal follow-up tracker. Not part of the rendered docs site. Each item has
enough context to pick up cold: what, why, where to start.

The legacy `docs/BACKLOG.md` and `docs/ROADMAP.md` still need folding in here —
tracked in `migration.md`. What's below is the TUI/CLI work captured during
design review.

## Command surface

The rename and the `config` namespace are designed in
`../design/command-surface.md`; this is the implementation slice.

### Rename `setup` → `tui`, add `bv config init|path|show`

Rename the `Setup` command to `Tui` (`main.rs`). Add a `Config` subcommand:
`init` scaffolds `~/.config/bluefin-vm/config.toml` with a default profile and
**never clobbers** an existing file (print its path and exit; `--force` to
overwrite); `path` prints the resolved path; `show` prints the current config.
Start in `main.rs` (the `Command` enum) and `core/config.rs` (a
save-if-absent + a default-profile constructor).

## TUI as a control surface

The TUI graduates from a one-shot profile editor into a front-end that also
drives the VM. The core ops are already UI-agnostic (`core/tart.rs`,
`core/provision.rs`), so the bulk of the work is running long operations inside
the ratatui event loop without freezing it (a worker thread + status channel),
not new core logic.

### Launch / create / up from within the TUI

Buttons to create or start a VM, and to run the `up-patched` / `up-provisioned`
flows, from inside the TUI. Where to start: the worker-thread + status-channel
plumbing above; the ops themselves already exist. This unlocks progress
reporting below.

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
