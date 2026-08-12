#!/usr/bin/env bats
# config.toml: the disk-sizing customization that fixes the ostree
# "min-free-space-percent" build failure on Bluefin's large desktop image.

setup() {
  cd "$BATS_TEST_DIRNAME/../.." || exit 1
}

@test "config.toml sizes the root filesystem" {
  run grep -F 'mountpoint = "/"' config.toml
  [ "$status" -eq 0 ]
}

@test "config.toml sets a minsize" {
  run grep -F 'minsize' config.toml
  [ "$status" -eq 0 ]
}

# TOML well-formedness is checked by the check-toml pre-commit hook.
