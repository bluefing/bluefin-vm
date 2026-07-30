#!/usr/bin/env bats
# Justfile: recipe wiring across the modular structure. Regression guard for the
# -f flag (a bare positional arg is silently ignored by build-disk.sh's getopts, so
# `build iso` must pass `-f iso`), for the image default living in the porcelain,
# and for the tart verbs (up/start/stop) sharing one arg meaning.

setup() {
  cd "$BATS_TEST_DIRNAME/.." || exit 1
  command -v just >/dev/null 2>&1 || skip "just not installed"
}

@test "just build iso passes -f iso to build-disk.sh" {
  run just --dry-run build iso
  [[ "$output" == *"-f iso"* ]]
}

@test "just build qcow2 passes -f qcow2 to build-disk.sh" {
  run just --dry-run build qcow2
  [[ "$output" == *"-f qcow2"* ]]
}

@test "just build raw passes -f raw to build-disk.sh" {
  run just --dry-run build raw
  [[ "$output" == *"-f raw"* ]]
}

@test "just build qcow2 supplies the default image (the porcelain holds it)" {
  run just --dry-run build qcow2
  [[ "$output" == *"-i "* ]]
  # Read default_image from the config rather than hardcoding a tag here, so the
  # test can't drift from it: the porcelain should pass whatever it's set to.
  [[ "$output" == *"$(just --evaluate default_image)"* ]]
}

@test "just build qcow2 -i overrides the default image" {
  run just --dry-run build qcow2 -i custom/img:tag
  [[ "$output" == *"custom/img:tag"* ]]
}

@test "just tart up wires build raw -> import -> start, each step guarded" {
  run just --dry-run tart up
  [[ "$output" == *"-f raw"* ]]
  [[ "$output" == *"bin/create-vm.sh"* ]]
  [[ "$output" == *"tart run"* ]]
  # Incremental: build only if the disk is absent, re-import only if newer.
  [[ "$output" == *'if [ -f "$disk" ]'* ]]
  [[ "$output" == *"-nt"* ]]
  # Detached: tart run must be backgrounded with its output captured.
  [[ "$output" == *"nohup tart run"* ]]
  # Durable tier: the share must be passed on every run (BL-13).
  [[ "$output" == *'--dir="bluefin-share:'* ]]
}

@test "just tart start passes the durable share" {
  run just --dry-run tart start
  [[ "$output" == *'--dir="bluefin-share:'* ]]
}

@test "just tart import passes the canonical raw disk (never auto-detect)" {
  run just --dry-run tart import
  [[ "$output" == *"bin/create-vm.sh -d output/image/disk.raw"* ]]
}

@test "just tart ssh targets the VM's IP with the test login" {
  run just --dry-run tart ssh
  [[ "$output" == *"tart ip"* ]]
  [[ "$output" == *"bluefin"* ]]
}

@test "just tart smoke delivers the script, runs it, and asserts the round-trip" {
  run just --dry-run tart smoke
  [[ "$output" == *"tests/smoke/guest-checks.sh"* ]]
  [[ "$output" == *"tart ip"* ]]
  [[ "$output" == *"bash ~/Shared/guest-checks.sh"* ]]
  # host generates the run id and asserts that exact file came back
  [[ "$output" == *"run_id="* ]]
  [[ "$output" == *'guest-checks.log"'* ]]
}

@test "just tart up-patched chains container build -> disk -> import -> up" {
  run just --dry-run tart up-patched
  [[ "$output" == *"bin/build-image.sh"* ]]
  [[ "$output" == *'-i "localhost/bluefin-vm-patched'* ]]
  [[ "$output" == *"bin/create-vm.sh"* ]]
}

@test "just build image tags the patched ref and tracks the default base" {
  run just --dry-run build image
  [[ "$output" == *"bin/build-image.sh"* ]]
  [[ "$output" == *'-t "localhost/bluefin-vm-patched'* ]]
  [[ "$output" == *"-b "* ]]
}
