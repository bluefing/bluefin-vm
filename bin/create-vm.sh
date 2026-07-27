#!/usr/bin/env bash
set -euo pipefail

# Import a built disk into a Tart VM, replacing the VM if it exists.
#
# Tart boots a RAW disk. A qcow2 is accepted and converted via the builder
# image's bundled qemu-img (the conversion writes a non-sparse raw over
# virtiofs -- prefer building raw directly). Inputs are identified by
# CONTENT, not extension -- extensions lie -- and validated before the
# destructive VM replacement: a bad input must never cost a working VM.

disk=""
name="Bluefin"
cpu="${TART_CPU:-4}"
mem="${TART_MEM:-4096}"
display_res="${TART_DISPLAY:-1920x1200}"
TART_HOME="${TART_HOME:-$HOME/.tart}"
builder="quay.io/centos-bootc/bootc-image-builder:latest"
dryrun="" # -n: print the commands instead of running them

usage() {
  cat <<EOF
Usage: $(basename "$0") -d DISK [-N NAME] [-n] [-h]

Import a raw (or qcow2) disk into a Tart VM, replacing the VM if it exists.

Options:
  -d DISK   Disk image to import (required): raw or qcow2, detected by
            content.
  -N NAME   Tart VM name (default: ${name}).
  -n        Dry run: print the commands without executing them.
  -h        Show this help and exit.

Env: TART_CPU (default 4), TART_MEM (MiB, default 4096),
     TART_DISPLAY (WxH, default 1920x1200)
EOF
}

while getopts ":d:N:nh" opt; do
  case "$opt" in
    d) disk="$OPTARG" ;;
    N) name="$OPTARG" ;;
    n) dryrun=1 ;;
    h)
      usage
      exit 0
      ;;
    :)
      echo "Error: -$OPTARG requires an argument." >&2
      usage
      exit 1
      ;;
    \?)
      echo "Error: unknown option -$OPTARG." >&2
      usage
      exit 1
      ;;
  esac
done

if [ -z "$disk" ]; then
  echo "Error: -d DISK is required (raw or qcow2). Build one:  just build raw" >&2
  usage
  exit 1
fi
if [ ! -f "$disk" ]; then
  echo "Error: disk not found: $disk" >&2
  exit 1
fi

# Dry runs stay offline-testable (bats, CI) -- only real runs need tart.
if [ -z "$dryrun" ]; then
  command -v tart >/dev/null 2>&1 || {
    echo "Error: tart not installed (brew install cirruslabs/cli/tart)." >&2
    exit 1
  }
fi

run_cmd() {
  if [ -n "$dryrun" ]; then
    printf '+ %s\n' "$*"
  else
    "$@"
  fi
}

# Identify the input by magic bytes: qcow2 header, ISO9660 signature, GPT
# header at LBA 1. The ISO check must come BEFORE the GPT check -- hybrid
# installer ISOs contain a GPT and would otherwise pass as raw disks.
magic4=$(dd if="$disk" bs=4 count=1 2>/dev/null | od -An -tx1 | tr -d ' \n')
iso_sig=$(dd if="$disk" bs=1 skip=32769 count=5 2>/dev/null || true)
gpt_sig=$(dd if="$disk" bs=8 skip=64 count=1 2>/dev/null || true)

raw="$disk"
if [ "$magic4" = "514649fb" ]; then
  diskdir="$(cd "$(dirname "$disk")" && pwd)"
  raw="$diskdir/$(basename "$disk" .qcow2).raw"
  echo ">> Converting qcow2 -> raw via builder qemu-img..."
  run_cmd docker run --rm --platform linux/arm64 -v "$diskdir:/w" \
    --entrypoint /usr/bin/qemu-img "$builder" \
    convert -f qcow2 -O raw "/w/$(basename "$disk")" "/w/$(basename "$raw")"
elif [ "$iso_sig" = "CD001" ]; then
  echo "Error: $disk is an installer ISO -- Tart boots a disk image, not an ISO." >&2
  echo "Build a disk instead:  just build raw" >&2
  exit 1
elif [ "$gpt_sig" != "EFI PART" ]; then
  echo "Error: $disk is neither a raw disk (no GPT) nor a qcow2." >&2
  echo "Build one:  just build raw" >&2
  exit 1
fi

echo ">> Creating Tart VM '$name' from $raw"
run_cmd tart delete "$name" 2>/dev/null || true
run_cmd tart create --linux "$name"

vmdir="$TART_HOME/vms/$name"
if [ -z "$dryrun" ] && [ ! -d "$vmdir" ]; then
  echo "Error: expected Tart VM dir not found: $vmdir" >&2
  exit 1
fi

echo ">> Swapping in the built disk (APFS clone if possible)..."
run_cmd cp -c "$raw" "$vmdir/disk.img" 2>/dev/null || run_cmd cp "$raw" "$vmdir/disk.img"

# 16:10 instead of tart's 1024x768 default; --display-refit makes the guest
# resolution follow window resizes/fullscreen where the guest supports it
# (GNOME + virtio-gpu does).
run_cmd tart set "$name" --cpu "$cpu" --memory "$mem" --display "$display_res" --display-refit

echo ">> Done: Tart VM '$name'"
echo "   Start (window):    just tart start $name"
echo "   Start (headless):  just tart start-headless $name"
echo "   Ship it (OCI):     just tart push $name"
