# Testing strategy

How bluefin-vm is tested as it grows, so configuration variations and behaviour
stay covered without manual regression sweeps. The governing rule: **test each
seam at the cheapest tier that can actually exercise it**, so most breakages fail
in seconds and only genuinely runtime ones need a VM.

## The chain of seams

A setting travels through four stages, and a regression can appear at any arrow:

```
config.toml ──► flag files in the share ──► guest system state ──► live VM behaviour
   (Rust)          (provision::write)           (provision.sh)       (systemd/sshd/PAM)
```

Each arrow is owned by a tier below. Today the first two are covered (Rust unit
tests); the last two have no automated coverage — that's the gap this strategy
closes.

## Tiers

| Tier | Proves | Tooling | Speed | Where |
| --- | --- | --- | --- | --- |
| **0 · Unit** | config ↔ flag files, arg building, TUI form logic | `cargo test`, bats dry-runs | seconds | local + CI, every push |
| **1 · Guest-logic** | `provision.sh` decisions → correct files/accounts, across the config matrix | bats + podman/docker | seconds–a minute | local + CI (Linux), every PR |
| **2 · End-to-end boot** | real runtime: systemd ordering, SELinux enforcing, sshd/PAM live, greeter, share, display | Tart + the guest assertion script | minutes | local on-demand + CI scheduled |

### Tier 0 — Unit (have)

`cargo test` over `cli/src` (config round-trip, account resolution,
`provision::write` flag-file output, tart arg building, TUI form logic) plus the
offline bats suite (`just`-recipe wiring, `config.toml`, `package-cli.sh`
dry-runs). No I/O beyond temp files; runs anywhere. This is the fast inner loop
(`just test`) and stays that way.

### Tier 1 — Guest-logic integration (the high-value new piece)

`image/provision.sh` runs cleanly in a plain Linux container: its VM-only steps
already self-guard (`restorecon` gated on `/sys/fs/selinux/enforce`, the sshd
reload is `|| true`, the scale block degrades when DRM debugfs is absent). A test:

1. starts a throwaway `fedora` container,
2. drops synthetic `…/.bluefin-vm/` files — username, key, and a chosen
   combination of `passwordless-sudo` / `disable-ssh-password` / `scale`,
3. runs `provision.sh` as root,
4. asserts the result: user in `wheel`; `authorized_keys` 600/owner;
   `password == username` in `/etc/shadow`; the sudoers drop-in present *iff*
   passwordless; the sshd drop-in present *iff* ssh-off; the share cleared.

Then it **matrixes** over the flag combinations — every posture permutation, no
VM, on any Linux runner, every PR. Enabler: make `provision.sh`'s `pdir`
overridable (`pdir="${BLUEFIN_VM_PDIR:-/var/mnt/…}"`); nothing else changes.

**Boundary:** Tier 1 tests the script's decisions and file outputs. It cannot see
SELinux-enforcing behaviour, a running sshd, or a real login — the container has
none of those. That is Tier 2's job.

### Tier 2 — End-to-end boot, kept small

Build the patched image + disk, provision a profile, import, boot via Tart, wait
for ssh, run assertions over ssh, tear down. The only tier that catches the
runtime integration: provision-unit ordering (`Before=gdm`), the SELinux
`ssh_home_t` relabel actually mattering, sshd honouring the drop-in live, the
greeter existing, the share mounting, display/clipboard. Run a **handful of
representative profiles** (default; both toggles hardened; scale-on) — each is a
real boot, so no full matrix here.

## Test layout

Where each tier's tests live, so the separation is structural and doesn't smear
as tests accrue. This is the target; the tree today is a flat `tests/*.bats` plus
`tests/integration/` and `tests/smoke/`, and moving to the layout below is a
follow-up.

| Tier | Location | Kind |
| --- | --- | --- |
| 0 · Unit | `cli/src/**` (`#[cfg(test)]`, `cargo test`) | true unit tests, in-crate by Rust convention |
| 0 · Offline | `tests/offline/*.bats` | script arg/dry-run and recipe/config contracts; no external deps |
| 1 · Integration | `tests/integration/` | `provision.sh` in a container (the config matrix) |
| 2 · End-to-end | `tests/e2e/` | in-VM boot checks over ssh |

```
tests/
  offline/      # Tier 0 bats (moved from the tests/ root)
  integration/  # Tier 1
  e2e/          # Tier 2 (renamed from smoke/)
  README.md     # this taxonomy, in brief, beside the tests
```

Naming decisions:

- **`offline/`, not `unit/`.** These bats test *contracts* — argument handling,
  dry-run command construction, recipe wiring, config validity — not isolated
  units. The true unit tests are the Rust `#[cfg(test)]` modules, which stay in
  the crate (they reach private items; Rust's own `tests/` dir is for public-API
  integration only). `tests/README.md` points there so the units don't look
  missing.
- **`e2e/`, renamed from `smoke/`.** Tier 2 grows past a smoke check into the
  over-ssh posture assertions that reuse `assert-posture.sh`.

The governing rule, stated in `tests/README.md`: **a test goes in the directory
for the cheapest tier that can run it; never add docker- or VM-dependent tests to
`offline/`.** That one rule is what keeps the separation from eroding across
sessions.

## One shared asset: posture assertions

Write the guest assertions once, parametrised by expected state (env such as
`EXPECT_PASSWORDLESS_SUDO=1`, `EXPECT_SSH_PASSWORD=0`). Tier 1 runs them inside
the container after `provision.sh`; Tier 2 runs the same script over ssh in the
booted VM. Reuse keeps the two tiers honest with each other rather than drifting.
This extends the existing check/report pattern in `tests/smoke/guest-checks.sh`.

## Local and CI mapping

- `just test` → Tier 0. Fast, offline inner loop (unchanged).
- `just test-integration` → Tier 1. Run when touching `provision.sh`; CI runs it
  every PR on a Linux runner with podman/docker.
- `just test-e2e` (or an extended `just tart smoke`) → Tier 2. Local on-demand;
  CI on a schedule.
- CI: `unit` + `integration` jobs on `ubuntu-latest` per PR; an `e2e` job on
  `schedule` + `workflow_dispatch`.

## Runner constraint for boot tests

Tart uses Apple's Virtualization.framework, which needs real Apple-Silicon virt.
GitHub-hosted macOS runners do not allow nested VMs, so Tier 2 in CI needs a
**self-hosted Apple-Silicon runner** or **Cirrus CI** (Tart's own platform).
Locally it runs on the developer's Mac. This is the open dependency to resolve
before wiring Tier 2 into CI; Tiers 0 and 1 have no such constraint.

## Build order

1. **Tier 1 first** — the most coverage per unit of effort, and it runs
   everywhere. Needs the `pdir` hook, a bats + container harness, and the
   assertion script.
2. **Refactor the smoke script** into the shared, posture-parametrised assertions
   (used by Tier 1 immediately, Tier 2 later).
3. **Tier 2** once the runner question is settled — wire boot + assert + teardown
   and a schedule.

## Fidelity variants (later)

Tier 1 on a `fedora` container tests the script's logic against a generic
userland. A variant that runs against the actual Bluefin base image (heavier
pull) tests the script as it behaves on the real base; run that nightly rather
than per-PR.
