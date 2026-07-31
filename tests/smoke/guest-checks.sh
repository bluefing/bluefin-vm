#!/usr/bin/env bash
# Guest-side smoke test for a booted Bluefin VM. Run it INSIDE the VM
# (a GUI terminal, or over `just tart ssh`). It writes a result log back to
# the host through the shared folder -- which doubles as the share's own
# round-trip proof: if the host sees the log, the share works.
#
# Arg 1 is an optional run label used in the log filename. `just tart smoke`
# passes a host-generated timestamp so the host knows the exact file to expect
# and can assert the round-trip; run by hand, it self-timestamps.
#
# Delivery: the script reaches the guest via the shared folder itself
# (host ~/bluefin-share -> guest ~/Shared), so:  bash ~/Shared/guest-checks.sh
set -uo pipefail

share="$HOME/Shared"
stamp="${1:-$(date +%Y%m%d-%H%M%S)}"
if [ -d "$share" ] && [ -w "$share" ]; then
  log="$share/${stamp}-guest-checks.log"
  share_ok=1
else
  log="$HOME/${stamp}-guest-checks.log"
  share_ok=0
fi

pass=0
fail=0
say() { echo "$*" | tee -a "$log"; }
check() { # check "label" cmd...
  local label="$1"
  shift
  if "$@" >/dev/null 2>&1; then
    say "PASS  $label"
    pass=$((pass + 1))
  else
    say "FAIL  $label"
    fail=$((fail + 1))
  fi
}
report() { say "----  $1: $2"; } # informational, not pass/fail

say "Bluefin VM guest checks -- $stamp -- $(uname -n)"
say "================================================"

# Arch + desktop versions (report; the arch one is also a hard check)
report "arch" "$(uname -m)"
check "arch is aarch64" test "$(uname -m)" = aarch64
report "gnome-shell" "$(rpm -q gnome-shell 2>/dev/null || echo '?')"
report "mutter" "$(rpm -q mutter 2>/dev/null || echo '?')"

# Baked guest config (the patched-image payload)
check "sshd enabled+active" systemctl is-active --quiet sshd
check "vdagent wired into graphical-session" \
  test -e /usr/lib/systemd/user/graphical-session.target.wants/spice-vdagent.service
report "vdagent running" \
  "$(systemctl --user is-active spice-vdagent 2>/dev/null || echo 'inactive (needs a GUI session)')"
check "virtiofs share mounted" mountpoint -q /var/mnt/shared
report "share source" "$(findmnt -no SOURCE,FSTYPE /var/mnt/shared 2>/dev/null || echo 'not mounted')"
check "Shared symlink resolves to a dir" test -d "$HOME/Shared"

# Workload sanity
check "podman present" command -v podman
check "distrobox present" command -v distrobox
report "system state" "$(systemctl is-system-running 2>/dev/null || echo '?')"

say "================================================"
say "RESULT: $pass passed, $fail failed"
if [ "$share_ok" = 1 ]; then
  say "Log written to the share: $log"
  say "(host sees it at ~/bluefin-share/${stamp}-guest-checks.log)"
else
  say "SHARE TEST FAILED: ~/Shared not writable -- log kept at $log"
fi

[ "$fail" -eq 0 ]
