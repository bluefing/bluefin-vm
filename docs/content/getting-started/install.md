# Installation

`bluefin-vm` installs from a [personal Homebrew tap](https://github.com/bluefing/homebrew-tap) and shells out to `tart`
(the VM runtime), which lives in OpenAI's own third-party tap. First-time setup requires trusting **two** taps — tart's,
then this one.

```bash
# 1. tart's tap (provides tart + its softnet helper) — tap and trust it once:
brew tap openai/tools
brew trust openai/tools

# 2. the tool (the fully-qualified name self-trusts this one formula):
brew install bluefing/tap/bluefin-vm
```

Brewfile configuration, installation and trust details are described in the
[tap's README](https://github.com/bluefing/homebrew-tap).
