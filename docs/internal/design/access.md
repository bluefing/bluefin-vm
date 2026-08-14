# Design — account, access, and customisation

What bluefin-vm sets up on a running VM: the account, how you get in, how you administer it, and how you make it yours.
Scope is the *provisioned* state — the account layer the host writes to the durable share and the guest applies at first
boot. The OS image and the disk build are out of scope here.

## Context

bluefin-vm is a daily-driver dev VM: a Bluefin bootc desktop under Tart (Apple Virtualization.framework) on an Apple
Silicon Mac. It is long-lived, not disposable — durable data lives on a host share and the disk is re-imageable, so you
rebuild the VM without losing state, the way you'd rebuild a workstation.

The audience is Bluefin users and developers fluent in cloud-native IaC: comfortable with declarative config, ssh
agents, and tailscale. `bluefin-vm tui` exposes every choice below but requires none — the defaults boot a working,
personal VM, and each default is overridable per VM.

Mitchell Hashimoto's `mitchellh/nixos-config` is a useful reference point: a widely-copied local dev VM with a
well-reasoned credential posture. The governing principle: **follow Hashi's model unless there's a clear reason to
diverge.** The Decision column notes each divergence and its reason.

## Access and credentials

| Element                | Hashi (`nixos-config`)                                                         | bluefin-vm                                                                                         | Decision                                                                                                                                                                                                                                 |
| ---------------------- | ------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| User account           | declarative user in `wheel`, `docker`, `lxd`; fish; `mutableUsers = false`     | host writes the chosen name to the share; guest oneshot creates it in `wheel` with `--create-home` | **decided** — one admin account; name from the TUI, default the host `$USER`                                                                                                                                                             |
| Autologin              | none — GDM greeter                                                             | none                                                                                               | **decided** — greeter login always; drop the autologin path entirely                                                                                                                                                                     |
| Login password         | a real low-value password; `hashedPassword` committed in the public repo       | `password == username`, derived guest-side at first boot                                           | **decided** — nothing but the username (already needed) crosses the share; a self-evident throwaway, so immune to the reused-password trap                                                                                               |
| sudo                   | never prompts — a `NOPASSWD` rule (`security.sudo.wheelNeedsPassword = false`) | a `bluefin-vm tui` toggle; **prompts by default**, opt into passwordless                           | **decided (c)** — its own toggle, independent of everything else. Default prompts (a speed-bump against a mistyped/pasted root command, not security); toggle on writes the `NOPASSWD` drop-in for Hashi-style passwordless.             |
| ssh auth               | pubkey **and** `PasswordAuthentication = true`; `PermitRootLogin = no`         | same by default; a `bluefin-vm tui` toggle disables password auth (pubkey-only)                    | **follow Hashi by default** — behind the host (see Reachability), so password auth stays on. A toggle turns it off (pubkey-only) for bridged or hardened setups. No root login either way.                                               |
| Secrets (keys, tokens) | rsync's real `~/.ssh` + `~/.gnupg` into the VM; never in the nix config        | copy in by default; a TUI setting picks which keys                                                 | **decided (a)** — copy by default, like Hashi. Hardware-backed keys (YubiKey) are the exception: they can't be copied usefully and Tart has no USB passthrough, so they need agent forwarding — an optional toggle (BL-13). See Secrets. |
| Dotfiles / environment | home-manager, declarative in-repo                                              | chezmoi bootstrap hook                                                                             | **open** — mechanism undecided (below)                                                                                                                                                                                                   |

## Reachability

Both VMs sit behind the host. Tart's default networking is NAT/shared: the guest gets a host-private address, reachable
from the Mac but not from the physical LAN (we pass no `--net-*` flag). That is effectively Hashi's posture, so a
low-value `password == username` over ssh is no weaker here than his — which is why ssh follows him rather than
diverging. Exposure only widens if a user opts into `--net-bridged` (VM on the real LAN) or runs sibling VMs that share
the vmnet subnet; for those, a `bluefin-vm tui` toggle disables ssh password auth (pubkey-only). By default it stays on,
like Hashi.

Why Hashi commits a real password hash at all: it is a reproducibility trade. An inline `hashedPassword` rebuilds the VM
with zero secret-management plumbing, and the cracked value grants nothing beyond a sandbox behind his Mac. (It is a
real, distinct password, not his username hashed — verified.) We reach the same end more cleanly: the guest derives
`password == username` at boot, so there is no hash to store, back up, or leak.

## Secrets

Default follows Hashi: copy the host's `~/.ssh` (and `~/.gnupg`) into the guest, with a TUI setting to narrow which
keys. That works for software keys.

Hardware-backed keys are the exception, and they matter here: a YubiKey user's `~/.ssh` files are only *references* to
keys on the device, so copying them yields nothing usable. Tart exposes no USB passthrough (verified: no such option in
`tart run`, and Apple's Virtualization.framework has no general USB device passthrough), so the YubiKey can't be handed
to the guest at all. The only route is **agent forwarding** — the Mac's ssh-agent, holding the device, answers
challenges from inside the VM.

Its effort is uneven, which is why it's an optional toggle (BL-13), not the default:

- **Terminal / `just tart ssh`:** `ssh -A` forwards the agent for free — YubiKey-backed ssh-out just works.
- **GUI desktop session:** hard — guest apps read `$SSH_AUTH_SOCK`, but Tart shares directories over virtiofs, across
    which Unix sockets don't work, so it needs a vsock relay or similar. That is the real work behind the toggle.

## User customisation — a hook (mechanism open)

Goal: once the account exists, let the user layer their own environment without baking anything into the image, matching
how a cloud-native developer already provisions a container or a fresh box. The tool is the user's — chezmoi,
`ansible-pull`, home-manager, or a plain script — so the hook stays tool-agnostic; it just runs what the user brings.

The reference model is a devcontainer dotfiles feature: at post-create it runs a bootstrap command (in the maintainer's
case `chezmoi init --apply <repo>`) with the host ssh-agent forwarded, so even a private dotfiles repo bootstraps with
no secrets written to disk. The VM equivalent is a first-boot hook that runs once, as the provisioned user: a command
the user configures (a TUI field), e.g. `chezmoi init --apply <repo>`.

Open questions, no code yet:

- The exact shape: a single "run this once, as the user" command is the general case; a dotfiles-repo field is just the
    common convenience over it.
- Auth is only needed for the exception. A public repo (the common case — the maintainer's Codeberg dotfiles among them)
    clones with no credentials. Only a *private* repo needs them, and only then is the question live: agent forwarding,
    a scoped token, or deferring that bootstrap to the first interactive login.

## Open decisions

- **Customisation hook** — mechanism per the section above.

(sudo posture is decided: a setup toggle, prompts by default — see the table.)
