#!/usr/bin/env bash
# First-boot provisioning for a downloaded (generic) disk. The host writes the
# account details into the durable share before boot; this oneshot creates that
# account so a shipped disk feels personal, without a greeter. With no details
# present the service is condition-skipped and the baked test login stays the
# way in.
#
# Credential model: your ssh public key always goes in (the way into a terminal).
# The rest depends on the autologin choice:
#
# - Autologin on (default) -> a password-less account: reachable only via
#   autologin at the desktop, administering via a scoped passwordless-sudo rule,
#   idle lock disabled. No secret crosses the share -- the disposable-dev posture.
# - Autologin off -> a normal account with password == username. Not about
#   keeping attackers out (they gain nothing over a passwordless sudoer) but a
#   deliberate `sudo` prompt: a guard so a fat-fingered or pasted command can't
#   silently run as root. No secret in the share -- the value is the username,
#   already there.
#
# Either way no private key or real secret lives in the VM or the share, which
# this script clears after first boot.

set -euo pipefail

pdir=/var/mnt/shared/bluefin-share/.bluefin-vm
user=$(<"$pdir/username")

# Validate before creating anything: a malformed name would otherwise land in a
# sudoers.d file and break sudo for the whole VM.
if [[ ! $user =~ ^[a-z_][a-z0-9_-]{0,31}$ ]]; then
  echo "bluefin-vm-provision: invalid username '$user'" >&2
  exit 1
fi

# wheel = admin; /etc/skel gives the home its ~/Shared symlink (created by
# image/Containerfile).
id "$user" &>/dev/null || useradd --create-home --groups wheel "$user"
home=$(getent passwd "$user" | cut -d: -f6)

# Public key(s) for ssh -- always installed, whatever the password posture.
if [[ -s $pdir/authorized_keys ]]; then
  install -d -m 700 -o "$user" -g "$user" "$home/.ssh"
  install -m 600 -o "$user" -g "$user" \
    "$pdir/authorized_keys" "$home/.ssh/authorized_keys"
  # Label as ssh_home_t where SELinux is active (the VM); a no-op elsewhere.
  # The guard's own failure is exempt from set -e; restorecon's is not.
  [[ -f /sys/fs/selinux/enforce ]] && restorecon -R "$home/.ssh"
fi

# Credential posture keys off the autologin flag the host writes:
#
# - Autologin on -> a password-less account. A greeter has no password to take,
#   so autologin is the only way to the desktop; a scoped passwordless-sudo rule
#   is the only way to administer; and the idle lock is disabled so it can't trap
#   the session. No secret ever crosses the share.
# - Autologin off -> a normal account with password == username. The point isn't
#   security (an attacker gains nothing over a passwordless sudoer) -- it's a
#   deliberate `sudo` prompt, a guard against a fat-fingered or pasted command
#   running as root unasked. No secret in the share: the value is the username,
#   already here. The greeter is usable, sudo takes that password (so no
#   passwordless rule), and the lock screen works.
if [[ -e $pdir/autologin ]]; then
  sudoers="/etc/sudoers.d/bluefin-vm-$user"
  printf '%s ALL=(ALL) NOPASSWD:ALL\n' "$user" >"$sudoers"
  chmod 440 "$sudoers"

  # No greeter -- bluefin-vm-gdm-autologin edits custom.conf in place so any
  # settings the base image ships survive.
  /usr/libexec/bluefin-vm-gdm-autologin "$user"

  # A password-less account can't clear the lock screen either, so an idle lock
  # would trap the autologin desktop until reboot. Disable it with a dconf system
  # default (/etc is writable at runtime; /usr is not) -- the screen may still
  # blank, it just won't lock.
  if [[ ! -f /etc/dconf/profile/user ]]; then
    printf 'user-db:user\nsystem-db:local\n' >/etc/dconf/profile/user
  elif ! grep -q '^system-db:local' /etc/dconf/profile/user; then
    echo 'system-db:local' >>/etc/dconf/profile/user
  fi
  mkdir -p /etc/dconf/db/local.d
  printf '[org/gnome/desktop/screensaver]\nlock-enabled=false\n' \
    >/etc/dconf/db/local.d/00-bluefin-vm-nolock
  dconf update
else
  # Password == username: a usable greeter login and a real `sudo` prompt, so an
  # accidental root command is caught. No secret added -- the value is the
  # username, already in the share.
  echo "$user:$user" | chpasswd
fi

# Guest desktop scale, pre-seeded into monitors.xml before the session starts so
# the desktop comes up already scaled -- gnome-shell only shows its "keep these
# settings?" confirm-or-revert dialog for a *live* reconfiguration (gdctl, or
# Settings), never for a session reading its own config at login. The host
# writes the scale file only when the profile has refit off (a fixed
# resolution); with refit on the guest mode follows the window, so there's no
# stable mode to pin a scale to.
#
# mutter only honours a <logicalmonitor> whose <mode> exactly matches a real
# mode of the connector -- width, height, AND refresh rate (verified: a mode
# with the wrong rate, or no rate, is silently discarded and scale stays 1x).
# The rate isn't in /sys/class/drm/*/modes (width x height only), so read the
# active mode's timing from DRM debugfs and compute vrefresh = clock (kHz) /
# (htotal * vtotal). The guest's virtio-gpu output is always connector
# "Virtual-1" with no EDID. scale is a percentage (150, 200); GNOME wants a
# factor (1.5, 2).
if [[ -s $pdir/scale ]]; then
  pct=$(<"$pdir/scale")
  factor=$(awk "BEGIN { printf \"%s\", $pct / 100 }")

  # Parse the first `mode: "WxH": vref clock hdisp hss hse htotal vdisp ... vtotal`
  # line: $2 is the WxH name, $3 the space-separated timing (f[3]=clock kHz,
  # f[7]=htotal, f[11]=vtotal). Runs as root -- debugfs is root-only.
  # `|| true`: read returns non-zero at EOF (no mode found -> empty input), which
  # set -e would treat as fatal; the guard below handles the empty case instead.
  read -r width height rate < <(
    awk -F'"' '/mode: "/ {
      split($2, wh, "x")
      split($3, f, " ")
      printf "%s %s %.3f\n", wh[1], wh[2], f[3] * 1000 / (f[7] * f[11])
      exit
    }' /sys/kernel/debug/dri/*/state 2>/dev/null
  ) || true

  if [[ -n $width && -n $height && -n $rate ]]; then
    conf="$home/.config"
    install -d -m 700 -o "$user" -g "$user" "$conf"
    cat >"$conf/monitors.xml" <<EOF
<monitors version="2">
  <configuration>
    <layoutmode>logical</layoutmode>
    <logicalmonitor>
      <x>0</x>
      <y>0</y>
      <scale>$factor</scale>
      <primary>yes</primary>
      <monitor>
        <monitorspec>
          <connector>Virtual-1</connector>
          <vendor>unknown</vendor>
          <product>unknown</product>
          <serial>unknown</serial>
        </monitorspec>
        <mode>
          <width>$width</width>
          <height>$height</height>
          <rate>$rate</rate>
        </mode>
      </monitor>
    </logicalmonitor>
  </configuration>
</monitors>
EOF
    chown "$user:$user" "$conf/monitors.xml"
    [[ -f /sys/fs/selinux/enforce ]] && restorecon -R "$conf"
  else
    echo "bluefin-vm-provision: could not read display mode from DRM; scale skipped" >&2
  fi
fi

# Applied -- clear the details from the durable share. The host re-writes them
# for the next fresh disk, and nothing sensitive lingers (public key only).
rm -rf "$pdir"
