import '.just/_config.just'
import '.just/_common.just'

# The `bluefin-vm` tool (Rust) -- the front door: download, import, run (`up`)
[group('cli')]
mod cli '.just/cli'

# Run & manage an already-imported VM (Apple VF, CLI-first) -- dev/runtime
[group('tart')]
mod tart '.just/tart'

# Build images and disks (the plumbing tart reaches for)
[group('image-build')]
mod build '.just/build'

# Build and serve the documentation site (Zensical)
[group('docs')]
mod docs '.just/docs'

# The pinned project tooling from mise.toml (just, bats, pre-commit, uv, ...).
# A no-op once installed.
[doc('Install the pinned project tooling (mise.toml)')]
[group('setup')]
tools:
  mise install

# First-time setup after cloning: the pinned tooling, then the git hooks
# (pre-commit, commit-msg, pre-push). Needs just itself to run -- `mise install`
# provides it for a shell that only has mise.
[doc('First-time setup: pinned tooling + git hooks -- run once after cloning')]
[group('setup')]
setup: tools
  pre-commit install

# Tier 0: Rust unit tests, offline bats contracts, guest-script pytest -- fast, no deps
[group('test')]
test: cli::test
  bats tests/offline
  pre-commit run pytest --all-files

# Tier 1: run provision.sh in a container across the config matrix (needs docker)
[group('test')]
test-integration:
  bats tests/integration

# Tier 2: the guest smoke test in a running VM (run `just tart up` first)
[group('test')]
[arg('user', long, short)]
[arg('name', long, short)]
test-e2e name="Bluefin" user="$USER": (tart::smoke name user) (tart::check-scale name user)

# Run all pre-commit hooks on all files (shellcheck, shfmt, yaml, tests, ...)
[group('test')]
lint:
  pre-commit run --all-files

# Remove build outputs (disk images and the cli's Rust artifacts)
[confirm("This will delete generated output. Continue?")]
[group('maintenance')]
clean: cli::clean
  rm -rf output

# Reclaim the Docker/Colima space builds consume, on top of `clean`: the
# bootc-store cache volume, cached source and builder images, and unused build
# cache. All of it is re-pulled or rebuilt on the next `just build`.
[doc('Reclaim Docker/Colima build space (bootc-store, cached images, build cache)')]
[confirm("Remove the bootc-store volume, cached Bluefin/builder images, and unused Docker build cache. Continue?")]
[group('maintenance')]
[script]
really-clean: clean
  # -f / || true so a missing or in-use item is skipped, not fatal.
  docker volume rm -f bootc-store 2>/dev/null || true
  # Match by repo, not pinned tags, so it clears whatever source images are cached.
  imgs=$(docker images -q --filter=reference='ghcr.io/projectbluefin/*')
  [ -n "$imgs" ] && docker rmi -f $imgs 2>/dev/null || true
  docker rmi -f quay.io/centos-bootc/bootc-image-builder:latest 2>/dev/null || true
  docker builder prune -f
  docker system df
