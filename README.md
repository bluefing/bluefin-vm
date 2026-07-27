# bluefin-vm

Build the **ARM64 [Bluefin](https://projectbluefin.io) bootc containers** into
a bootable [Tart](https://tart.run) VM, so Bluefin runs as a fast,
near-native Linux dev environment beside macOS on Apple Silicon.

## Why

You can run Bluefin in a Mac VM the manual way — grab an ISO, create a VM,
click through the installer. It works, but it's hand-work you repeat on every
machine and redo each time the image refreshes, and what you get is a one-off
to reconfigure, not something reproducible.

This repo is a pipeline instead: **macOS keeps the hardware, and Bluefin runs
in a VM built from the upstream container** — booting straight to a desktop,
no installer, no greeter. A native `aarch64` guest under Apple's Virtualisation
framework runs at near-native speed; the goal is a VM so good that,
full-screened, you can't tell it isn't bare metal.

> **Status: spike / build / evaluate.** The pipeline works end-to-end and the
> runtime is Tart; packaging and delivery are still open.
> [docs/ROADMAP.md](docs/ROADMAP.md) tracks what is decided.

## Quick start

```bash
# Run Bluefin in a VM — builds what's missing, then launches detached:
just tart up-patched    # recommended: derived image, zero guest setup
just tart up            # stock upstream image (needs one-time guest setup)

# Just build a disk, don't launch:
just build raw          # or: iso / qcow2
just build raw -i ghcr.io/<org>/<image>:<tag>   # override the source image
```

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

## Running the VM

`just tart up` is incremental — it:

- builds the raw disk if absent
- re-imports only when the disk is newer than the VM's copy (that step
  replaces VM state)
- boots

A repeat `up` goes straight to boot and keeps your VM.

`just tart up-patched` is the opposite contract — it:

- always rebuilds from the derived image
- replaces the VM

Both start the VM detached and return your terminal (output lands in
`$TMPDIR/tart-<name>.log`; startup failures still fail the recipe).

`start` / `start-headless` are the attached variants, `just tart ssh` gets you
in, `just tart stop` shuts down. Defaults: 1920×1200 display with window refit
(override `TART_DISPLAY`, `TART_CPU`, `TART_MEM`).

### Sharper text

The Tart view maps one guest pixel to one host **point**, so the VM's pixel
density comes from the *host display mode*, not the guest resolution:

- **Default:** display-refit follows the window. On a default host mode this
  renders at 1× — right-sized, slightly soft next to native macOS text.
- **Crisp:** switch the host to a denser mode (e.g. 2880×1800 on a 15" panel)
  and fullscreen — refit matches the guest at near-native panel density. Pick
  UI size inside GNOME: Scale 100% (maximum space, small text) or 200%
  (looks-like half, larger UI, same crispness). Cost: the host mode is
  global, so the macOS UI shrinks too.

Don't pin `--display` above the window size — anything larger than the
window is cropped, not shrunk to fit. (Tart's HiDPI display units apply to
macOS guests only; the Linux scanout is raw pixels — a
Virtualisation.framework limitation.)

### Shared folder — where durable data lives

The recipes share `~/bluefin-share` into the VM over virtiofs automatically
(override with `TART_SHARE_DIR`; the default avoids `~/Documents`, which is
iCloud-synced on many Macs — evicted files would stall guest reads). In the
guest it lands at `/var/mnt/shared/bluefin-share`, with `~/Shared` as the
friendly symlink.

**The rule: the VM is disposable, the share is durable.** Anything in
`~/Shared` survives VM re-seed/reset/delete, is visible to macOS apps, and is
backed up with the Mac. But shares are slow for build/git workloads — keep
code in git on the VM's own disk; keep irreplaceable files in the share.

### One-time guest setup

Patched seeds (`just tart up-patched`) work out of the box.

Stock seeds (`just tart up`) need some setup first — run these in the guest:

#### SSH

```bash
# SSH first — the rest can then be pasted over ssh:
sudo systemctl enable --now sshd
```

#### Shared folder

```bash
# The durable share:
echo 'com.apple.virtio-fs.automount /var/mnt/shared virtiofs defaults,nofail 0 0' \
  | sudo tee -a /etc/fstab
sudo mkdir -p /var/mnt/shared/bluefin-share && sudo mount -a
ln -s /var/mnt/shared/bluefin-share ~/Shared
```

#### Clipboard

```bash
# Clipboard — the packaged spice-vdagent user unit is static (no [Install]),
# unordered, and GNOME 50 ignores the legacy autostart entry (upstream bug,
# BACKLOG BL-1). Wire it in, ordered after the session — unordered it races
# the session environment and dies at login:
mkdir -p ~/.config/systemd/user/spice-vdagent.service.d
printf '[Unit]\nAfter=graphical-session.target\nPartOf=graphical-session.target\n' \
  > ~/.config/systemd/user/spice-vdagent.service.d/10-order.conf
systemctl --user add-wants graphical-session.target spice-vdagent.service
systemctl --user daemon-reload && systemctl --user start spice-vdagent
```

## Tests

Fast, offline [bats](https://github.com/bats-core/bats-core) checks in
`tests/` — arg handling, `build-disk.sh -n` dry-run output, and `just` recipe
wiring. No container builds, no network. The lint gate needs the system
tools (`brew install bats-core shellcheck shfmt hadolint pre-commit`):

```bash
just test        # bats — fast inner loop
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

## License

[Apache-2.0](LICENSE) — matching upstream Bluefin.
