# Building

This guide covers building the Bluefin VM image and disk yourself: which
upstream image to use, how the build runs locally and in CI, and how to test it.
To run a built or downloaded VM, see [USAGE.md](USAGE.md); to install the shipped
tool, see the [README](../README.md).

From a checkout you can also run the published seed without building at all:
`just cli run-release up` downloads the seed, imports it, and boots — the same
pipeline as the installed `bluefin-vm up`.

## Build a disk

```bash
just build raw          # build a bootable disk (or: iso / qcow2)
just build image        # build the derived image into the container store
just build raw -i ghcr.io/<org>/<image>:<tag>   # override the source image
```

Or `just tart up-patched` to build the derived image, disk, import, and boot in
one go (see [USAGE.md](USAGE.md)).

## Which image

Live images publish under **`ghcr.io/projectbluefin/`** — `bluefin` and
`bluefin-lts`, browsable at
[projectbluefin/bluefin](https://github.com/projectbluefin/bluefin/pkgs/container/bluefin)
and [bluefin-lts](https://github.com/projectbluefin/bluefin/pkgs/container/bluefin-lts).
The old `ghcr.io/ublue-os/` images are **stale — don't use them.**

The mainline Bluefin (`:latest`, `:stable`, `:gts`) is **amd64-only**; ARM64
builds currently exist only in the `LTS` line. Always use an `*-arm64` (or
multi-arch `lts`) tag: an arm64 guest runs under the hypervisor with no CPU
emulation, so it's near-native — an amd64 image would force slow emulation.

| Tag | Arch |
| --- | --- |
| `bluefin:latest` / `stable` / `gts` | amd64 only |
| `bluefin:lts` | multi-arch (amd64 **+ arm64**) |
| `bluefin:lts-arm64` | arm64 (explicit) |

The stable `lts-arm64` ships a mismatched GNOME — gnome-shell 49.5 against
mutter 49.4 — which crash-loops to a black screen on aarch64. As of 2026-07
`ghcr.io/projectbluefin/bluefin:lts-arm64` still carries that pairing, so it is
unusable; the `lts-testing` arm64 tags ship a matched gnome-shell/mutter 50 and
boot. This repo therefore defaults to a GNOME 50 `lts-testing` arm64 tag.
Override per build with `-i`.

## How the build works

`bootc-image-builder` reads the image from container storage. So every build is
two steps: **pull** the image into the store, then **build**.
`bin/build-disk.sh` auto-selects the container engine:

- **Linux + Podman (CI):** uses the host's rootful container storage.
  Needs root for loop devices → run via `sudo ./bin/build-disk.sh`.
- **macOS + Docker/Colima:** there is no host container storage, so the
  script pulls into a named volume (`bootc-store`) using the builder image's
  bundled podman, then builds against it.
- **`localhost/` images** (e.g. the derived image below) exist only in the
  store — the pull step is skipped.

`config.toml` customises the built disk (root filesystem size, a test
login).

## The derived image

`image/Containerfile` layers the guest configuration that the stock image
lacks and the disk-build customisation can't express, such as:

- the clipboard-agent session wiring,
- sshd enabled,
- the host-share mount and the `~/Shared` symlink.

`just build image` builds it into the container store as a `localhost/` image —
with the same engine auto-selection as the disk build, so it works locally and
in CI, where the patched build is the default distribution artefact.

`just tart up-patched` runs the whole chain — container → disk → import → boot.
A VM seeded this way needs zero manual guest setup.

## Building locally on the Mac (Docker/Colima)

Colima must be running first: `colima start --cpu 4 --memory 8` — give it
that headroom or large builds can OOM, and budget ~20 GB of free disk per
raw build. Builds land in:

| format | output |
| --- | --- |
| `raw` | `./output/image/disk.raw` |
| `qcow2` | `./output/qcow2/disk.qcow2` |
| `iso` | `./output/bootiso/` |

## CI build (native ARM64 runner)

`.github/workflows/build-arm-image.yml` runs the same `build-disk.sh` on a
`ubuntu-24.04-arm` runner (Podman pre-installed) and uploads the image as an
artefact. Trigger from the GitHub Actions tab (`workflow_dispatch`).

> Artifacts are multi-GB; retention is 7 days. A real release belongs in
> object storage or a GitHub Release.

## Tests

Fast, offline [bats](https://github.com/bats-core/bats-core) checks in
`tests/` — arg handling, `build-disk.sh -n` dry-run output, and `just` recipe
wiring. No container builds, no network. `just lint` needs `bats` and
`pre-commit` on the system (`brew install bats-core pre-commit`);
shellcheck/shfmt/hadolint come from pre-commit's pinned environments, not the
system:

```bash
just test        # bats + the cli's Rust unit tests — fast inner loop
just lint        # pre-commit run --all-files: shellcheck, shfmt, hadolint, tests, ...
```

A `pre-commit` hook gates every commit; set it up once with `just setup`.

Those are the offline dev-loop checks. There's also a **runtime** check that
runs *inside* a booted VM — `just tart smoke <name>` on the host delivers
`tests/smoke/guest-checks.sh` through the share, runs it in the guest, and
reports on the baked patches (sshd, clipboard agent, share). Use it to
validate a seed actually works, not just that the plumbing is wired.

## What to test once booted

The workload is the test, not the conversion — check the Bluefin-specific
tooling works on ARM:

- `ujust` recipes (the `just`-based system management commands)
- the **dx / developer-mode** toggle
- anything the docs drive that might assume x86
