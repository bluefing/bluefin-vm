#!/usr/bin/env bats
# package-cli.sh: argument handling and dry-run command construction. Dry runs
# need no cargo and no arm64 host, so these run everywhere (CI is x86 Linux).

setup() {
  cd "$BATS_TEST_DIRNAME/../.." || exit 1
}

@test "package-cli.sh -h exits 0 and prints usage" {
  run ./bin/package-cli.sh -h
  [ "$status" -eq 0 ]
  [[ "$output" == *"Usage:"* ]]
}

@test "package-cli.sh rejects an unknown option" {
  run ./bin/package-cli.sh -z
  [ "$status" -ne 0 ]
}

@test "package-cli.sh errors when -v is given no argument" {
  run ./bin/package-cli.sh -v
  [ "$status" -ne 0 ]
  [[ "$output" == *"requires an argument"* ]]
}

@test "dry-run builds, tars a bare binary, and writes a sha256 sidecar" {
  run ./bin/package-cli.sh -n
  [ "$status" -eq 0 ]
  [[ "$output" == *"cargo build --release --manifest-path cli/Cargo.toml"* ]]
  # -C into the binary dir so the archive holds a bare `bluefin-vm`.
  [[ "$output" == *"tar -czf output/bluefin-vm-"*"-aarch64-apple-darwin.tar.gz -C cli/target/release bluefin-vm"* ]]
  [[ "$output" == *"shasum -a 256"* ]]
}

@test "dry-run defaults the version to the crate version" {
  crate=$(sed -n 's/^version = "\(.*\)"/\1/p' cli/Cargo.toml | head -n1)
  run ./bin/package-cli.sh -n
  [ "$status" -eq 0 ]
  [[ "$output" == *"bluefin-vm-${crate}-aarch64-apple-darwin.tar.gz"* ]]
}

@test "dry-run honours -v and -o in the asset path" {
  run ./bin/package-cli.sh -n -v 9.9.9 -o dist
  [ "$status" -eq 0 ]
  [[ "$output" == *"dist/bluefin-vm-9.9.9-aarch64-apple-darwin.tar.gz"* ]]
}
