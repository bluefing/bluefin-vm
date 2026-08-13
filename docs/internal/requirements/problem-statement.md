# Problem statement

What bluefin-vm is actually for.

## Objective

Give a developer on an Apple Silicon Mac a **slick, daily-driver Linux VM** —
specifically a Bluefin desktop — that comes up personalised in one command and is
treated as a long-lived workstation, not a scratch box.

Two properties carry the weight:

1. **It's a daily driver, not disposable.** You live in it. Durable data lives on
   a host share and the disk is re-imageable, so you can rebuild the VM from the
   published image without losing your state — the way you'd reinstall a laptop,
   not the way you'd `docker rm` a container.
2. **Zero-to-personal in one command.** `bluefin-vm up` downloads, imports,
   provisions your account, and boots. Sane defaults require no input; everything
   is overridable through the interactive TUI, which writes a per-VM profile that
   later commands reuse.

## Audience

Bluefin users and developers fluent in cloud-native workflows: comfortable with
declarative config, ssh agents, and tailscale. They expect infrastructure-as-code
idioms and will recognise the OS image as a bootc `Containerfile` and the user
environment as chezmoi — so the design leans on those rather than inventing a
bespoke config language.

## What is not an objective

- **A throwaway/sandbox VM.** The disposable framing is explicitly rejected; it
  shaped some earlier docs and is being corrected.
- **Multi-VM fleets or cloud-hosted Tart.** The target is one workstation on one
  Mac. Tooling that only pays off at fleet scale (see the declarative-tooling
  decision in `open-questions.md`) is out of scope until that changes.
- **Hardening against an attacker with the Mac.** The VM sits behind the host; the
  credential model guards against fat-fingered commands and casual LAN exposure,
  not an adversary who already owns the host.
