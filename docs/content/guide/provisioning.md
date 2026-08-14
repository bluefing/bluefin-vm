# First-boot provisioning

The published disk is generic, so the host stages your account details in the share before the VM starts, and the guest
creates your admin account from them on its first boot. With nothing staged the step is skipped and the baked `bluefin`
test login stays the way in.

Only public material crosses the share, and the guest deletes it once applied. No private key or other secret is written
there, which matters because the share is a host directory that outlives the VM.

## Timing

Provisioning runs once, on a VM's first boot, and an existing VM keeps the account it was given.
`bluefin-vm up --no-provision` stages nothing, leaving the baked test login as the way in.

## The login password

The password is set to the username. Nothing about it is secret; it is there because the greeter, the lock screen,
`sudo`, and polkit prompts all need a password to exist.

To set a password of your own, run `bluefin-vm-harden` in the VM. It is a convenience wrapper around `passwd`; GNOME's
Settings will do the same job.

## Posture

Two settings tune how the account is reached, both defaulting to the safer choice:

- **`sudo_password`** — `sudo` asks for the login password. The prompt is a guard against a pasted or mistyped root
    command, which is worth more than the keystrokes it costs. Turning it off writes a `NOPASSWD` rule.
- **`ssh_password_auth`** — sshd accepts passwords, which is reasonable while the VM sits behind the host's NAT. Turning
    it off makes the VM key-only, for a bridged or hardened one.

Your ssh key is installed either way. Both are profile keys, described in [VM profiles](../reference/configuration.md).

## Display scale

A scale is staged as a percentage rather than applied, because the scales the desktop accepts are per-mode values only
its session knows. The guest reads them at your first login and snaps your target to the nearest. The host stages it
only when the profile fixes the resolution — with refit on the guest follows the window, leaving nothing stable to scale
against.
