#!/usr/bin/env bats
# build-image.sh: argument handling and dry-run command construction.

setup() {
  cd "$BATS_TEST_DIRNAME/.." || exit 1
}

@test "build-image.sh -h exits 0 and prints usage" {
  run ./bin/build-image.sh -h
  [ "$status" -eq 0 ]
  [[ "$output" == *"Usage:"* ]]
}

@test "build-image.sh requires a tag (-t)" {
  run ./bin/build-image.sh -n
  [ "$status" -ne 0 ]
}

@test "build-image.sh rejects an unknown option" {
  run ./bin/build-image.sh -z
  [ "$status" -ne 0 ]
}

@test "dry-run builds the image context with the given tag" {
  run ./bin/build-image.sh -n -t localhost/x:y
  [ "$status" -eq 0 ]
  [[ "$output" == *"build"* ]]
  [[ "$output" == *"-t localhost/x:y"* ]]
}

@test "dry-run passes the base override as a build arg" {
  run ./bin/build-image.sh -n -t localhost/x:y -b ghcr.io/example/base:tag
  [ "$status" -eq 0 ]
  [[ "$output" == *"--build-arg BASE=ghcr.io/example/base:tag"* ]]
}
