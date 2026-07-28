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

# Run the test suite: bats (offline) + the cli's Rust unit tests
[group('test')]
test: cli::test
  bats tests

# Run all pre-commit hooks on all files (shellcheck, shfmt, yaml, tests, ...)
[group('test')]
lint:
  pre-commit run --all-files

# Remove build outputs (disk images and the cli's Rust artifacts)
[group('maintenance')]
clean: cli::clean
  rm -rf output
