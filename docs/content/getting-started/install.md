# Installation

`bluefin-vm` installs from a Homebrew tap and shells out to `tart` (the VM
runtime), which lives in OpenAI's own third-party tap. First-time setup requires
trusting **two** taps — tart's, then this one.

```bash
# 1. tart's tap (provides tart + its softnet helper) — tap and trust it once:
brew tap openai/tools
brew trust openai/tools

# 2. the tool (the fully-qualified name self-trusts this one formula):
brew install bluefing/tap/bluefin-vm
```

Then bring a VM up:

```bash
bluefin-vm up
```

`bluefin-vm up` downloads the published disk, imports it into Tart, provisions
your account, and boots the VM. The formula installs only the tool (~1.6 MB);
the multi-GB disk is downloaded at runtime.

Brewfile installs and the trust details are in the
[tap's README](https://github.com/bluefing/homebrew-tap).

## Customising the VM

`bluefin-vm up` uses sane defaults, creates the VM when it is missing, and
simply boots it when it exists. To change the account, ssh key, CPU/memory,
or display resolution and scale, run `bluefin-vm tui` — it writes a per-VM
profile that later commands reuse. Account and resource changes take effect
on a fresh VM (`bluefin-vm up --replace`); the share settings apply on every
boot. See the
[tool module](../just/cli.md) for what the CLI does and the
[provisioning guide](../guide/provisioning.md) for what happens at first boot.
