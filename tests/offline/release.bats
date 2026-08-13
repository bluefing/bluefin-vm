#!/usr/bin/env bats
# release.sh: argument handling and the refusals that do not depend on repo
# state. The branch, cleanliness, sync, stamp and tag checks all read the
# working repo, so asserting them here would make the suite's result depend
# on whatever the developer happens to have checked out -- those are covered
# by the script refusing loudly when they fail, not by these tests.

setup() {
  cd "$BATS_TEST_DIRNAME/../.." || exit 1
}

@test "release.sh -h exits 0 and prints usage" {
  run ./bin/release.sh -h
  [ "$status" -eq 0 ]
  [[ "$output" == *"Usage:"* ]]
}

@test "release.sh rejects an unknown option" {
  run ./bin/release.sh -z
  [ "$status" -eq 1 ]
}

@test "release.sh requires a version" {
  run ./bin/release.sh
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage:"* ]]
}

@test "release.sh rejects anything that is not bare semver" {
  # Leading zeros included: semver forbids them, and 01.0.0 would tag a
  # version no Cargo.toml can declare.
  for bad in 1.2 v1.2.3 1.2.3.4 1.2.3-rc1 x.y.z 01.0.0 1.02.0 1.0.00; do
    run ./bin/release.sh "$bad"
    [ "$status" -eq 1 ]
    [[ "$output" == *"not a bare semver"* ]]
  done
}

@test "release.sh checks semver before touching the repo" {
  # The version is validated first, so a malformed one fails the same way
  # whatever branch or tree state the caller happens to be in.
  run ./bin/release.sh -n nonsense
  [[ "$output" == *"not a bare semver"* ]]
}
