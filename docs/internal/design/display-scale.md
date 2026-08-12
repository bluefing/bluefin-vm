# Display scale

How a profile's `scale` percentage becomes the guest desktop's scale, and why
the mechanism is session-time snapping via mutter's DBus API rather than a
pre-written `monitors.xml`.

## Verified behaviour (GNOME Shell 50, virtio-gpu `Virtual-1`)

These facts were established against a running guest with
`org.gnome.Mutter.DisplayConfig` probes; each one shapes the design.

- **mutter applies only scales from a per-mode supported set.** A
  `monitors.xml` carrying any other value is silently discarded at session
  start: the desktop comes up at 1x and mutter rewrites the file with
  `<scale>1</scale>`. Nothing is logged. This is how a provisioned 150% at
  2560x1600 produced an unscaled desktop while the same 150% at 1920x1200
  worked.
- **The supported set is irregular and unknowable outside a session.**
  `GetCurrentState` reports it per mode: 1920x1200 offers
  `1.0 1.25 1.3333 1.5 1.6667 2.0`; 2560x1600 offers the same *minus 1.5* plus
  `2.5 2.6667`; 1792x1344 offers 1.75; 1366x768 offers only 1.0. The values are
  float32-precision doubles (`1.3333333730697632`), so no host-side or
  boot-time computation can reproduce them — the running mutter is the only
  source.
- **A scale not bit-exact in the set crashes the compositor.**
  `ApplyMonitorsConfig` with such a value — including a rounded `1.3333` for
  the set's `1.3333333730697632`, and including method 0 (verify) — aborts
  gnome-shell (SIGABRT, coredump) and ends the session. Verify is therefore
  not a safe acceptance oracle; the only safe validation is client-side
  membership, sending doubles copied verbatim from `GetCurrentState`.
- **Method 1 (temporary) applies instantly with no confirmation dialog.**
  Method 2 (persistent) applies, writes `monitors.xml`, but raises the
  keep-or-revert dialog, which reverts on timeout — unusable unattended.
- **A `monitors.xml` read at session start applies silently** (no dialog),
  provided every value is acceptable: the mode must match a real mode of the
  connector in width, height *and* refresh rate, and the scale must be in that
  mode's supported set. mutter's own persistent write is the format template:
  full-precision scale, three-decimal rate.
- **Fractional scales need no feature flag.** A 1.5 scale was offered,
  applied, and persisted with `org.gnome.mutter experimental-features` empty
  (`@as []`) — no dconf enablement belongs in the image.

## Design

The host cannot know what the guest display will support, so the profile's
`scale` is a **target**: the guest applies the nearest scale the live mode
supports. Split across the existing seams:

- **Host** (unchanged): the TUI stores `scale` as a percentage in the profile;
  `provision` writes it to the share only when refit is off (with refit on the
  mode follows the window, so there is nothing stable to scale against).
- **provision.sh** (first boot, root, no session): cannot query mutter — the
  DisplayConfig API lives on the session bus and exists only inside a running
  session. It hands the request over: copy the percentage to
  `~/.config/bluefin-vm/scale-request` in the new account's home.
- **`bluefin-vm-apply-scale` (a user oneshot at graphical-session):** gated on
  the request file, so every session without one skips at zero cost. It waits
  for mutter to own the DisplayConfig name, reads the live mode's supported
  set, snaps the request to the nearest entry, applies it with method 1
  (instant, silent), writes `~/.config/monitors.xml` with the exact learned
  values so later sessions come up scaled with no dialog, and deletes the
  request.

The failure contract protects the session first: the oneshot never transmits
a value it did not read from `GetCurrentState`, which rules out the
compositor abort, and any other failure logs and leaves the desktop at 1x —
the same posture as no request at all. Exit codes tell the truth: 0 for
applied or nothing to do, 1 for an invalid or refused request (consumed so it
cannot loop), and `EX_TEMPFAIL` for a transient failure (mutter not up within
the wait), which keeps the request so the next login retries. The unit treats
`EX_TEMPFAIL` as success so retries stay quiet.

The oneshot is Python — the only Python in the repo, which the guest platform
justifies: it is a Fedora/GNOME system where python3 with PyGObject is the
native glue for structured DBus work, and the doubles must survive the
round-trip bit-exact. The image build asserts `import gi` so a base image
that stops shipping PyGObject fails at build time rather than at login.

## Testing

The logic worth guarding is the snap: nearest-supported selection over real,
irregular, float32 arrays. It is a pure function, unit-tested offline against
supported-scales sets captured from the guest. The DBus glue and unit wiring
are thin and need a booted session — one tier-2 assertion (desktop scaled,
`monitors.xml` written, request consumed), not a matrix.

## Rejected

- **Pre-writing `monitors.xml` with a fractional scale** (the original
  mechanism): predicts what mutter will accept, and loses silently when the
  prediction is off. Prediction cannot be fixed host-side because the accepted
  values are per-mode float32 artefacts of the running compositor.
- **Restricting scale to the integers 100/200** (always accepted, no session
  machinery): rejected because the default 1920x1200 profile is best served by
  150% — 200% leaves a 960x600 logical desktop, 100% is small on a HiDPI host
  window. The snap keeps fractional targets honest at exactly one extra moving
  part.
- **`ApplyMonitorsConfig` method 2 (persistent)** for the oneshot: the
  keep-or-revert dialog reverts unattended sessions.
- **bash with `gdbus`** for the oneshot: `gdbus` prints the reply as one
  GVariant text blob, and extracting exact doubles from nested tuples with
  shell tools is the same fragile text-parsing the DRM-debugfs mechanism
  died of — with a compositor abort as the cost of a parse slip.
- **gjs or a cross-compiled Rust binary** for the oneshot: gjs is present
  wherever gnome-shell is but untestable from the host and an orphan in this
  repo; Rust in the guest needs a cross-compilation stage the image build
  doesn't have, for ~200 lines of glue.
