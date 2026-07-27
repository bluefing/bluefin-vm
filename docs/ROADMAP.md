# Roadmap

Goal: Bluefin as a fast, low-effort VM on Apple Silicon macOS — good enough
that, fullscreened, you can't tell it isn't bare metal. macOS keeps the
hardware; the VM is built from the upstream bootc container, not installed by
hand.

## Decided

- **Runtime: Tart.** UTM was evaluated first and mostly worked; Tart was
  chosen for simplicity and fit — CLI end to end, installable as a brew
  formula, scriptable share/ssh/lifecycle, headless and CI capable. Known
  trade-off: no Linux-guest suspend yet (tracked in the backlog).
- **Source image:** an arm64 Bluefin bootc container, regenerated from
  upstream so it stays fresh. (arm64 only lives in the LTS line *today* — a
  moving upstream constraint, not a choice; which tag currently boots is in
  README "Which image?".)
- **Build:** `bootc-image-builder` via `bin/build-disk.sh`, identical locally
  (Docker/Colima) and in CI (ARM64 Linux runner).
- **Delivery is a one-time seed:** the VM self-updates via bootc after first
  boot. Tooling never re-seeds an existing VM — updates arrive inside;
  re-seeding is an explicit, clearly destructive reset.
- **Install: brew, via the `ublue-os` tap.** What the package contains is
  undecided (open question 2).
- **First run: no greeter.** The VM boots to a usable desktop.

## Proposed

- **`bluefin-vm` as a CLI/TUI tool:** brew installs a small tool that
  downloads the CI-built seed, configures per-user choices (share location,
  username + ssh key), and drives the runtime. Principles: assume as little
  as possible about the host (no container toolchain, no local builds);
  images build upstream in CI.
- **Flavours:** Bluefin variants are just different bootc images (e.g.
  `bluefin-dx`), so a flavour seed is this pipeline pointed at that image.
- **Durable-data model:** three tiers — OS (replaced by updates), VM home
  (survives updates, dies on re-seed), host share (survives everything).
  The VM is disposable; the share is durable.

## Open questions

1. **Seamlessness:** retina crispness, GPU smoothness, dynamic resolution —
   the full parity audit against the goal.
2. **Brew package shape:** download the VM artifact through brew, ship a
   thin builder, or ship a thin downloader that fetches the seed
   out-of-band?
3. **Publish pipeline:** where seeds live, versioning, how updates reach
   the seed.
4. **First-boot account creation:** downloaded seeds are identical for
   every user, so the account cannot come from build time — it must be
   created at first boot or injected by host tooling.
5. **Gatekeeper:** does a downloaded artifact open without quarantine
   friction?
6. **Disk sizing:** is a 20 GiB root the right default? User-resizable?
