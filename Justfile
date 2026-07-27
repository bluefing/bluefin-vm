import '.just/_config.just'
import '.just/_common.just'

# Run and manage the Bluefin VM (Apple VF, CLI-first) -- the front door
[group('tart')]
mod tart '.just/tart'

# Build images and disks (the plumbing tart reaches for)
[group('image-build')]
mod build '.just/build'

# Run the bats test suite (no container builds)
[group('test')]
test:
  bats tests

# Run all pre-commit hooks on all files (shellcheck, shfmt, yaml, tests, ...)
[group('test')]
lint:
  pre-commit run --all-files

# Remove build outputs
[group('maintenance')]
clean:
  rm -rf output
