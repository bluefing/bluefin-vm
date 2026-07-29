# Building

Build the Bluefin VM image and disk yourself — which upstream image to use, how
the build runs locally and in CI, and how to test it. To run a built or
downloaded VM, see [USAGE.md](USAGE.md); to install the shipped tool, see the
[README](../README.md).

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

The mainline Bluefin (`:latest`, `:stable`, `:gts`) is **amd64-only**; ARM64
builds currently exist only in the `LTS` line. Always use an `*-arm64` (or
multi-arch `lts`) tag: an arm64 guest runs under the hypervisor with no CPU
emulation, so it's near-native — an amd64 image would force slow emulation.

| Tag | Arch |
| --- | --- |
| `bluefin:latest` / `stable` / `gts` | amd64 only |
| `bluefin:lts` | multi-arch (amd64 **+ arm64**) |
| `bluefin:lts-arm64` | arm64 (explicit) |

As of 2026-07 the stable `lts-arm64` ships a broken GNOME (gnome-shell 49.5
against mutter 49.4) which crash-loops to a black screen on aarch64. The
GNOME 50 testing image works and is this repo's default:

```
# gnome-shell 50.0 / mutter 50.0 — validated booting on Apple Silicon
ghcr.io/ublue-os/bluefin:lts-testing-50-arm64
```

Override per build with `-i`. Once GNOME 50 lands on the stable tag, swap
`default_image` in `.just/_config.just` back to `:lts-arm64`.

## How the build works

`bootc-image-builder` does **not** pull the source image itself — it reads it
from container storage. So every build is two steps: **pull** the image into
the store, then **build**. `bin/build-disk.sh` auto-selects the container engine:

- **Linux + Podman (CI):** uses the host's rootful container storage.
  Needs root for loop devices → run via `sudo ./bin/build-disk.sh`.
- **macOS + Docker/Colima:** there is no host container storage, so the
  script pulls into a named volume (`bootc-store`) using the builder image's
  bundled podman, then builds against it.
- **`localhost/` images** (e.g. the derived image below) exist only in the
  store — the pull step is skipped.

`config.toml` customises the built disk (root filesystem size, a test
login); its comments explain why each setting exists.

## The derived image

`image/Containerfile` layers the guest configuration that the stock image
lacks and disk-build customisation can't express (the why of each piece is in
the file's comments): the clipboard-agent session wiring, sshd enabled, the
host-share mount (condition-gated so share-less boots stay clean), and the
`~/Shared` symlink. `just build image` builds it into the container store as
a `localhost/` image — with the same engine auto-selection as the disk build,
so it works locally and in CI, where the patched build is the default
distribution artifact. `just tart up-patched` runs the whole chain —
container → disk → import → boot. A VM seeded this way needs **zero manual
guest setup**.

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
`ubuntu-24.04-arm` runner (Podman preinstalled) and uploads the image as an
artifact. Trigger from the Actions tab (`workflow_dispatch`).

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

A `pre-commit` hook gates every commit; set it up once with
`pre-commit install`.

Those are the offline dev-loop checks. There's also a **runtime** check that
runs *inside* a booted VM — `just tart smoke <name>` delivers
`tests/smoke/guest-checks.sh` through the share, runs it in the guest, and
reports on the baked patches (sshd, clipboard agent, share). Use it to
validate a seed actually works, not just that the plumbing is wired.

## What to test once booted

The workload is the test, not the conversion — check the Bluefin-specific
tooling works on ARM:

- `ujust` recipes (the `just`-based system management commands)
- the **dx / developer-mode** toggle
- anything the docs drive that might assume x86
