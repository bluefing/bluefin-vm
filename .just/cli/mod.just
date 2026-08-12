import '../_config.just'
import '../_common.just'

# The `bluefin-vm` tool: downloads a disk, imports it into Tart, and runs it.
# Recipes wrap cargo so the crate is driven the same way as the rest of the
# repo. [no-cd] keeps them running from the repo root, then they cd into cli/.

# --- build & run ------------------------------------------------------------

# Build the CLI (debug)
[no-cd]
[group('build & run')]
build:
  cd cli && cargo build

# Run the CLI in debug, forwarding ARGS (e.g. `just cli run download --out s.zip`)
[no-cd]
[group('build & run')]
run *args:
  cd cli && cargo run -- {{args}}

# Debug `run` inflates the multi-GiB disk far too slowly, so real work (up,
# extract) goes through the optimised build.
[doc('Run the optimised binary, forwarding ARGS (e.g. `just cli run-release up`)')]
[no-cd]
[group('build & run')]
run-release *args:
  cd cli && cargo run --release -- {{args}}

# Wraps bin/package-cli.sh; the release workflow runs that same script, so a
# local package matches a released one.
[doc('Package the release binary into a Homebrew tarball (output/*.tar.gz + .sha256); forwards ARGS')]
[no-cd]
[group('build & run')]
package *args:
  ./bin/package-cli.sh {{args}}

# --- quality gate -----------------------------------------------------------

# Format the Rust code in place
[no-cd]
[group('quality gate')]
fmt:
  cd cli && cargo fmt

# Rust unit tests
[no-cd]
[group('quality gate')]
test:
  cd cli && cargo test --quiet

# The Rust gate -- format check, clippy, tests. Also run by the pre-commit
# hook, so this recipe defines what a clean crate means.
[doc('Rust gate: fmt check + clippy + tests (also run by pre-commit)')]
[no-cd]
[group('quality gate')]
check:
  cd cli && cargo fmt --check
  cd cli && cargo clippy --all-targets -- -D warnings
  cd cli && cargo test --quiet

# --- maintenance ------------------------------------------------------------

# Remove the Rust build artifacts (cli/target)
[no-cd]
[group('maintenance')]
clean:
  cd cli && cargo clean

# The cache holds one content-addressed disk per build (~/.cache/bluefin-vm/<sha>/),
# so it grows by ~21 GiB each time `up` fetches a new build. Path mirrors the
# CLI's default_work_dir ($BLUEFIN_VM_WORK_DIR overrides both) -- keep them in step.
[doc('Purge the up download cache (~/.cache/bluefin-vm); next `up` re-downloads')]
[no-cd]
[confirm("Delete every cached VM disk image under ~/.cache/bluefin-vm? The next `up` re-downloads (~2.9 GiB). Continue?")]
[group('maintenance')]
purge-cache:
  rm -rf "${BLUEFIN_VM_WORK_DIR:-$HOME/.cache/bluefin-vm}"
