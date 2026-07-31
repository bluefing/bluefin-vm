# build — images and disks

The `build` recipes turn the upstream Bluefin bootc image into the derived image
and the bootable disks that Tart runs. `just build` lists the verbs; this page
covers what each produces, which upstream image to start from, and how a build
runs on a Mac.

## What the verbs produce

- **`raw`** → `output/image/disk.raw` — the format Tart imports.
- **`qcow2`** → `output/qcow2/disk.qcow2` — thin-provisioned; generic.
- **`iso`** → `output/bootiso/` — an installer image.
- **`image`** → the derived image built from `image/Containerfile` into the
  container store as a `localhost/` ref, ready for a patched disk build.

Every disk verb takes `-i`/`--image` to override the source. The default source
is `default_image` in `.just/_config.just`; `build image` passes it as the base
so the patched layer builds on the configured image rather than the
Containerfile's own fallback.

## Which upstream image

Live images are published under **`ghcr.io/projectbluefin/`** — `bluefin` and
`bluefin-lts`. The old `ghcr.io/ublue-os/` images are stale; don't use them.

Always use an `*-arm64` (or multi-arch `lts`) tag — Tart runs natively on Apple
Silicon.

| Tag | Arch |
| --- | --- |
| `bluefin:latest` / `stable` / `gts` | amd64 only (currently) |
| `bluefin:lts` | multi-arch (amd64 **+ arm64**) |
| `bluefin:lts-arm64` | arm64 (explicit) |

The stable `lts-arm64` has shipped a mismatched gnome-shell/mutter that
crash-loops to a black screen on aarch64, while the `lts-testing` arm64 tags ship
a matched GNOME 50 that boots — so this repo defaults to a GNOME 50 `lts-testing`
arm64 tag. Confirm the current state before trusting a tag, and override per
build with `-i`.

## How a build runs

`bootc-image-builder` reads the image from container storage, so every build is
two steps: **pull** the image into the store, then **build**. `bin/build-disk.sh`
auto-selects the engine:

- **Linux + Podman (CI)** uses the host's rootful storage — needs root for loop
  devices, so run it under `sudo`.
- **macOS + Docker/Colima** has no host container storage, so the script pulls
  into a named volume (`bootc-store`) with the builder image's bundled podman,
  then builds against it.
- **`localhost/` images** (the derived image) exist only in the store, so the
  pull step is skipped.

`config.toml` (auto-read at `/config.toml`) owns disk-build concerns only: a
20 GiB root, which gives ostree room to write the deployment — without it the
build fails on `min-free-space-percent` — and the baked `bluefin` test login.
Everything OS-side lives in `image/Containerfile` instead, where it can be
conditional and travels with bootc updates: the clipboard-agent wiring, sshd,
the host-share mount, `~/Shared`, and first-boot provisioning.

## Building on the Mac

Colima must be running first, with headroom: `colima start --cpu 4 --memory 8`,
and budget ~20 GB of free disk per raw build (a raw disk is a full-size file, so
20 GiB there is 20 GiB used; qcow2 is thin, so nearly free).

## CI build

`.github/workflows/build-arm-image.yml` runs the same `build-disk.sh` on a
`ubuntu-24.04-arm` runner (Podman pre-installed) and uploads the image as an
artefact (multi-GB, kept 7 days) — trigger it from the Actions tab.
