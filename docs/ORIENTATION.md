# Orientation

This document describes where things live and how the pieces fit together. Each
file documents itself in its own header, so for a file's scope, purpose, and
rationale, read the file.

## Build plumbing (`bin/`, run from repo root)

- `build-disk.sh` — container image → bootable disk (raw/qcow2/iso) via
  bootc-image-builder; the same entrypoint locally and in CI.
- `build-image.sh` — build `image/Containerfile` into the container store as a
  `localhost/` ref; runs before `build-disk.sh` for patched disks.
- `create-vm.sh` — import a built disk into a Tart VM (macOS).
- `package-cli.sh` — package the `bluefin-vm` tool into a release tarball for
  the Homebrew tap.

## Disk / image inputs

- `config.toml` — bootc-image-builder config: disk-build concerns only (20 GiB
  root, a dev/test login).
- `image/Containerfile` — derived image: the OS-side guest fixes (clipboard,
  sshd, share mount, `~/Shared`, first-boot provisioning) that `config.toml`
  can't express.
- `image/provision.sh`, `image/harden.sh` — guest scripts baked into that image:
  first-boot account creation from the host share, and the opt-in lock-down.

## Porcelain (`Justfile`, `.just/`)

- `Justfile` + `.just/{build,tart,cli}` — `just` recipes over the scripts. The
  rule: plumbing = scripts, porcelain = recipes.
- `.just/_config.just` — shared defaults, including `default_image` (the source
  image builds use unless overridden with `-i`); `_common.just` — shared helpers.

## The tool (`cli/`)

- `cli/` — the `bluefin-vm` Rust binary a user installs: download → extract →
  import → provision → run. `src/core/` is UI-agnostic; `src/main.rs` is the
  clap front-end. A future ratatui TUI drives the same core.

## Supporting

- `tests/*.bats` — offline suite (`just test`); `tests/smoke/guest-checks.sh` —
  in-VM acceptance check (`just tart smoke`).
- `.github/workflows/build-arm-image.yml` — CI disk build on an ARM64 runner;
  `release.yml` — on a `v*` tag, package + attach the tool tarball.
- The Homebrew formula lives in the tap repo (`bluefing/homebrew-tap`), not here.
- `README.md` (landing), `CLAUDE.md` (agent overlay), and `docs/` — `BUILDING`,
  `USAGE`, `PROVISIONING`, `ROADMAP`, `BACKLOG`.
