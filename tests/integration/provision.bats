#!/usr/bin/env bats
# Tier 1 integration: run image/provision.sh in a container against a synthetic
# share and assert the resulting account posture across the credential matrix.
# The whole point is fast, VM-free coverage of config variations -- see
# docs/internal/design/testing-strategy.md. Needs docker; skipped without it.

IMAGE="bluefin-vm-provtest"

setup_file() {
  command -v docker >/dev/null 2>&1 || return 0
  docker info >/dev/null 2>&1 || return 0
  # Build the tiny test image once: fedora plus what provision.sh touches --
  # the account tools (useradd/chpasswd from shadow-utils, passwd -S) and
  # openssh-server, so /etc/ssh/sshd_config.d exists as it does on the real
  # Bluefin base. Inline so there's no Dockerfile in the tree for hadolint.
  docker build -q -t "$IMAGE" - >/dev/null <<'DOCKERFILE'
FROM fedora:latest
RUN dnf -y install shadow-utils passwd openssh-server && dnf clean all
DOCKERFILE
}

setup() {
  command -v docker >/dev/null 2>&1 || skip "docker not installed"
  docker info >/dev/null 2>&1 || skip "docker daemon not running"
  repo="$BATS_TEST_DIRNAME/../.."
}

# Run provision.sh + the posture assertions in a fresh container. Extra args
# (CASE_* via -e) are passed through to docker run.
run_case() {
  docker run --rm "$@" \
    -v "$repo/image/provision.sh:/provision.sh:ro" \
    -v "$BATS_TEST_DIRNAME/run-case.sh:/run-case.sh:ro" \
    -v "$BATS_TEST_DIRNAME/assert-posture.sh:/assert-posture.sh:ro" \
    "$IMAGE" bash /run-case.sh
}

@test "default posture: password set, sudo prompts, ssh password on, key installed" {
  run run_case -e CASE_USER=usera
  [ "$status" -eq 0 ]
  [[ "$output" == *"posture: OK"* ]]
}

@test "passwordless sudo writes the sudoers drop-in" {
  run run_case -e CASE_USER=usera -e CASE_PASSWORDLESS_SUDO=1
  [ "$status" -eq 0 ]
}

@test "ssh password off writes the sshd drop-in" {
  run run_case -e CASE_USER=usera -e CASE_SSH_PASSWORD=0
  [ "$status" -eq 0 ]
}

@test "both hardened: sudoers and sshd drop-ins present together" {
  run run_case -e CASE_USER=usera -e CASE_PASSWORDLESS_SUDO=1 -e CASE_SSH_PASSWORD=0
  [ "$status" -eq 0 ]
}

@test "no ssh key: account created, no authorized_keys" {
  run run_case -e CASE_USER=usera -e CASE_KEY=0
  [ "$status" -eq 0 ]
}

@test "invalid username: provision.sh refuses before creating anything" {
  run run_case -e CASE_USER='Bad!'
  [ "$status" -ne 0 ]
  [[ "$output" == *"invalid username"* ]]
}
