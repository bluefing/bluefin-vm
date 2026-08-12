#!/usr/bin/env bash
# Guest-side check for display-scale provisioning. Delivered and run from the
# host by `just tart check-scale` (over ssh, as the provisioned user), or by
# hand in a guest terminal: bash ~/Shared/check-scale.sh
#
# Reports the apply-scale oneshot's unit state, its journal for this boot,
# the request and monitors.xml files, and the live display state, then gives
# a PASS/FAIL summary. Works for both postures: a profile with a scale (the
# unit ran and applied) and one without (refit on -- the unit was skipped).

# No -e: this is a report script -- a failing check is collected into the
# summary, not aborted on (the repo's action scripts keep -euo pipefail).
set -uo pipefail

unit=bluefin-vm-apply-scale.service
config="${XDG_CONFIG_HOME:-$HOME/.config}"
request="$config/bluefin-vm/scale-request"
monitors="$config/monitors.xml"
fail=0

section() { printf '\n== %s ==\n' "$1"; }

section "unit state"
systemctl --user show "$unit" \
  -p ActiveState,SubState,Result,ExecMainStatus | sed 's/^/  /'

section "journal (this boot)"
journalctl --user -u "$unit" -b --no-pager | sed 's/^/  /'

# Error-priority lines from the unit mean a rejected or refused request.
if journalctl --user -u "$unit" -b -p err -q --no-pager | grep -q .; then
  echo "  FAIL: the unit logged error-priority lines (see above)"
  fail=1
fi

section "request file"
if [[ -e $request ]]; then
  echo "  FAIL: $request still present ($(cat "$request")) -- not consumed"
  fail=1
else
  echo "  ok: no pending scale-request"
fi

section "monitors.xml"
if [[ -s $monitors ]]; then
  grep -E '<scale>|<width>|<height>|<rate>|<connector>' "$monitors" |
    sed 's/^ */  /'
else
  echo "  none -- expected when the profile sets no scale (refit on)"
fi

section "live display (gdctl)"
if command -v gdctl >/dev/null 2>&1; then
  gdctl show | sed 's/^/  /'
else
  echo "  gdctl not present; compare Settings -> Displays by hand"
fi

section "summary"
result=$(systemctl --user show "$unit" -p Result --value)
if [[ $fail -eq 0 && ($result == "success" || -z $result) ]]; then
  echo "  PASS"
else
  echo "  FAIL"
  exit 1
fi
