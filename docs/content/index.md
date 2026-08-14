# bluefin-vm

This project is a convenience wrapper that turns an upstream [Bluefin](https://projectbluefin.io) bootc container into a
running Linux VM on Apple Silicon with one command.

It delivers a provisioned desktop, no ISO, no installer, no greeter to click through.

## Performance

An `aarch64` guest under Apple's Virtualisation framework (via [Tart](https://tart.run)) runs at near-native speed. In
my experience, full-screened, you would struggle to tell it was not a bare metal install.

On my Mac air M3 it took less than three minutes to go from invoking the `up` command to a provisioned, running desktop.
Subsequent runs took around 30 seconds (the time it takes to boot) because the disk is cached on first run.

## Status

Experimental. This is a semver versioned proof-of-concept, beta release. The pipeline runs end-to-end, the runtime is
Tart, and the tool ships via a personal Homebrew tap.

## Planned

Not built yet:

- **Flavours**
    - pick a Bluefin variant (e.g. `bluefin-dx`) as the base; each is just a different upstream image.
- **Host key integration**
    - use your host ssh keys / YubiKey from inside the guest via agent forwarding, no secrets copied in.
- **Suspend / resume**
    - pause keeping the in-RAM session (waiting on Tart's Linux-guest support).
