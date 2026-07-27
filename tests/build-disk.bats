#!/usr/bin/env bats
# build-disk.sh: argument handling and dry-run command construction.

setup() {
  cd "$BATS_TEST_DIRNAME/.." || exit 1
}

@test "build-disk.sh -h exits 0 and prints usage" {
  run ./bin/build-disk.sh -h
  [ "$status" -eq 0 ]
  [[ "$output" == *"Usage:"* ]]
}

@test "build-disk.sh rejects an unsupported format" {
  run ./bin/build-disk.sh -f bogus -n
  [ "$status" -ne 0 ]
}

@test "build-disk.sh rejects an unknown option" {
  run ./bin/build-disk.sh -z
  [ "$status" -ne 0 ]
}

@test "build-disk.sh -f requires an argument" {
  run ./bin/build-disk.sh -f
  [ "$status" -ne 0 ]
}

@test "build-disk.sh requires an image (-i) -- no default in the plumbing" {
  run ./bin/build-disk.sh -n -f qcow2
  [ "$status" -ne 0 ]
}

@test "dry-run qcow2 builds a --type qcow2 command for the given image" {
  run ./bin/build-disk.sh -n -f qcow2 -i ghcr.io/example/img:tag
  [ "$status" -eq 0 ]
  [[ "$output" == *"--type qcow2"* ]]
  [[ "$output" == *"ghcr.io/example/img:tag"* ]]
  [[ "$output" == *"--platform linux/arm64"* ]]
  [[ "$output" == *"/config.toml:ro"* ]]
}

@test "dry-run passes a custom image and iso type through" {
  run ./bin/build-disk.sh -n -f iso -i ghcr.io/example/dakota:lts-arm64
  [ "$status" -eq 0 ]
  [[ "$output" == *"--type iso"* ]]
  [[ "$output" == *"dakota:lts-arm64"* ]]
}

@test "dry-run skips the pull step for store-only localhost/ images" {
  run ./bin/build-disk.sh -n -f raw -i localhost/derived:latest
  [ "$status" -eq 0 ]
  [[ "$output" != *"pull"* ]]
  [[ "$output" == *"--type raw"* ]]
}
