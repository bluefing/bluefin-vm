import '../_config.just'
import '../_common.just'

# rustup installs cargo under ~/.cargo/bin, which just's non-interactive recipe
# shells and the pre-commit hook don't inherit from the login profile. Put it
# on PATH so `cargo` is always found, whoever invokes the recipe.
export PATH := env('HOME') / '.cargo/bin' + ':' + env('PATH')

# The `bluefin-vm` tool: downloads a seed, imports it into Tart, and runs it.
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

# Debug `run` inflates the multi-GiB seed far too slowly, so real work (up,
# extract) goes through the optimised build.
[doc('Run the optimised binary, forwarding ARGS (e.g. `just cli run-release up`)')]
[no-cd]
[group('build & run')]
run-release *args:
  cd cli && cargo run --release -- {{args}}

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
# hook, so this recipe is the single source of truth for "is the crate clean".
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
