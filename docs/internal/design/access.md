# Design — account, access, and customisation

What bluefin-vm sets up on a running VM: the account, how you get in, how you
administer it, and how you make it yours. Scope is the *provisioned* state — the
account layer the host writes to the durable share and the guest applies at
first boot. The OS image and the disk build are out of scope here.

## Context

bluefin-vm is a daily-driver dev VM: a Bluefin bootc desktop under Tart (Apple
Virtualization.framework) on an Apple Silicon Mac. It is long-lived, not
disposable — durable data lives on a host share and the disk is re-imageable, so
you rebuild the VM without losing state, the way you'd rebuild a workstation.

The audience is Bluefin users and developers fluent in cloud-native IaC:
comfortable with declarative config, ssh agents, and tailscale. `bluefin-vm
setup` (the TUI) exposes every choice below but requires none — the defaults
boot a working, personal VM, and each default is overridable per VM.

Mitchell Hashimoto's `mitchellh/nixos-config` is a useful reference point: a
widely-copied local dev VM with a well-reasoned credential posture. We follow it
selectively — the landscape differs enough (below) that a few choices diverge.

## Access and credentials

| Element | Hashi (`nixos-config`) | bluefin-vm | Decision |
| --- | --- | --- | --- |
| User account | declarative user in `wheel`, `docker`, `lxd`; fish; `mutableUsers = false` | host writes the chosen name to the share; guest oneshot creates it in `wheel` with `--create-home` | **decided** — one admin account; name from the TUI, default the host `$USER` |
| Autologin | none — GDM greeter | none | **decided** — greeter login always; drop the autologin path entirely |
| Login password | a real low-value password; `hashedPassword` committed in the public repo | `password == username`, derived guest-side at first boot | **decided** — nothing but the username (already needed) crosses the share; a self-evident throwaway, so immune to the reused-password trap |
| sudo | passwordless (`security.sudo.wheelNeedsPassword = false`) | prompts, using the login password | **proposed (a)** — a deliberate prompt guards a fat-fingered or pasted root command, and it deletes the passwordless-sudo drop-in. (b) = follow Hashi (passwordless) |
| ssh auth | pubkey **and** `PasswordAuthentication = true`; `PermitRootLogin = no` | pubkey-only; password auth off; no root login | **decided** — the VM is LAN-reachable, so `user`/`user` over ssh is unacceptable (see Threat model) |
| Secrets (keys, tokens) | rsync'd in separately; never in the nix config | none copied in; host ssh-agent forwarding | **proposed** — agent forwarding (BL-13) is the model, not copied keys |
| Dotfiles / environment | home-manager, declarative in-repo | chezmoi bootstrap hook | **open** — mechanism undecided (below) |

## Threat model — why ssh diverges from Hashi

Hashi's "a password hash in the open is fine" rests on the VM being unreachable:
VMware runs it on a hypervisor loopback with no routable address. Ours isn't —
a Tart VM gets a real address on the Mac's local network (what `tart ip`
resolves), so a `user`/`user` login reachable over ssh is genuinely weak on a
shared LAN.

So the low-value login password stays for the *local* path — console, greeter,
keyring unlock, sudo — where exposure is bounded by access to the Mac, but ssh is
**pubkey-only**. That is the one deliberate divergence from Hashi.

Why Hashi commits a real password hash at all: it is a reproducibility trade. An
inline `hashedPassword` rebuilds the VM with zero secret-management plumbing, and
the cracked value grants nothing beyond a sandbox behind his Mac. (It is a real,
distinct password, not his username hashed — verified.) We reach the same end
more cleanly: the guest derives `password == username` at boot, so there is no
hash to store, back up, or leak.

## User customisation — chezmoi hook (mechanism open)

Goal: once the account exists, let the user layer their own environment without
baking anything into the image, matching how a cloud-native developer already
provisions a container or a fresh box.

The reference model is a devcontainer dotfiles feature: at post-create it runs
`chezmoi init --apply <repo>` with the host ssh-agent forwarded, so even a
private dotfiles repo bootstraps with no secrets written to disk. The VM
equivalent is a first-boot hook that runs once, as the provisioned user: given an
optional dotfiles repo (a TUI field), `chezmoi init --apply`.

Open questions, no code yet:

- How the guest reaches a *private* repo at first boot — agent forwarding, a
  scoped token, or deferring the bootstrap to the first interactive login.
- Whether the hook is chezmoi-specific or a generic "run this once, as the user"
  escape hatch that chezmoi is merely the common case of.

## Open decisions

- **sudo posture** — proposed (a), password sudo. Picking (a) removes the
  passwordless-sudo drop-in; (b) keeps it (ungated from autologin).
- **Customisation hook** — mechanism per the section above.
