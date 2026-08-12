#!/usr/bin/env bash
# Tier 1 driver, run inside the test container: build a synthetic share from
# CASE_* env, run provision.sh against it, then assert the resulting posture
# with the shared assert-posture.sh. Both scripts are bind-mounted at /.
#   CASE_USER               account name (default usera)
#   CASE_KEY 0|1            install an ssh key (default 1)
#   CASE_PASSWORDLESS_SUDO 0|1   write the passwordless-sudo flag (default 0)
#   CASE_SSH_PASSWORD 0|1        1=leave on, 0=write disable-ssh-password (default 1)
#   CASE_SCALE              scale percentage to request (default: none)
set -euo pipefail

user="${CASE_USER:-usera}"
export BLUEFIN_VM_PDIR="/tmp/share/.bluefin-vm"

rm -rf /tmp/share
mkdir -p "$BLUEFIN_VM_PDIR"
printf '%s\n' "$user" >"$BLUEFIN_VM_PDIR/username"

if [[ "${CASE_KEY:-1}" == 1 ]]; then
  printf 'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAtest %s@test\n' "$user" \
    >"$BLUEFIN_VM_PDIR/authorized_keys"
else
  : >"$BLUEFIN_VM_PDIR/authorized_keys" # empty -> provision.sh installs no key
fi

if [[ "${CASE_PASSWORDLESS_SUDO:-0}" == 1 ]]; then
  : >"$BLUEFIN_VM_PDIR/passwordless-sudo"
fi
if [[ "${CASE_SSH_PASSWORD:-1}" == 0 ]]; then
  : >"$BLUEFIN_VM_PDIR/disable-ssh-password"
fi
if [[ -n "${CASE_SCALE:-}" ]]; then
  printf '%s\n' "$CASE_SCALE" >"$BLUEFIN_VM_PDIR/scale"
fi

/provision.sh # reads BLUEFIN_VM_PDIR

POSTURE_USER="$user" \
  EXPECT_KEY="${CASE_KEY:-1}" \
  EXPECT_PASSWORDLESS_SUDO="${CASE_PASSWORDLESS_SUDO:-0}" \
  EXPECT_SSH_PASSWORD="${CASE_SSH_PASSWORD:-1}" \
  EXPECT_SCALE="${CASE_SCALE:-}" \
  /assert-posture.sh
