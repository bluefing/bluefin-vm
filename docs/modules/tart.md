# tart — run and manage the VM

The `tart` recipes run a built or downloaded Bluefin disk as a VM under Apple's
Virtualisation framework (through [Tart](https://tart.run)). `just tart` lists
the verbs; this page covers which "up" to reach for, how you get in, and how
the display and shared folder behave.

## Nomenclature

- **stock**: built straight from the upstream Bluefin image.
- **patched**: built from `image/Containerfile`, which bakes in the guest glue the
  stock image lacks (clipboard, sshd, the host-share mount, `~/Shared`), including
  a first-boot provisioner that stays dormant — so it boots to the baked `bluefin`
  login.
- **provisioned**: a patched disk booted with your account staged in the share, so
  first-boot provisioning personalises it (account, ssh key, autologin). Same disk
  as patched, plus the staged data.

## The three ways up

The lifecycle verbs differ in which of those they build and whether they keep your VM.

- **`up`** is incremental. It builds a disk from the upstream image only if one
  is missing and reuses it otherwise, re-imports only when the disk is newer
  than the VM's copy, then boots — so a repeat `up` goes straight to boot and
  keeps the VM you have. The everyday verb once a VM exists.
- **`up-patched`** builds the patched image and a fresh disk from it, then
  **replaces** the VM. It's how you get a ready-to-use disk that skips the
  manual patching below; reach for it after changing `image/Containerfile`.
- **`up-provisioned`** is `up-patched` with your account staged first: it writes
  the provisioning data through the real `bluefin-vm provision` writer, so the
  fresh disk boots *through* first-boot provisioning into your own account
  rather than the baked test login. It's the loop for changing `image/provision.sh`.

`up-patched` and `up-provisioned` both discard VM state; `up` preserves it.
Force a fresh disk under `up` with `just build raw` — `up` then picks it up.

All three start the VM **detached**: the terminal comes back, output goes to
`$TMPDIR/tart-<name>.log`, and a startup failure still fails the recipe.
`start` and `start-headless` are the attached variants (a window, or none) for
when you want to watch the boot.

## Stock disk — manual patching

A stock (unpatched) disk requires some manual configuration:

```bash
# SSH — enable it first, then the rest can be pasted over ssh:
sudo systemctl enable --now sshd

# Shared folder:
echo 'com.apple.virtio-fs.automount /var/mnt/shared virtiofs defaults,nofail 0 0' \
  | sudo tee -a /etc/fstab
sudo mkdir -p /var/mnt/shared/bluefin-share && sudo mount -a
ln -s /var/mnt/shared/bluefin-share ~/Shared

# Clipboard — the packaged spice-vdagent user unit is static and unordered, and
# GNOME 50 ignores the legacy autostart entry, so wire it in ordered after the
# session (unordered it races the session environment and dies at login):
mkdir -p ~/.config/systemd/user/spice-vdagent.service.d
printf '[Unit]\nAfter=graphical-session.target\nPartOf=graphical-session.target\n' \
  > ~/.config/systemd/user/spice-vdagent.service.d/10-order.conf
systemctl --user add-wants graphical-session.target spice-vdagent.service
systemctl --user daemon-reload && systemctl --user start spice-vdagent
```

## Getting in

`ssh` defaults to your **host account** — the one provisioning creates — so a
provisioned VM needs no flags. Pass `--user bluefin` for the baked test login,
which every disk has whether or not provisioning ran. `ip` prints the guest
address; ssh and smoke resolve through it. Both use the ARP resolver, because
the default DHCP-lease lookup returns nothing on macOS 26.

The provisioned account is password-less by design; for the credential model and
the `bluefin-vm-harden` lock-down, see `docs/PROVISIONING.md`.

## Display density

Tart maps one guest pixel to one host **point**, so the VM's sharpness comes
from the *host display mode*, not the guest resolution:

- **Default** — display-refit follows the window. On a default host mode this
  renders at 1×: right-sized, slightly soft next to native macOS text.
- **Crisp** — switch the host to a denser mode (e.g. 2880×1800 on a 15" panel)
  and fullscreen; refit then matches the guest at near-native density. Choose UI
  size inside GNOME (Scale 100% for space, 200% for larger UI at the same
  crispness). The cost is that the host mode is global, so macOS shrinks too.

Don't pin `--display` above the window size — anything larger is cropped, not
scaled to fit. (Tart's HiDPI units apply to macOS guests only; the Linux
scanout is raw pixels, a Virtualisation.framework limitation.) Resources and the
default mode come from `TART_DISPLAY` (1920×1200), `TART_CPU` (4), and
`TART_MEM` (4096 MiB).

## The shared folder

The recipes share `~/bluefin-share` into the VM over virtiofs automatically
(`TART_SHARE_DIR` overrides it; the default avoids `~/Documents`, which is
iCloud-synced on many Macs — evicted files would stall guest reads). In the
guest it lands at `/var/mnt/shared/bluefin-share`, with `~/Shared` as the
friendly symlink.

## Checking a patched VM works

`smoke` validates a *booted* VM rather than the plumbing: it delivers
`tests/smoke/guest-checks.sh` through the share, runs it in the guest, and
asserts the result log came back — which doubles as proof the share round-trips.
It logs in as the baked `bluefin` account by default (the one a fresh disk
always has); pass `--user` for a provisioned VM. The recipe's exit code is the
guest checks' own.

Beyond that, the workload is the real test — confirm the Bluefin tooling works
on ARM: the `ujust` recipes, the dx / developer-mode toggle, and anything the
Bluefin docs drive that might assume x86.
