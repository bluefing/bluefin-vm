# First-boot provisioning

This document describes how a downloaded, one-size-fits-all disk is customised
to become your VM on first boot. It covers what provisioning delivers, how the
mechanism works, limitations, and the reasoning behind the credential model.

## What it delivers

A published disk is byte-identical for everyone, so it can't carry your account
from build time. Provisioning closes that gap: the host writes your account
details into the share before boot, and a guest service applies them on first
boot.

The result, from `bluefin-vm up`, with no extra steps:

- an account that is *yours* (host `$USER` by default), in `wheel`
- your ssh *public* key installed, so `ssh you@vm` works
- a login password of `password == username`, so the greeter, the lock screen,
  `sudo`, and GUI polkit prompts all work
- `sudo` that prompts for that password (a guard against a mistyped or pasted
  root command); passwordless is an opt-in toggle
- ssh password login left on, as the base image ships it

With no provision data present, the service does nothing and the baked
`bluefin` / `bluefin` test login remains the way in.

## Mechanism

Two moving parts: the host writes data to the shared directory before first
boot, and the guest applies it — then cleans up — on first boot.

### Host

The host writes a hidden directory into the durable share, `~/bluefin-share/.bluefin-vm/`.

Relevant context is `core::provision`, driven by `up` or `bluefin-vm provision`.
Each flag file names a *non-default* choice, so a plain account writes neither:

| file | contents |
| --- | --- |
| `username` | the account name |
| `authorized_keys` | your ssh public key(s) |
| `passwordless-sudo` | present = grant passwordless `sudo` |
| `disable-ssh-password` | present = turn ssh password login off (pubkey-only) |
| `scale` | guest desktop scale percentage; absent = leave GNOME's default |

**Only** *public* material crosses the share — never a private key, and no
secret password: `password == username` is derived guest-side from the username
that's already there. The `scale` file is only written when the profile has
refit off (a fixed resolution to pin the scale to); see the display-density
notes in `docs/content/just/tart.md`.

### Guest

Guest (`bluefin-vm-provision.service` → `image/provision.sh`), on first
boot, when the host has left that directory in the share (detected by its
`username` file):

1. `useradd` the user in `wheel` (skipped if it already exists);
2. install `authorized_keys` → `~/.ssh` (700 dir, 600 file, owned by the user,
   SELinux-relabelled `ssh_home_t`[^selinux] — sshd ignores a mislabelled key);
3. set the login password to the username (`chpasswd`);
4. if `passwordless-sudo` is present, add a `/etc/sudoers.d/bluefin-vm-<user>`
   drop-in[^sudo] granting `NOPASSWD`;
5. if `disable-ssh-password` is present, write an `sshd_config.d` drop-in setting
   `PasswordAuthentication no` and reload sshd[^sshd];
6. if a `scale` is set, stash it in the account's home
   (`~/.config/bluefin-vm/scale-request`); the `bluefin-vm-apply-scale` user
   oneshot, gated on that file, applies it at first login by snapping to the
   nearest scale mutter reports for the live mode — the accepted values are
   per-mode and only the session-bus API knows them;
7. delete `…/.bluefin-vm/`[^hygiene] — nothing sensitive lingers in the share.

The unit is **gated** on that `username` file
(`ConditionPathExists=…/.bluefin-vm/username`) and **ordered**[^ordering]
(`After=` the share mount so the file is visible; `Before=` gdm and
user-sessions so the account exists before login). sshd itself is *not*
provisioning — it is enabled in the base image.

## Credential model, and why

This is a **daily-driver account you log into**, not a throwaway. The governing
principle is to follow a proven local-dev-VM posture (Mitchell Hashimoto's
`nixos-config`) unless there's a clear reason to diverge; the design rationale
lives in `docs/internal/design/access.md`.

The one hard rule: **no secret crosses the share** — a private key or a chosen
password would be sitting in a host-visible, backed-up folder. So the login
password is set to `password == username`: not a secret, just a public
convention (like the baked `bluefin`/`bluefin` login) derived from the username
already in the share. A real password *does* exist, which is what makes the
greeter, the lock screen, `sudo`, and GUI polkit prompts all work normally —
none of the password-less papercuts.

Two postures then sit on top, each a `bluefin-vm tui` toggle, each defaulting
to the safer/behind-the-host choice:

- **`sudo` prompts by default.** Not for security — the account is an admin
  either way — but as a speed-bump, so a fat-fingered or pasted command can't
  silently run as root. Toggle **passwordless** on to skip the prompt; that
  writes the `NOPASSWD` drop-in.
- **ssh password login stays on by default.** Tart's default NAT keeps the VM
  behind the host, so `user`/`user` over ssh is no more exposed than the console.
  Toggle it **off** (pubkey-only) for a VM you've bridged onto the LAN, or just
  to harden; that writes the sshd drop-in.

## Trade-offs and hardening

The default login password is the username — fine as a public convention behind
the host, but you may want a real one. Run **`bluefin-vm-harden`** in the VM (on
`PATH`, over ssh or a terminal); it self-elevates and sets a password you
choose. For a pubkey-only VM or passwordless `sudo`, set those in
`bluefin-vm tui` before `up` (or edit the flag files in the share and reboot).

`bluefin-vm up` still takes `--no-provision`, which skips provisioning entirely
and boots the stock baked `bluefin` login untouched.

## Limits / open questions

- **ssh-key auto-detect is narrow.** The host only auto-finds
  `~/.ssh/id_ed25519|id_ecdsa|id_rsa.pub`; non-standard names (e.g. FIDO
  `id_ed25519_sk_*.pub`) aren't found — pass `--ssh-key`. FIDO/`sk` keys also
  need the token present to authenticate, and Tart has no USB passthrough — see
  the secrets discussion in `docs/internal/design/access.md`.
- **The sudo and ssh-password postures are profile-only.** They're set through
  `bluefin-vm tui` (or the share flag files), not `up` flags yet.
- **One account.** The model provisions a single primary user; multi-user or
  per-key policies aren't expressed.

[^selinux]: SELinux (enforcing on Bluefin) gates file access by *type label*,
    separately from Unix permissions: sshd runs in the `sshd_t` domain, and
    policy only lets it read files typed `ssh_home_t`. A key installed from the
    virtiofs share into a freshly-created `~/.ssh` can land with the wrong type
    — right `600`/owner, wrong label — and sshd is then denied from reading it,
    silently rejecting the login (a `publickey` failure that looks like a key
    problem but is SELinux). `restorecon -R ~/.ssh` resets the label to the
    policy-mandated `ssh_home_t`. Guarded on `/sys/fs/selinux/enforce`, so it is
    a no-op where SELinux is off (e.g. the container check).

[^sudo]: A per-user drop-in, not a blanket `%wheel NOPASSWD`: the account is
    already in `wheel`, but a global rule would make *every* wheel user
    password-less — including the baked `bluefin` account. Scoping it to one
    file touches only the provisioned user and is trivially reversible (delete
    the file, or re-provision with the toggle off). The username is validated
    before the file is written, since a malformed `sudoers.d` entry can break
    `sudo` wholesale.

[^sshd]: A `/etc/ssh/sshd_config.d/00-bluefin-vm-nopassword.conf` drop-in. The
    `00-` prefix sorts first, and sshd takes the first value it sees for an
    option, so it wins over the base image's own drop-ins. Reloaded in-place so
    it applies on this first boot rather than only the next.

[^hygiene]: The durable share is the user's *data* tier, so provisioning cleans
    up after itself. Nothing is lost by deleting — the host re-writes the
    details for each fresh disk — and it is public-key material anyway, so even
    mid-provision nothing secret sits in a host-visible, backed-up folder.

[^ordering]: `After=var-mnt-shared.mount` so the provision files are visible
    when the condition is evaluated and the script runs — before the mount, the
    gate would miss them. `Before=gdm.service` and `systemd-user-sessions.service`
    so the account (with its `authorized_keys`) exists before first login or the
    greeter fires, rather than racing account creation against the login.
