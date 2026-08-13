# bluefin-vm

Turn the upstream [Bluefin](https://projectbluefin.io) bootc container into a
running Linux VM on Apple Silicon with one command — a provisioned desktop, no
ISO, no installer, no greeter to click through.

An `aarch64` guest under Apple's Virtualisation framework (via
[Tart](https://tart.run)) runs at near-native speed. What this project adds is
the convenience around it:

- **One command up** — `bluefin-vm up` downloads the published disk, imports it
  into Tart, provisions your account, and boots the desktop.
- **Reproducible build** — the OS image is a bootc `Containerfile`; the disk is
  built from it declaratively. Infrastructure-as-code, the OCI way.
- **A daily driver, not a throwaway** — durable data lives on a host share and
  the disk is re-imageable, so you rebuild the VM without losing state.
- **Guest tweaks that matter** — clipboard, a shared folder, ssh, and display
  scaling, set up so the VM feels like a real workstation.

It's aimed at Bluefin users and developers who are already comfortable with
cloud-native workflows — declarative config, ssh agents, tailscale. The
[interactive TUI](getting-started/install.md) exposes every choice but
requires none: the defaults boot a working, personal VM.

## Status

Experimental — a working proof-of-concept, not a stable release. The pipeline
runs end-to-end, the runtime is Tart, and the tool ships via a personal
Homebrew tap.
