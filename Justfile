import '.just/_config.just'
import '.just/_common.just'

# Run and manage the Bluefin VM (Apple VF, CLI-first) -- the front door
[group('tart')]
mod tart '.just/tart'

# Build images and disks (the plumbing tart reaches for)
[group('image-build')]
mod build '.just/build'

# The `bluefin-vm` downloader/runner tool (Rust)
[group('cli')]
mod cli '.just/cli'

# Run the test suite: bats (offline) + the cli's Rust unit tests
[group('test')]
test:
  bats tests
  just cli test

# Run all pre-commit hooks on all files (shellcheck, shfmt, yaml, tests, ...)
[group('test')]
lint:
  pre-commit run --all-files

# Remove build outputs
[group('maintenance')]
clean:
  rm -rf output
