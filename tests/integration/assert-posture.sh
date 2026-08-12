#!/usr/bin/env bash
# Assert a provisioned account's posture. Shared by the Tier 1 container tests
# and (later) the Tier 2 in-VM smoke checks, so both verify the same contract.
# Runs as root -- it reads /etc/shadow and /etc/sudoers.d. Driven by env:
#   POSTURE_USER              account to check (required)
#   EXPECT_KEY 0|1            authorized_keys installed (default 1)
#   EXPECT_PASSWORDLESS_SUDO 0|1   (default 0)
#   EXPECT_SSH_PASSWORD 0|1        (default 1)
#   EXPECT_SCALE              expected scale-request content (default: none staged)
#   BLUEFIN_VM_PDIR           if set, assert the share sub-dir was cleared
set -euo pipefail

user="${POSTURE_USER:?POSTURE_USER is required}"
fails=0
pass() { echo "ok: $1"; }
fail() {
  echo "FAIL: $1" >&2
  fails=$((fails + 1))
}

if id "$user" >/dev/null 2>&1; then pass "user '$user' exists"; else fail "user '$user' missing"; fi
if id -nG "$user" 2>/dev/null | tr ' ' '\n' | grep -qx wheel; then
  pass "user in wheel"
else
  fail "user not in wheel"
fi

# A usable login password is set (password == username is provision.sh's job;
# here we assert a usable password exists -- `passwd -S` status starts with `P`,
# i.e. `P`/`PS`, as opposed to `L` (locked) or `NP` (none)).
pw_status="$(passwd -S "$user" 2>/dev/null | awk '{print $2}')"
if [[ "$pw_status" == P* ]]; then
  pass "login password set"
else
  fail "no usable login password (passwd -S: '${pw_status:-none}')"
fi

home="$(getent passwd "$user" | cut -d: -f6)"
ak="$home/.ssh/authorized_keys"
if [[ "${EXPECT_KEY:-1}" == 1 ]]; then
  if [[ -f "$ak" ]]; then pass "authorized_keys present"; else fail "authorized_keys missing"; fi
  if [[ -f "$ak" && "$(stat -c '%a' "$ak")" == 600 ]]; then
    pass "authorized_keys mode 600"
  else
    fail "authorized_keys wrong mode"
  fi
  if [[ -f "$ak" && "$(stat -c '%U' "$ak")" == "$user" ]]; then
    pass "authorized_keys owned by $user"
  else
    fail "authorized_keys wrong owner"
  fi
else
  if [[ ! -e "$ak" ]]; then pass "no authorized_keys"; else fail "unexpected authorized_keys"; fi
fi

sudoers="/etc/sudoers.d/bluefin-vm-$user"
if [[ "${EXPECT_PASSWORDLESS_SUDO:-0}" == 1 ]]; then
  if [[ -f "$sudoers" ]] && grep -q 'NOPASSWD:ALL' "$sudoers"; then
    pass "passwordless-sudo drop-in present"
  else
    fail "passwordless-sudo drop-in missing/wrong"
  fi
else
  if [[ ! -e "$sudoers" ]]; then
    pass "no passwordless-sudo drop-in"
  else
    fail "unexpected passwordless-sudo drop-in"
  fi
fi

sshd_drop="/etc/ssh/sshd_config.d/00-bluefin-vm-nopassword.conf"
if [[ "${EXPECT_SSH_PASSWORD:-1}" == 1 ]]; then
  if [[ ! -e "$sshd_drop" ]]; then
    pass "ssh password auth on (no drop-in)"
  else
    fail "unexpected ssh-password drop-in"
  fi
else
  if [[ -f "$sshd_drop" ]] && grep -qi '^PasswordAuthentication no' "$sshd_drop"; then
    pass "ssh password disabled"
  else
    fail "ssh-password drop-in missing/wrong"
  fi
fi

# The scale hand-off: provisioning stages the requested percentage into the
# account's config for the first-login oneshot; applying it needs a session,
# so only the staging is assertable here.
req="$home/.config/bluefin-vm/scale-request"
if [[ -n "${EXPECT_SCALE:-}" ]]; then
  if [[ -f "$req" && "$(cat "$req")" == "$EXPECT_SCALE" ]]; then
    pass "scale-request staged ($EXPECT_SCALE)"
  else
    fail "scale-request missing or wrong content"
  fi
  if [[ -f "$req" && "$(stat -c '%a' "$req")" == 644 ]]; then
    pass "scale-request mode 644"
  else
    fail "scale-request wrong mode"
  fi
  if [[ -f "$req" && "$(stat -c '%U' "$req")" == "$user" ]]; then
    pass "scale-request owned by $user"
  else
    fail "scale-request wrong owner"
  fi
else
  if [[ ! -e "$req" ]]; then pass "no scale-request"; else fail "unexpected scale-request"; fi
fi

if [[ -n "${BLUEFIN_VM_PDIR:-}" ]]; then
  if [[ ! -e "$BLUEFIN_VM_PDIR" ]]; then pass "share cleared"; else fail "share not cleared"; fi
fi

if ((fails > 0)); then
  echo "posture: $fails failure(s) for '$user'" >&2
  exit 1
fi
echo "posture: OK for '$user'"
