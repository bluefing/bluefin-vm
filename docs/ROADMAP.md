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
- **Install: brew installs a thin tool** (`bluefin-vm`) — *not* a multi-GB
  cask, *not* a local build. The tool downloads the CI-built seed and imports +
  provisions it locally (package shape "downloader", decided 2026-07-28).
  **Rust** — a single static binary, and aligns with the upstream stack (bootc
  is Rust). `clap` for the CLI now, `ratatui` for a TUI later, both driving one
  UI-agnostic core so the CLI isn't throwaway.
  - **Distribution (decided 2026-07-29):** the formula ships a **prebuilt
    arm64 binary attached to a GitHub Release** (built by
    `.github/workflows/release.yml`, versioned by `v*` tag). The tool is ~1.6
    MB, so GitHub Releases fits it — the size objection that ruled GH Releases
    out was about the multi-GB *seed*, a separate artifact the tool fetches at
    runtime. This keeps releasing the tool fully decoupled from the seed's R2
    hosting. Shipped first from an **own tap** (`bluefing/homebrew-tap`) to
    iterate without upstream review; move to the `ublue-os` tap once stable.
- **Seed hosting: Cloudflare R2**, served at `projectbluefin.dev`, **live
  2026-07-28** (`projectbluefin.dev/bluefin-vm-raw-arm64.zip` — anonymous,
  resumable). Long-term the bucket builds/hosts from repo releases, so this
  side ships no large files itself. GitHub Releases ruled out: the seed is
  ~2.75 GiB zipped (1.989 GiB even at max zstd — clears the 2 GiB cap by only
  ~11 MiB, too fragile as the image grows).
- **Upstreaming:** the repo moves into `ublue-os` when ready (Jorge offered;
  user-paced — prove on the personal repo first).
- **First run: no greeter, provisioned to the user.** The host writes the
  account (username + ssh public key) into the share pre-boot; a guest oneshot
  creates it on first boot and the VM autologs in. Pubkey-only, no password —
  hence autologin (desktop) plus a scoped passwordless-sudo rule (admin), the
  disposable-dev-VM posture. No provision data → the baked test login. (BL-8.)

## Proposed

- **Flavours:** Bluefin variants are just different bootc images (e.g.
  `bluefin-dx`), so a flavour seed is this pipeline pointed at that image.
- **Durable-data model:** three tiers — OS (replaced by updates), VM home
  (survives updates, dies on re-seed), host share (survives everything).
  The VM is disposable; the share is durable.

## Open questions

1. **Seamlessness:** retina crispness, GPU smoothness, dynamic resolution —
   the full parity audit against the goal.
2. **Brew package shape — decided 2026-07-28:** a thin Rust downloader tool
   (see Decided).
3. **Publish pipeline:** seeds live in R2 at `projectbluefin.dev` (done);
   still open: versioning, and wiring the bucket to build/host from releases.
4. **First-boot account creation — decided 2026-07-28:** host tooling writes
   the account into the share; a guest oneshot creates it on first boot (see
   Decided, BL-8).
5. **Install trust — two layers.** *Tap trust* (the formula's Ruby, which runs
   with the user's privileges) is handled: a fully-qualified `brew install`
   self-trusts the one formula, and Brewfile users add `trusted: true` for the
   non-official tap (documented in README "Install with Homebrew"). *Gatekeeper*
   is still open — does the unsigned/un-notarised binary open without quarantine
   friction on a clean Mac?
6. **Disk sizing:** is a 20 GiB root the right default? User-resizable?
