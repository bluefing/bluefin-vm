# bluefin-vm

Turn the upstream **[Bluefin](https://projectbluefin.io)** bootc container into a
running Linux VM on Apple Silicon with one command — a provisioned desktop, no
ISO, no installer, no greeter.

An `aarch64` guest under Apple's Virtualisation framework (via
[Tart](https://tart.run)) delivers near-native speed. What this project adds is
the convenience: a reproducible build from the upstream bootc image, first-boot
provisioning, and the guest tweaks (clipboard, shared folder, ssh) that make the
VM a fast, low-friction dev environment — a rock-solid, immutable Linux desktop
on rock-solid Apple hardware.

## Install with Homebrew

`bluefin-vm` installs from a Homebrew tap and shells out to `tart` (the VM
runtime), which lives in OpenAI's own third-party tap. First-time setup requires
trusting **two** taps — tart's, then this one:

```bash
# 1. tart's tap (provides tart + its softnet helper) — tap and trust it once:
brew tap openai/tools
brew trust openai/tools

# 2. the tool (the fully-qualified name self-trusts this one formula):
brew install bluefing/tap/bluefin-vm
bluefin-vm up
```

`bluefin-vm up` downloads the published seed, imports it into Tart, provisions
your account, and boots the VM. The formula installs only the tool (~1.6 MB);
the multi-GB seed is downloaded at runtime.

Brewfile installs and the trust details are in the
[tap's README](https://github.com/bluefing/homebrew-tap).

## Status

Experimental — a working proof-of-concept, not a stable release.

- Pipeline works end-to-end.
- Runtime is Tart.
- Tool ships via a personal Homebrew tap.

## Planned

Not built yet — where this is headed:

- **Interactive setup (TUI)** — a [ratatui](https://ratatui.rs) front-end over
  the same core, to customise the VM (account, ssh key, autologin, CPU/memory,
  image flavour) interactively instead of via flags.
- **Flavours** — pick a Bluefin variant (e.g. `bluefin-dx`) as the seed; each is
  just a different upstream image.
- **Automated seed delivery** — CI builds and publishes seeds so downloads stay
  fresh without manual uploads.
- **Host key integration** — use your host ssh keys / YubiKey from inside the
  guest via agent forwarding, no secrets copied in.
- **Suspend / resume** — pause keeping the in-RAM session (waiting on Tart's
  Linux-guest support).

## Docs

- **[Building](docs/BUILDING.md)** — build the image and disk yourself: which
  upstream image to use, local and CI builds, and tests.
- **[Running & using a VM](docs/USAGE.md)** — start/stop/ssh, display density,
  the shared folder, and one-time guest setup for stock seeds.
- **[First-boot provisioning](docs/PROVISIONING.md)** — the account and
  credential model (autologin, passwordless sudo, hardening).
- **[Roadmap](docs/ROADMAP.md)** and **[Backlog](docs/BACKLOG.md)** — decisions,
  open questions, and stories.
- **[File map](docs/FILES.md)** — what each file in the repo does.

## License

[Apache-2.0](LICENSE) — matching upstream Bluefin.
