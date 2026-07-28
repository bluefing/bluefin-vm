# Files

Per-file reference: scope, purpose, and behaviour/rationale for each
executable and config file — **not** its arguments (a script's `-h` is the
source of truth for those; don't restate them here). Docs and CI/test glue
are one-liners at the end.

## bin/build-disk.sh

The build entrypoint: turns a bootc container image into a bootable artifact
via bootc-image-builder. CI calls it too, so local and CI builds are
identical.

### Scope

The whole build step, every format (raw, qcow2, iso). Runtime-agnostic — it
produces the disk; Tart packaging happens later.

### Purpose

One command from source container to `./output/…`, hiding the
container-engine differences between a Mac (Docker/Colima) and a Linux CI
runner (Podman).

### Detail

- The two-step pull-then-build flow and per-OS engine selection are README
  "How the build works" — the script implements exactly that.
- `localhost/` images skip the pull step (store-only; nothing to pull from).
- Mounts `config.toml` at `/config.toml` when present. Assumes CWD = repo
  root.

## bin/build-image.sh

Builds the derived image (`image/Containerfile`) into the container store
that bootc-image-builder reads.

### Scope

The derived-image build step, both engines. Runs before `build-disk.sh` when
building patched disks, locally and in CI.

### Purpose

One engine-portable command from Containerfile to a `localhost/` ref in the
store.

### Detail

- Engine auto-select mirrors `build-disk.sh`: Podman on Linux (run via `sudo`,
  rootful storage), Docker otherwise. Assumes CWD = repo root.

## bin/create-vm.sh

Imports a built raw disk into a Tart VM.

### Scope

The runtime packaging step; macOS only. Needs a raw disk (converts a qcow2
if given one).

### Purpose

Create the Tart VM from the pipeline's disk.

### Detail

- The disk is required — no auto-detect (auto-detection once picked a stale
  disk and booted the wrong VM).
- Inputs are identified by **content**, not extension (qcow2 magic, ISO9660
  signature, GPT header), and validated *before* the destructive VM
  replacement — an ISO or junk file is rejected instead of becoming a broken
  VM.
- Creates an empty `tart create --linux` VM, swaps our raw in for its blank
  disk, and applies CPU / memory / display settings (tunable via env — see
  `-h`).
- qcow2→raw conversion uses the builder's bundled `qemu-img`, which writes a
  non-sparse raw over virtiofs — prefer building raw directly.

## config.toml

bootc-image-builder's config file — auto-read (at `/config.toml`, where
`bin/build-disk.sh` mounts it) to customise the disk it builds.

### Scope

Disk-build concerns only, every format, local and CI. Everything OS-side
lives in `image/Containerfile`.

### Purpose

Make the builder's bare default disk usable: big enough for Bluefin, with an
account to log into.

### Detail

- Enlarges the root filesystem to 20 GiB — without it the build fails on
  ostree `min-free-space-percent`.
- Adds a test-only `bluefin` / `bluefin` login (sudo) — without it a built
  disk has no account.
- The full rationale lives in the file's own comments — keep it there.

## image/Containerfile

Derived VM image: upstream Bluefin plus the guest configuration that needs
files inside the image.

### Scope

Optional layer, all formats. Sits between the upstream image and the build
step; `config.toml` still applies on top.

### Purpose

Own everything OS-side a VM guest needs: the clipboard-agent session wiring
(ordered, VM-conditional), sshd enabled, the host-share mount
(condition-gated so share-less boots stay clean), the `/etc/skel`
`~/Shared` symlink, and first-boot provisioning of the user's account (BL-8).

### Detail

- Built into the container store by `bin/build-image.sh` (`just build image`),
  then consumed as a `localhost/` ref.
- One-shot: `just tart up-patched` (always rebuilds; replaces VM state).
- First-boot provisioning (`provision.sh` run by `bluefin-vm-provision.service`):
  creates the account the host wrote into the share, then clears it.
  `bluefin-vm-harden` is the opt-in lock-down. Credential model lives in
  `provision.sh`.
- As upstream adopts the fixes (BL-1), this layer shrinks.

## Justfile + .just/

The porcelain: `just` recipes wrapping the `bin/` scripts, with defaults
centralised.

### Scope

The whole user/developer interface.

### Purpose

Friendly, consistent verbs over the plumbing. The rule: plumbing = scripts,
porcelain = recipes.

### Detail

- `Justfile` imports `_config`/`_common` and declares the `build` / `tart` /
  `cli` modules plus `test` / `lint` / `clean`.
- `up` is incremental: build only if the disk is absent, re-import only if
  the disk is newer than the VM's copy (which replaces VM state), then
  start. `up-patched` always rebuilds from the derived image.
- `up` starts the VM detached (terminal returns; output in
  `$TMPDIR/tart-<NAME>.log`; a death at startup fails the recipe loudly).
  `start` / `start-headless` stay attached.
- `.just/_config.just` holds shared defaults: `default_image`,
  `default_name`, `default_ref`, `default_share`, `patched_image`.
- Module recipes use `[no-cd]` so they run from the repo root, where the
  scripts expect to be.

## cli/

### Scope

The `bluefin-vm` tool — the Rust consumer binary a user installs (later via
brew), distinct from the image-build plumbing in `bin/`.

### Purpose

Turn a published seed into a running Bluefin VM: download → extract → import →
provision → run.

### Detail

- `src/core/` is UI-agnostic (nothing prints or draws): `download` (resumable,
  checksummed), `extract` (streams `image/disk.raw` out of the seed zip),
  `tart` (import + run — a port of `create-vm.sh`), `provision` (writes the
  first-boot account into the share). A future ratatui TUI drives the same core
  the CLI does.
- `src/main.rs` is the clap front-end: `up` runs the whole pipeline; `download`
  / `extract` / `import` / `provision` expose the individual steps for debugging.
- Driven by the `cli` module (`just cli run-release up`, `just cli check`); the
  pre-commit `rust` hook runs `just cli check`, so that recipe is the single
  source of truth for "is the crate clean".
- Shells out to `tart` at runtime, so the brew formula must depend on it
  (BACKLOG BL-7).

## Supporting files

- `tests/*.bats` — offline bats suite (no builds, no network): arg handling,
  dry-run output, recipe wiring. Run with `just test`.
- `tests/smoke/guest-checks.sh` — runtime acceptance test run *inside* a
  booted VM (not part of `just test`): verifies the baked patches (sshd,
  vdagent, share) and writes a timestamped result back through the share.
  Driven by `just tart smoke <name>`.
- `.github/workflows/build-arm-image.yml` — runs `bin/build-image.sh` +
  `bin/build-disk.sh` on an ARM64 runner (patched build by default) and uploads the
  disk as an artifact.
- `README.md` — user-facing source of truth. `CLAUDE.md` — agent overlay.
  `docs/ROADMAP.md` (decisions/questions), `docs/BACKLOG.md` (stories).
