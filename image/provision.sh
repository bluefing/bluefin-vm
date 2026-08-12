#!/usr/bin/env bash
# First-boot provisioning for a downloaded (generic) disk. The host writes the
# account details into the durable share before boot; this oneshot creates that
# account so a shipped disk feels personal. With no details present the service
# is condition-skipped and the baked test login stays the way in.
#
# Credential model (a daily-driver account, not a throwaway): your ssh public
# key goes in, and the login password is set to `password == username` -- a
# public convention, not a secret (the value is the username, already in the
# share), giving a usable greeter, a working lock screen, and a password for
# `sudo` and GUI polkit prompts. From there two host-set flags tune the posture:
#
# - passwordless-sudo (default off) -> `sudo` prompts for that password, a guard
#   against a fat-fingered or pasted root command; the flag grants NOPASSWD.
# - disable-ssh-password (default off) -> ssh password login stays on (the VM
#   sits behind the host's NAT); the flag makes it pubkey-only.
#
# No private key or real secret lives in the VM or the share, which this script
# clears after first boot.

set -euo pipefail

# The share sub-directory the host wrote. Overridable so the integration tests
# can run this script against a synthetic share in a container.
pdir="${BLUEFIN_VM_PDIR:-/var/mnt/shared/bluefin-share/.bluefin-vm}"
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

# Login password == username: not a secret (the value is the username, already
# in the share), just a public convention that gives a usable greeter, a working
# lock screen, and a password for `sudo` and GUI polkit prompts.
echo "$user:$user" | chpasswd

# sudo prompts by default -- the login password guards a fat-fingered or pasted
# root command. Grant NOPASSWD only when the host asked for it.
if [[ -e $pdir/passwordless-sudo ]]; then
  sudoers="/etc/sudoers.d/bluefin-vm-$user"
  printf '%s ALL=(ALL) NOPASSWD:ALL\n' "$user" >"$sudoers"
  chmod 440 "$sudoers"
fi

# ssh password login stays on by default (the VM sits behind the host's NAT).
# Turn it off (pubkey-only) only when the host asked -- for a bridged or hardened
# VM. The 00- prefix wins first-match over the base image's own drop-ins; reload
# so it takes effect on this first boot, not just the next.
if [[ -e $pdir/disable-ssh-password ]]; then
  drop=/etc/ssh/sshd_config.d/00-bluefin-vm-nopassword.conf
  printf 'PasswordAuthentication no\n' >"$drop"
  # Relabel where SELinux is active so sshd (sshd_t) may read the drop-in.
  [[ -f /sys/fs/selinux/enforce ]] && restorecon "$drop"
  systemctl try-reload-or-restart sshd.service 2>/dev/null || true
fi

# Guest desktop scale: hand the requested percentage to the user session. It
# can't be applied here -- the scales mutter accepts are per-mode values only
# its session-bus API reports (the design doc and apply-scale.py carry the
# detail) -- so the bluefin-vm-apply-scale user oneshot, gated on this file,
# snaps and applies it at first login. The host writes the scale file only
# when the profile has refit off (a fixed resolution); with refit on the guest
# mode follows the window, so there's no stable mode to pin a scale to.
if [[ -s $pdir/scale ]]; then
  conf="$home/.config"
  install -d -m 700 -o "$user" -g "$user" "$conf" "$conf/bluefin-vm"
  install -m 644 -o "$user" -g "$user" "$pdir/scale" "$conf/bluefin-vm/scale-request"
  [[ -f /sys/fs/selinux/enforce ]] && restorecon -R "$conf"
fi

# Applied -- clear the details from the durable share. The host re-writes them
# for the next fresh disk, and nothing sensitive lingers (public key only).
rm -rf "$pdir"
