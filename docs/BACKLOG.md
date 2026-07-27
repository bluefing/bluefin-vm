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
  README. Known gap: no suspend (BL-10).

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

### BL-5 — Fix `create-vm` qcow2→raw non-sparse conversion  ·  `backlog` · `S`

**As** a maintainer,
**I want** the qcow2→raw conversion to stay sparse,
**so that** it doesn't fill the host disk.

- **Acceptance:**
  - [ ] Conversion yields a sparse raw, **or** the flow builds raw directly
        (current default, which sidesteps it).

### BL-6 — Seed the VM login from the host username  ·  `backlog` · `S`

**As** a Mac user,
**I want** a locally built VM's account to match my username,
**so that** the VM feels like mine.

- **Acceptance:**
  - [ ] Derive the `config.toml` user from the host `$USER` (or an explicit
        argument); keep a documented test-only fallback.
- **Notes:** Only personalises locally built seeds — downloaded seeds need
  BL-8.

### BL-7 — Decide the brew package shape; request it in the tap  ·  `backlog` · `M`

**As** a Mac user,
**I want** `brew install` to hand me the Bluefin VM experience,
**so that** setup is one command.

- **Acceptance:**
  - [ ] Package shape decided (ROADMAP question 2), including the
        update/VM-state story.
  - [ ] The artifact it fetches or builds exists somewhere stable with a
        version scheme.
  - [ ] Formula requested in the `ublue-os` tap; `brew install` → first boot
        works end to end.
- **Notes:** Product rule: shipped tooling must never implicitly replace an
  existing VM — if a VM exists, boot it; re-seed only via an explicit reset.

### BL-8 — Spike: first-boot provisioning via the durable share  ·  `backlog` · `M`

**As** a Mac user,
**I want** the VM to create *my* account on first boot,
**so that** a downloaded seed feels personal — without a greeter.

- **Acceptance:**
  - [ ] The image ships a oneshot service (ordered after the share mount,
        conditioned on a provision file in the share) that creates the user
        — username, ssh public key, optional autologin — then removes the
        file.
  - [ ] Host tooling writes the provision file before first boot.
  - [ ] No file → fallback to the baked test login.
  - [ ] Secrets stance: public keys only; no passwords through the share.

---

## Blocked / waiting

### BL-9 — Promote the default image to stable once it ships GNOME 50  ·  `blocked` · `S`

**As** a maintainer,
**I want** `default_image` back on the stable `:lts-arm64` when it carries GNOME 50,
**so that** we track the stable channel instead of a testing tag.

- **Acceptance:**
  - [ ] Detect stable shipping matched gnome-shell/mutter 50; update
        `default_image` and the README image note.
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

*(none yet — move finished stories here with a completion date)*
