#!/usr/bin/env bats
# config.toml: the disk-sizing customization that fixes the ostree
# "min-free-space-percent" build failure on Bluefin's large desktop image.

setup() {
  cd "$BATS_TEST_DIRNAME/.." || exit 1
}

@test "config.toml sizes the root filesystem" {
  run grep -F 'mountpoint = "/"' config.toml
  [ "$status" -eq 0 ]
}

@test "config.toml sets a minsize" {
  run grep -F 'minsize' config.toml
  [ "$status" -eq 0 ]
}

@test "config.toml parses as valid TOML" {
  command -v python3 >/dev/null 2>&1 || skip "no python3"
  python3 -c 'import tomllib' 2>/dev/null || skip "no tomllib"
  run python3 -c 'import tomllib; tomllib.load(open("config.toml", "rb"))'
  [ "$status" -eq 0 ]
}
