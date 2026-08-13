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

`bluefin-vm up` downloads the published disk, imports it into Tart, provisions
your account, and boots the VM. The formula installs only the tool (~1.6 MB);
the multi-GB disk is downloaded at runtime.

Brewfile installs and the trust details are in the
[tap's README](https://github.com/bluefing/homebrew-tap).

## Status

Experimental — a working proof-of-concept, not a stable release.

- Pipeline works end-to-end.
- Runtime is Tart.
- Tool ships via a personal Homebrew tap.

## Planned

Not built yet — where this is headed:

- **Flavours** — pick a Bluefin variant (e.g. `bluefin-dx`) as the base; each is
  just a different upstream image.
- **Host key integration** — use your host ssh keys / YubiKey from inside the
  guest via agent forwarding, no secrets copied in.
- **Suspend / resume** — pause keeping the in-RAM session (waiting on Tart's
  Linux-guest support).

## Docs

Documentation is a [Zensical](https://github.com/squidfunk/zensical) site under
[`docs/`](docs/) — build it locally with `cd docs && uv run zensical serve`.
Module docs double as `just <module> help` in the terminal:

- **[Building](docs/content/just/build.md)** — build the image and disk yourself:
  which upstream image to use, and local and CI builds.
- **[Running a VM](docs/content/just/tart.md)** — the ways up, ssh, display
  density, the shared folder, and one-time guest setup for stock disks.
- **[The tool](docs/content/just/cli.md)** — what `bluefin-vm` does and how the
  recipes drive it.
- **[First-boot provisioning](docs/content/guide/provisioning.md)** — the account
  and credential model.
- **[Orientation](docs/content/reference/repo-structure.md)** — a map of what
  lives where and how the pieces relate.

Design notes, decisions, and the backlog live in
[`docs/internal/`](docs/internal/) (not part of the published site).

## License

[Apache-2.0](LICENSE) — matching upstream Bluefin.
