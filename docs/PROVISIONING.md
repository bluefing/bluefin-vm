# First-boot provisioning

This document describes how a downloaded, one-size-fits-all seed is customised
to become your VM on first boot. It covers what provisioning delivers, how the
mechanism works, limitations, and the reasoning behind the credential model.

## What it delivers

A published seed is byte-identical for everyone, so it can't carry your account
from build time. Provisioning closes that gap: the host writes your account
details into the share before boot, and a guest service applies them on first
boot.

The result, from `bluefin-vm up`, with no extra steps:

- an account that is *yours* (host `$USER` by default), in `wheel`
- your ssh *public* key installed, so `ssh you@vm` works
- autologin straight to the desktop — no greeter
- admin that works (passwordless `sudo`)

With no provision data present, the service does nothing and the baked
`bluefin` / `bluefin` test login remains the way in.

## Mechanism

Three moving parts: the host writes data to the shared directory before first
boot, the guest applies changes based on that data set and cleans up afterwards.

### Host

The host writes a hidden directory into the durable share, `~/bluefin-share/.bluefin-vm/`.

Relevant context is `core::provision`, driven by `up` or `bluefin-vm provision`:

| file | contents |
| --- | --- |
| `username` | the account name |
| `authorized_keys` | your ssh public key(s) |
| `autologin` | present = enable autologin |

**Only** *public* material crosses the share — never a password or private key.

### Guest

Guest (`bluefin-vm-provision.service` → `image/provision.sh`), on first
boot, when the host has left that directory in the share (detected by its
`username` file):

1. `useradd` the user in `wheel` (skipped if it already exists);
2. install `authorized_keys` → `~/.ssh` (700 dir, 600 file, owned by the user,
   SELinux-relabelled `ssh_home_t`[^selinux] — sshd ignores a mislabelled key);
3. add a `/etc/sudoers.d/bluefin-vm-<user>` drop-in[^sudo] granting that user
   `NOPASSWD`;
4. if autologin is requested, enable it (GDM `custom.conf`, edited with
   `configparser`[^gdm] so shipped settings survive) and disable the idle screen
   lock[^lock] (a password-less account can't clear it);
5. delete `…/.bluefin-vm/`[^hygiene] — nothing sensitive lingers in the share.

The unit is **gated** on that `username` file
(`ConditionPathExists=…/.bluefin-vm/username`) and **ordered**[^ordering]
(`After=` the share mount so the file is visible; `Before=` gdm and
user-sessions so the account and its autologin exist before login). sshd itself
is *not* provisioning — it is enabled in the base image.

## Credential model, and why

The one hard rule (BACKLOG BL-8): **public key only, no password through the
share.** A password would be a secret sitting in a host-visible, backed-up
folder; we won't put one there.

But a password-*less* account has three consequences, and the rest of the model
simply follows from them:

- **It can't authenticate at a greeter** — GDM checks a password — so the only
  way to *reach* the account's desktop is **autologin**. A greeter would be a
  dead end.
- **It can't `sudo`** — the prompt wants a password it doesn't have — and on
  Bluefin nearly everything (`rpm-ostree`, the dx toggle, `ujust`) needs
  privilege, so the account gets a **scoped passwordless-sudo** rule. (GUI
  polkit auth is a separate mechanism this doesn't cover — see Limits.)
- **It can't clear the lock screen** — same password — so an idle lock would
  trap the desktop until reboot; the autologin case therefore **disables the
  idle lock**.

So "no password" is not one choice among many — it *forces* autologin,
passwordless sudo, and a lock-free desktop. The only alternative (a password,
hence a greeter and normal sudo) means a secret through the share, which the
rule forbids. (A first-boot *setup* — the user creating their own account
interactively — is the other way to avoid a shared secret, but it's a greeter by
another name, ruled out by the decided no-greeter goal.)

This is the **disposable-dev-VM posture**: the VM is throwaway and holds no
durable secrets — those live host-side in the share, behind macOS's own auth.
Rooting a VM you can re-seed in minutes is low-consequence, so the convenience
is worth it — the same posture cloud images, WSL, and vagrant boxes take.

## Trade-offs and hardening

What you give up: passwordless root *inside* the VM, and a desktop that is
unlocked on boot and never idle-locks. Fine for a personal VM behind macOS; not
what you'd want on a shared or exposed host.

For the stricter posture, run **`bluefin-vm-harden`** — a helper baked into the
guest image (on `PATH`); run it in the VM, over ssh or a terminal, and it
self-elevates with sudo. It sets a password and removes the sudoers drop-in,
after which sudo, the greeter, and the lock screen all work normally.

To opt out up front, `bluefin-vm up` takes two flags (`up --help`):

- `--no-autologin` leaves the greeter — which a password-less account *can't*
  use, so it's for ssh-first use or once you've hardened.
- `--no-provision` skips provisioning and boots the stock baked `bluefin` login
  untouched.

## Limits / open questions

- **ssh-key auto-detect is narrow.** The host only auto-finds
  `~/.ssh/id_ed25519|id_ecdsa|id_rsa.pub`; non-standard names (e.g. FIDO
  `id_ed25519_sk_*.pub`) aren't found — pass `--ssh-key`. FIDO/`sk` keys also
  need the token present to authenticate. (BL-14)
- **sshd config is untouched** — provisioning only plants a key; it doesn't
  change `PasswordAuthentication` or anything else in the base image.
- **GUI polkit auth is unhandled.** The passwordless-sudo rule covers
  command-line `sudo`, not polkit (GUI privilege prompts) — a password-less user
  would still be asked for a password there. Untested; much Bluefin admin is CLI
  or Flatpak, so the gap is narrow, but closing it would need a polkit rule.
  (BL-15)
- **One account.** The model provisions a single primary user; multi-user or
  per-key policies aren't expressed.
- **Most reshape-able upstream:** the passwordless-sudo default and
  autologin-on default are the opinionated choices most likely to warrant
  discussion with the Bluefin team.

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
    file touches only the provisioned user and is trivially reversible
    (`bluefin-vm-harden` just removes it). The username is validated before the
    file is written, since a malformed `sudoers.d` entry can break `sudo`
    wholesale.

[^gdm]: Edited with Python `configparser` rather than overwritten: Fedora and
    Bluefin ship a `custom.conf` with a `[daemon]` section (and others); a blind
    overwrite would drop settings the base image relies on. `configparser` sets
    only `AutomaticLoginEnable` / `AutomaticLogin` and leaves the rest —
    verified in the container check, where the base image's `[security]` /
    `[debug]` sections survived.

[^lock]: A dconf *system* default — `/etc/dconf/db/local.d` sets
    `org.gnome.desktop.screensaver lock-enabled=false`, then `dconf update` —
    because a glib schema override would live in `/usr`, which is read-only at
    runtime on bootc, whereas `/etc` is writable. It disables only the lock; the
    screen may still blank on idle, it just won't lock. The dconf profile is
    extended to read the system db if it doesn't already.

[^hygiene]: The durable share is the user's *data* tier, so provisioning cleans
    up after itself. Nothing is lost by deleting — the host re-writes the
    details for each fresh seed — and it is public-key material anyway, so even
    mid-provision nothing secret sits in a host-visible, backed-up folder.

[^ordering]: `After=var-mnt-shared.mount` so the provision files are visible
    when the condition is evaluated and the script runs — before the mount, the
    gate would miss them. `Before=gdm.service` and `systemd-user-sessions.service`
    so the account (with its `authorized_keys` and autologin config) exists
    before first login or the greeter/autologin fires, rather than racing
    account creation against the login.
