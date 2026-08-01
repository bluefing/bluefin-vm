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
  upstream so it stays fresh.
- **Build:** `bootc-image-builder` via `bin/build-disk.sh`, identical locally
  (Docker/Colima) and in CI (ARM64 Linux runner).
- **Delivery is a one-time disk:** the VM self-updates via bootc after first
  boot. Tooling never re-images an existing VM — updates arrive inside;
  re-imaging is an explicit, clearly destructive reset.
- **Install: brew installs a thin tool** (`bluefin-vm`) — *not* a multi-GB
  cask, *not* a local build. The tool downloads the CI-built disk and imports +
  provisions it locally (package shape "downloader", decided 2026-07-28).
  **Rust** — a single static binary, and aligns with the upstream stack (bootc
  is Rust). `clap` for the CLI now, `ratatui` for a TUI later, both driving one
  UI-agnostic core so the CLI isn't throwaway.
  - **Distribution (decided 2026-07-29):** the formula ships a **prebuilt
    arm64 binary attached to a GitHub Release** (built by
    `.github/workflows/release.yml`, versioned by `v*` tag). The tool is ~1.6
    MB, so GitHub Releases fits it — the size objection that ruled GH Releases
    out was about the multi-GB *disk*, a separate artifact the tool fetches at
    runtime. This keeps releasing the tool fully decoupled from the disk's R2
    hosting. Shipped first from an **own tap** (`bluefing/homebrew-tap`) to
    iterate without upstream review; move to the `ublue-os` tap once stable.
- **Disk hosting: Cloudflare R2**, served at `projectbluefin.dev`, **live
  2026-07-28** (`projectbluefin.dev/bluefin-vm-raw-arm64.zip` — anonymous,
  resumable). Long-term the bucket builds/hosts from repo releases, so this
  side ships no large files itself. GitHub Releases ruled out: the disk is
  ~2.75 GiB zipped (1.989 GiB even at max zstd — clears the 2 GiB cap by only
  ~11 MiB, too fragile as the image grows).
- **Upstreaming:** the repo moves into `ublue-os` when ready (Jorge offered;
  user-paced — prove on the personal repo first).
- **First run: provisioned to the user.** The host writes the account (username
  + ssh public key) into the share pre-boot; a guest oneshot creates it on first
  boot. The default is autologin + pubkey-only, no password — hence a scoped
  passwordless-sudo rule (admin), the disposable-dev-VM posture. Turning
  autologin off instead sets password == username (a public convention, no
  secret in the share), giving a normal greeter login and password sudo; whether
  *that* should be the default is an open question (below). No provision data →
  the baked test login.

## Non-goals

- **Intel Macs are out of scope.** The blocker is the runtime: Tart is
  Apple-Silicon-only, so an Intel host has nothing to run the VM with.
  Supporting Intel would mean a second runtime (UTM/QEMU), which forks the
  CLI-first, brew-installable, scriptable, headless/CI-capable design that
  motivated choosing Tart in the first place — for a platform Apple has already
  sunset. Validating it would need Intel hardware regardless.

## Proposed

- **Flavours:** Bluefin variants are just different bootc images (e.g.
  `bluefin-dx`), so a flavour is this pipeline pointed at that image.
- **Durable-data model:** three tiers — OS (replaced by updates), VM home
  (survives updates, dies on re-image), host share (survives everything).
  The VM is disposable; the share is durable.

## Open questions

1. **Seamlessness:** retina crispness, GPU smoothness, dynamic resolution —
   the full parity audit against the goal.
2. **Brew package shape — decided 2026-07-28:** a thin Rust downloader tool
   (see Decided).
3. **Publish pipeline:** disks live in R2 at `projectbluefin.dev` (done);
   still open: versioning, and wiring the bucket to build/host from releases.
4. **First-boot account creation — decided 2026-07-28:** host tooling writes
   the account into the share; a guest oneshot creates it on first boot.
5. **Install trust — verified 2026-07-29.** *Tap trust*: a fully-qualified
   `brew install` self-trusts our formula, but the `tart` dependency (and its
   `softnet` dep) come from the unofficial `openai/tools` tap, which must be
   tapped + trusted too — documented in the README (Brewfile users add
   `trusted: true`). *Gatekeeper*: the unsigned/un-notarised
   arm64 binary runs with no quarantine friction (brew strips quarantine on
   formula downloads; the ad-hoc signature suffices to execute) — confirmed on
   the brew-installed tool.
6. **Disk sizing:** is a 20 GiB root the right default? User-resizable?
7. **Default credential posture — decision pending (surfaced 2026-08-01).** The
   default is autologin + password-less. Testing showed that posture carries
   three papercuts, all rooted in autologin bypassing the password entry GDM/pam
   would otherwise use:
   - **Keyring:** the first secret-storing app pops a "create keyring" dialog,
     then you must pick *unencrypted + silent* (blank password) or *encrypted +
     an unlock prompt every boot*. Autologin can't give encrypted-and-silent.
   - **polkit:** GUI privilege prompts are unusable with no password (BL-15).
   - **Logout:** logging out strands you at the greeter — reboot to recover.

   The autologin-**off** posture (implemented: password == username, a public
   convention, so no secret crosses the share) clears all three at once — the
   login keyring is encrypted *and* auto-unlocked by pam, polkit works, the
   greeter works — for the cost of one login at boot. Both are wired to the
   `setup` Autologin toggle; the open question is which is the **default**.
   Leaning greeter+password for a VM whose desktop you actually use, with
   autologin as the opt-in "don't make me type it" mode plus documented caveats
   — but zero-friction boot is a real draw for throwaway use.
