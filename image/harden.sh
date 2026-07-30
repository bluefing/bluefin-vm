#!/usr/bin/env bash
# Lock down a provisioned account: give it a password and drop the passwordless
# sudo rule that first-boot provisioning added. A downloaded seed's account
# starts password-less by design (pubkey only) -- run this once if you want the
# stricter posture.
set -euo pipefail

# Re-exec as root: this sets a password and edits /etc/sudoers.d. While the
# NOPASSWD rule is still in place, the sudo prompt won't ask for one.
if [[ $EUID -ne 0 ]]; then
  exec sudo "$0" "$@"
fi

# The account is whoever invoked sudo (or an explicit argument for a root shell).
user="${SUDO_USER:-${1:-}}"
if [[ -z $user ]]; then
  echo "usage: bluefin-vm-harden [USER]" >&2
  exit 1
fi

# Password first: if it's mistyped this exits before touching sudoers, so a
# failed run never leaves the account both password-less and un-elevatable.
echo "Set a password for '$user':"
passwd "$user"

drop="/etc/sudoers.d/bluefin-vm-$user"
if [[ -e $drop ]]; then
  rm -f "$drop"
  echo "Removed passwordless sudo; it now asks for your password."
else
  echo "No passwordless-sudo rule for '$user' -- already hardened."
fi
