# Backlog

Actionable stories. Strategy and open questions live in
[`ROADMAP.md`](ROADMAP.md).

Format: user story + acceptance criteria. Status: `ready` · `backlog` ·
`blocked` · `done`. Size: `S` (hours) · `M` (~a day) · `L` (multi-day).
Finished stories move to **Done** with the date.

---

## Ready

### BL-1 — File the arm64 findings upstream  ·  `ready` · `S`

**As** a maintainer,
**I want** the arm64 findings filed as upstream issues,
**so that** the image improves for everyone and this repo's workarounds shrink.

- **Acceptance:**
  - [ ] Positive result reported: the GNOME 50 testing image boots to a
        desktop on Apple Silicon; `ujust`, distrobox, and the dx switch work
        on aarch64.
  - [ ] Bug filed: `umotd` is broken on arm64 (wrong-arch binary, missing
        `env.sh`).
  - [ ] Bug filed: `spice-vdagent` never autostarts on GNOME 50 (legacy
        autostart entry ignored; packaged user unit is static and unordered)
        — clipboard is dead by default in VMs. Include the fix carried in
        `image/Containerfile`.
  - [ ] Issue links recorded here.

### BL-11 — Reconcile config.toml disk sizing with what the builder accepts  ·  `ready` · `S`

**As** a maintainer,
**I want** config.toml to carry only customizations the builder actually applies,
**so that** the docs and the built disk match reality.

- **Acceptance:**
  - [ ] Explain the first-CI-run warning `blueprint validation failed for
        image type "raw": customizations.filesystem: not supported` — raw-only
        limitation, schema change (e.g. moved under `customizations.disk`), or
        silently inert all along?
  - [ ] Confirm whether the 20 GiB root is applied for any format we ship; if
        not, fix the schema or drop it and correct config.toml's comment + the
        docs that cite the `min-free-space` rationale.
  - [ ] Re-verify a raw build still boots with whatever sizing resolves to.
- **Notes:** Surfaced by the first CI run (2026-07-27): the build succeeded
  and produced a 22 GB raw regardless, so the default may now suffice.
  Investigate the current bootc-image-builder schema before changing anything.

### BL-16 — `bluefin-vm up` must not replace an existing VM  ·  `ready` · `S`

**As** a user,
**I want** a re-run of `bluefin-vm up` to boot my existing VM, not silently
re-image it,
**so that** I never lose VM state to a repeat command.

- **Acceptance:**
  - [ ] If the named VM exists, `up` boots it — no re-import (matches the
        product rule and `just tart up`'s guarded behaviour).
  - [ ] Re-imaging is explicit and clearly destructive (e.g. `--reset` or a
        separate subcommand).
  - [ ] docs/modules/cli.md documents CLI `up` vs the `just tart up` recipe.
  - [ ] Document the update / VM-state story (folded from BL-7): `brew upgrade`
        replaces the tool only, the VM in `~/.tart` untouched; re-imaging is
        explicit.
- **Notes:** Today `up()` calls `core::tart::import` unconditionally, which does
  `tart delete` + `tart create` — a second `up` destroys the VM. `just tart up`
  is already incremental; the Rust CLI isn't. Surfaced verifying BL-7
  (2026-07-29). Product rule: shipped tooling must never implicitly replace an
  existing VM — if a VM exists, boot it; re-image only via an explicit reset.

---

## Backlog

### BL-2 — Fullscreen parity audit  ·  `backlog` · `L`

**As** a Mac user,
**I want** the VM to feel indistinguishable from bare metal when fullscreened,
**so that** it meets the project goal.

- **Acceptance:**
  - [ ] Audit retina crispness, GPU smoothness, dynamic resolution,
        clipboard, shared folders.
  - [ ] Log each gap as its own story.
- **Notes:** Display-density and clipboard behaviour are documented in
  `docs/modules/tart.md`. Guest resolution and desktop scale are now
  user-settable per VM via `bluefin-vm tui` (the Refit toggle, off = fixed
  resolution + scale applied at first boot); the audit should judge that
  against true fullscreen parity. Known gap: no suspend (BL-10).

### BL-3 — Spike: persistent osbuild build cache  ·  `backlog` · `M`

**As** a maintainer,
**I want** repeat builds of an unchanged image to skip osbuild work,
**so that** iteration isn't a full rebuild every time.

- **Acceptance:**
  - [ ] Mount the builder's `/store` to a named volume; time a second
        identical build; decide whether to keep it.

### BL-4 — Decide: pin digest vs moving tag  ·  `backlog` · `S`

**As** a maintainer,
**I want** an explicit image-pinning decision,
**so that** reproducibility-vs-freshness is intentional.

- **Acceptance:**
  - [ ] Choose the default (moving tag vs pinned `@sha256`); document the
        trade-off and the `-i` override.

### BL-13 — Dev env: use host ssh keys / YubiKey *from* the guest  ·  `backlog` · `M`

**As** a developer,
**I want** my host ssh keys and YubiKey usable from inside the VM,
**so that** git/ssh and signing work without copying private keys into it.

- **Acceptance:**
  - [ ] Agent forwarding is the supported path — e.g. a `bluefin-vm ssh` that
        forwards the host agent, so guest git-over-ssh signs via the host
        (YubiKey touch on the Mac); no private key ever in the VM.
  - [ ] Desktop story decided: forwarding only covers ssh sessions, not the
        autologin desktop — evaluate USB/YubiKey passthrough (Tart/VZ support)
        or a desktop-side agent.
  - [ ] Documented.
- **Notes:** Distinct from provisioning, which installs a *public* key to
  get you *into* the VM. This is using keys *from* it. Nothing host-specific is
  baked into the disk.

### BL-14 — Provisioning: robust ssh-key selection  ·  `backlog` · `S`

**As** a user with non-standard or multiple ssh keys,
**I want** provisioning to find or let me choose the right public key,
**so that** `up` installs my key without me hand-specifying it each time.

- **Acceptance:**
  - [ ] Auto-detect beyond the three fixed names: use a single non-standard
        `~/.ssh/*.pub`; don't guess when several exist.
  - [ ] A configurable default (e.g. `BLUEFIN_VM_SSH_KEY`) so FIDO/multi-key
        users set it once.
  - [ ] The interactive TUI, when it lands, lets the user pick among keys
        rather than the tool guessing.
  - [ ] `--ssh-key` stays the explicit override.
- **Notes:** Today `default_ssh_key()` matches only
  `id_ed25519|id_ecdsa|id_rsa.pub`; FIDO `sk` keys and multi-key setups need
  `--ssh-key` (and `sk` keys need the token present to authenticate — expected).

### BL-15 — Provisioning: GUI polkit auth for the password-less account  ·  `backlog` · `S`

**As** a user of a provisioned VM,
**I want** graphical privilege prompts (polkit) to work for my password-less account,
**so that** GUI admin actions aren't blocked by a password I don't have.

- **Acceptance:**
  - [ ] Decide: a scoped polkit rule (`/etc/polkit-1/rules.d/`) letting the
        provisioned user authorise admin actions without a password — mirroring
        the passwordless-sudo posture — vs. leaving it to `bluefin-vm-harden`
        (set a password → polkit works).
  - [ ] If a rule: scope it to the provisioned user, consistent with the
        per-user sudoers approach.
  - [ ] Verify a GUI-gated admin action works on the autologin desktop.
- **Notes:** The passwordless-sudo drop-in covers CLI `sudo` only; polkit is a
  separate mechanism (PROVISIONING.md Limits). Same "no password" root cause as
  autologin / passwordless-sudo / lock-disable — the one such consequence not
  yet handled. Narrow in practice (much Bluefin admin is CLI or Flatpak).

### BL-17 — Clean up `image/provision.sh`  ·  `backlog` · `S`

**As** a maintainer,
**I want** `provision.sh` to drop the embedded Python and carry fewer concerns,
**so that** the first-boot logic stays readable and maintainable.

- **Acceptance:**
  - [x] Replace the embedded `configparser` heredoc (editing
        `/etc/gdm/custom.conf` for autologin) with a non-Python approach —
        `crudini`, a GDM drop-in, or a tidy `sed` — or extract it to a helper.
  - [ ] Review the script for other cruft; decompose if it's carrying too many
        concerns (account, ssh, sudoers, autologin, dconf lock-disable).
- **Notes:** Flagged 2026-07-29. Heredoc extracted 2026-07-31 to a standalone
  helper (`/usr/libexec/bluefin-vm-gdm-autologin`, a `configparser` script);
  autologin verified end-to-end. The decompose review remains.

---

## Blocked / waiting

### BL-9 — Promote the default image to stable once it ships GNOME 50  ·  `blocked` · `S`

**As** a maintainer,
**I want** `default_image` back on the stable `:lts-arm64` when it carries GNOME 50,
**so that** we track the stable channel instead of a testing tag.

- **Acceptance:**
  - [ ] Detect stable shipping matched gnome-shell/mutter 50; update
        `default_image` and the image note in `docs/modules/build.md`.
- **Notes:** Blocked on upstream — stable currently ships a broken
  gnome-shell/mutter pairing on arm64.

### BL-10 — Add `tart suspend` once tart supports Linux guests  ·  `blocked` · `S`

**As** a Mac user,
**I want** suspend/resume instead of shutdown,
**so that** pausing work keeps the in-RAM session.

- **Acceptance:**
  - [ ] `just tart suspend` then `just tart start` resumes with the session
        intact; window-close behaviour documented.
- **Notes:** Blocked in tart, not the framework: saving Linux VM state
  works, but tart's resume path guards on a macOS-only platform check —
  openai/tart#1177. Interim: `stop`/`up` cycles are cheap.
- **Warning:** running `tart suspend` on a Linux VM today half-works and
  leaves the VM unbootable. Recovery:
  `rm ~/.tart/vms/<name>/state.vzvmsave` (disk unaffected).

---

## Done

### BL-6 — Set the VM login from the host username  ·  `done` 2026-07-31 · `S`

Met via first-boot provisioning, not the `config.toml` derivation the story
proposed: `up-provisioned` (and `bluefin-vm up`) stage a `$USER` account the
guest oneshot creates on boot, so a locally-built disk feels like yours.
`config.toml` still bakes the `bluefin` login as the documented test-only
fallback.

### BL-5 — Fix `create-vm` qcow2→raw non-sparse conversion  ·  `done` 2026-07-31 · `S`

Sidestepped by the raw-direct flow — the story's own second acceptance. `up` /
`up-patched` build with `build-disk.sh -f raw` and Tart boots raw, so the
non-sparse qcow2→raw conversion was never on the path. It later dropped out
entirely when the import logic consolidated into the `bluefin-vm` CLI (raw
only); bring it back only if a qcow2 import is ever actually needed.

### BL-7 — Ship `bluefin-vm` via a Homebrew tap  ·  `done` 2026-07-31 · `M`

Shipped from an own tap (`bluefing/homebrew-tap`): a prebuilt arm64 binary — the
tool, not the disk — on a GitHub Release, built by
`.github/workflows/release.yml` on a `v*` tag via `bin/package-cli.sh` (local and
CI identical). The formula `depends_on openai/tools/tart`, pins arm64, and scans
its version from the url. `v0.1.0` cut 2026-07-29 and verified end-to-end:
`brew install bluefing/tap/bluefin-vm` → `bluefin-vm up` → first boot, `brew
audit --online` clean. Own tap first to iterate without upstream review; move to
`ublue-os` once stable. The update / VM-state box moved to BL-16.

### BL-12 — `bluefin-vm` tool: disk → running VM pipeline  ·  `done` 2026-07-29 · `M`

One command turns a published disk into a running VM: `up` chains `download`
(resumable, checksum-verified) → `extract` (streams `image/disk.raw` out of the
zip64 archive) → `import` (ports `create-vm.sh`: `tart create --linux`,
APFS-clone the raw in, set cpu/memory/display + `--display-refit`) → provision →
`tart run` detached with the durable share attached. The individual steps are
also exposed as subcommands for debugging. `import`/`up` shell out to `tart`
(BL-7's formula depends on it).

The import → provision → boot leg was live-verified as part of BL-8
(2026-07-28). End-to-end from the published R2 disk (download → extract) is the
clean-Mac check folded into BL-7.

### BL-8 — First-boot provisioning via the durable share  ·  `done` 2026-07-28 · `M`

A downloaded disk is personalised at first boot: the host writes the account
into the share (`core::provision` / `bluefin-vm provision`, called by `up`); a
gated oneshot (`bluefin-vm-provision.service` + `image/provision.sh`) creates
it, then clears the file. Credential model: pubkey-only → autologin + a scoped
passwordless-sudo drop-in (`bluefin-vm-harden` reverts it). No provision file →
the baked test login.

**Live-verified 2026-07-28** on a patched-image boot: the oneshot ran and
cleared the share, key-based ssh worked under enforcing SELinux, scoped
passwordless sudo worked, autologin reached the desktop (seat0 session), and
the account password is locked.
