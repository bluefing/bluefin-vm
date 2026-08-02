#!/usr/bin/env bash
# Set a real login password for a provisioned account, replacing the default
# (password == username) that first-boot provisioning sets. Run it once in the
# VM if you'd rather log in with a password you chose.
set -euo pipefail

# Re-exec as root to set the password. sudo prompts for the current password
# (the username) unless the account was provisioned with passwordless sudo.
if [[ $EUID -ne 0 ]]; then
  exec sudo "$0" "$@"
fi

# The account is whoever invoked sudo (or an explicit argument for a root shell).
user="${SUDO_USER:-${1:-}}"
if [[ -z $user ]]; then
  echo "usage: bluefin-vm-harden [USER]" >&2
  exit 1
fi

echo "Set a new password for '$user':"
passwd "$user"
