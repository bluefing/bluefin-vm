# Quick start

One command downloads the published disk, imports it into Tart, stages your account, and boots the VM:

```bash
bluefin-vm up
```

The first run fetches a ~2.9 GiB archive and expands it into a 21 GiB disk under `~/.cache/bluefin-vm`, so it takes a
few minutes. Later runs boot the VM you already have, in about the time it takes to boot a desktop.

The VM runs detached (a window will open displaying the guest greeter) with its console log in
`$TMPDIR/tart-Bluefin.log`.

The following sections describe default provisioning. The account, share, and resources can be
[customised before the first boot](customisation.md), which is when they take effect.

## Logging in

First boot creates an account named after your macOS user, puts it in `wheel`, installs your ssh public key, and sets
the login password to the username. That password is a convention rather than a secret — it exists so the greeter, the
lock screen, `sudo`, and polkit prompts work. Set one of your own inside the VM:

```bash
bluefin-vm-harden
```

ssh works from first boot with the key that was installed:

```bash
ssh "$USER@$(tart ip Bluefin --resolver arp)"
```

The ARP resolver is worth the extra flag because tart's default DHCP-lease lookup returns nothing on recent macOS.

## The shared directory

`~/bluefin-share` on the host arrives in the guest at `/var/mnt/shared/bluefin-share`, with `~/Shared` as a symlink to
it. It lives on the host, so it is the place for anything that should outlive the VM.

## Stopping and starting

Shut down from the desktop, or:

```bash
tart stop Bluefin
```

Running `bluefin-vm up` again boots the same VM; it never destroys one. To start over from a fresh disk, losing
everything inside the guest, use `bluefin-vm up --replace` or [create a new profile](customisation.md).
