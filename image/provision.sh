#!/usr/bin/env bash
# First-boot provisioning for a downloaded (generic) seed. The host writes the
# account details into the durable share before boot; this oneshot creates that
# account so a shipped seed feels personal, without a greeter. With no details
# present the service is condition-skipped and the baked test login stays the
# way in.
#
# Credential model: public key only, no password. A password-less account can't
# be reached through a greeter
# and can't sudo, so usability comes from three things together:
#
# - the ssh key (terminal),
# - autologin (desktop),
# - and a scoped passwordless-sudo rule (admin).
#
# That is the disposable-dev-VM posture: no password or private key ever lives
# in the VM, and the share only carries your public key, which this script
# clears after first boot.

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

# Public key(s): the only way in over ssh, since the account has no password.
if [[ -s $pdir/authorized_keys ]]; then
  install -d -m 700 -o "$user" -g "$user" "$home/.ssh"
  install -m 600 -o "$user" -g "$user" \
    "$pdir/authorized_keys" "$home/.ssh/authorized_keys"
  # Label as ssh_home_t where SELinux is active (the VM); a no-op elsewhere.
  # The guard's own failure is exempt from set -e; restorecon's is not.
  [[ -f /sys/fs/selinux/enforce ]] && restorecon -R "$home/.ssh"
fi

# Scoped passwordless sudo: without it a password-less account can administer
# nothing (sudo and polkit both want a password it doesn't have).
sudoers="/etc/sudoers.d/bluefin-vm-$user"
printf '%s ALL=(ALL) NOPASSWD:ALL\n' "$user" >"$sudoers"
chmod 440 "$sudoers"

# Autologin (no greeter) when the host asked for it -- a password-less account
# is only reachable at the desktop this way, and (below) the idle lock is
# disabled so it can't trap itself. Edit custom.conf in place with configparser
# so any settings the base image ships survive.
if [[ -e $pdir/autologin ]]; then
  python3 - "$user" <<'PY'
import configparser, os, sys
user = sys.argv[1]
path = "/etc/gdm/custom.conf"
cfg = configparser.ConfigParser()
cfg.optionxform = str  # keep key case (GDM is case-sensitive)
if os.path.exists(path):
    cfg.read(path)
if not cfg.has_section("daemon"):
    cfg.add_section("daemon")
cfg["daemon"]["AutomaticLoginEnable"] = "true"
cfg["daemon"]["AutomaticLogin"] = user
os.makedirs("/etc/gdm", exist_ok=True)
with open(path, "w") as f:
    cfg.write(f)
PY

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
fi

# Applied -- clear the details from the durable share. The host re-writes them
# for the next fresh seed, and nothing sensitive lingers (public key only).
rm -rf "$pdir"
