# Running & using a VM

This guide covers everything about a built or downloaded VM: starting and
stopping it, display density, the shared folder, first-boot provisioning, and
one-time guest setup for stock seeds. To build a VM, see [BUILDING.md](BUILDING.md).

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

## Sharper text

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

## Shared folder — where durable data lives

The recipes share `~/bluefin-share` into the VM over virtiofs automatically
(override with `TART_SHARE_DIR`; the default avoids `~/Documents`, which is
iCloud-synced on many Macs — evicted files would stall guest reads). In the
guest it lands at `/var/mnt/shared/bluefin-share`, with `~/Shared` as the
friendly symlink.

**The rule: the VM is disposable, the share is durable.** Anything in
`~/Shared` survives VM re-seed/reset/delete, is visible to macOS apps, and is
backed up with the Mac. But shares are slow for build/git workloads — keep
code in git on the VM's own disk; keep irreplaceable files in the share.

## First-boot provisioning

A downloaded seed is identical for everyone, so `up` personalises it before
first boot: it writes your account — username, ssh public key, autologin — into
the share, and a guest oneshot creates it on first boot, then clears the file.
Defaults come from the host (`$USER`, `~/.ssh/*.pub`); see `up --help` to
override, or to skip provisioning and keep the baked `bluefin`/`bluefin` login.
To lock the account down afterwards, run `bluefin-vm-harden` in the VM.

The credential model — why a password-less account gets autologin, passwordless
sudo, and a lock-free desktop — and the mechanism are in
[PROVISIONING.md](PROVISIONING.md).

## One-time guest setup

Patched seeds (`just tart up-patched`) work out of the box.

Stock seeds (`just tart up`) need some setup first — run these in the guest:

### SSH

```bash
# SSH first — the rest can then be pasted over ssh:
sudo systemctl enable --now sshd
```

### Shared folder

```bash
# The durable share:
echo 'com.apple.virtio-fs.automount /var/mnt/shared virtiofs defaults,nofail 0 0' \
  | sudo tee -a /etc/fstab
sudo mkdir -p /var/mnt/shared/bluefin-share && sudo mount -a
ln -s /var/mnt/shared/bluefin-share ~/Shared
```

### Clipboard

```bash
# Clipboard — the packaged spice-vdagent user unit is static (no [Install]),
# unordered, and GNOME 50 ignores the legacy autostart entry (upstream bug).
# Wire it in, ordered after the session — unordered it races
# the session environment and dies at login:
mkdir -p ~/.config/systemd/user/spice-vdagent.service.d
printf '[Unit]\nAfter=graphical-session.target\nPartOf=graphical-session.target\n' \
  > ~/.config/systemd/user/spice-vdagent.service.d/10-order.conf
systemctl --user add-wants graphical-session.target spice-vdagent.service
systemctl --user daemon-reload && systemctl --user start spice-vdagent
```
