# bluefin-vm — the map

Turn the upstream Bluefin bootc container into a running Linux VM on Apple
Silicon. `just` lists the top-level verbs and the module groups; this page covers
where things live, how they fit, and what the root verbs do. Each file documents
itself in its own header, so for a file's scope and rationale, read the file.

## The layout

The organising rule: **plumbing is scripts, porcelain is recipes.** The scripts
in `bin/` do the work; the `just` recipes are a thin, memorable interface over
them. Reach into a module for its own context with `just <module> help`.

- **`bin/`** (run from repo root) — `build-disk.sh` (image → bootable disk via
  bootc-image-builder, the same entrypoint locally and in CI), `build-image.sh`
  (build `image/Containerfile` into the store as a `localhost/` ref),
  `create-vm.sh` (import a disk into Tart), `package-cli.sh` (the tool's release
  tarball).
- **`config.toml`, `image/`** — the disk/image inputs. `config.toml` holds
  disk-build concerns only (root size, the test login); `image/Containerfile`
  layers the OS-side guest fixes it can't express; `image/provision.sh` and
  `harden.sh` are the guest scripts baked in (first-boot account creation, the
  opt-in lock-down).
- **`Justfile`, `.just/`** — the porcelain. Modules **build**, **tart**, and
  **cli** hold the recipes; `_config.just` carries shared defaults (including
  `default_image`), `_common.just` the shared helpers (including `help`).
- **`cli/`** — the `bluefin-vm` Rust binary a user installs. `src/core/` is
  UI-agnostic so a future TUI drives the same operations.
- **`tests/`** — the offline bats suite plus `tests/smoke/guest-checks.sh`, the
  in-VM acceptance check.
- **`.github/workflows/`** — CI: the ARM64 disk build, and the release that
  packages the tool on a `v*` tag. The Homebrew formula lives in the tap repo
  (`bluefing/homebrew-tap`), not here.

## Root verbs

- **`setup`** installs the git hooks (pre-commit, commit-msg, pre-push) — run it
  once after cloning. It needs `pre-commit` on the system.
- **`test`** is the fast inner loop: the bats suite plus the crate's Rust unit
  tests, all offline — arg handling, dry-run output, and recipe wiring, no
  container builds or network.
- **`lint`** runs every pre-commit hook over all files (shellcheck, shfmt,
  hadolint, the bats suite, the Rust gate, justfile validation). The same hook
  gates every commit.
- **`clean`** removes build outputs; **`really-clean`** additionally reclaims
  the Docker/Colima space a build consumes (the `bootc-store` volume, cached
  source and builder images, unused build cache), all re-pulled on the next
  build.

`just test` and `just lint` are the offline checks. A built disk's boot isn't
covered by them — confirm that in a VM with `just tart smoke`.
