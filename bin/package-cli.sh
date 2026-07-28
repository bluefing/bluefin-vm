#!/usr/bin/env bash
set -euo pipefail

# Build the `bluefin-vm` tool into a release tarball for the Homebrew tap.
#
# The brew formula ships the TOOL, not the seed: it fetches this tarball from a
# GitHub Release (versioned by git tag) and installs the binary; the installed
# tool downloads the seed at runtime. So the seed's hosting (R2, manual) is
# fully decoupled from releasing the tool. CI (.github/workflows/release.yml)
# runs this exact script on a native arm64 macOS runner -- local and CI produce
# the identical artifact. Assumes CWD = repo root.

version="" # -v: override; default is the crate version in cli/Cargo.toml
outdir="output"
dryrun="" # -n: print the commands instead of running them

# The tool is Apple-Silicon-only (it drives Apple's Virtualisation framework via
# tart), so there is one target triple. A universal/x86 build would never boot.
triple="aarch64-apple-darwin"

usage() {
  cat <<EOF
Usage: $(basename "$0") [-v VERSION] [-o OUTDIR] [-n] [-h]

Build the bluefin-vm release binary and package it as
  OUTDIR/bluefin-vm-VERSION-${triple}.tar.gz  (+ a .sha256 sidecar)
for the Homebrew tap to fetch from a GitHub Release.

Options:
  -v VERSION  Version string for the asset name (default: the crate version
              read from cli/Cargo.toml).
  -o OUTDIR   Directory to write the tarball into (default: ${outdir}).
  -n          Dry run: print the commands without executing them.
  -h          Show this help and exit.
EOF
}

while getopts ":v:o:nh" opt; do
  case "$opt" in
    v) version="$OPTARG" ;;
    o) outdir="$OPTARG" ;;
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

# Default the version to the crate's own -- keeping the asset name, the tag,
# and Cargo.toml in lockstep (release.yml asserts the tag matches too).
if [ -z "$version" ]; then
  version=$(sed -n 's/^version = "\(.*\)"/\1/p' cli/Cargo.toml | head -n1)
  [ -n "$version" ] || {
    echo "Error: could not read version from cli/Cargo.toml." >&2
    exit 1
  }
fi

asset="bluefin-vm-${version}-${triple}.tar.gz"
bindir="cli/target/release"

# Dry runs stay offline-testable (bats, CI) -- only real runs need the toolchain
# and the host to actually be arm64 macOS.
if [ -z "$dryrun" ]; then
  [ "$(uname -s)" = "Darwin" ] && [ "$(uname -m)" = "arm64" ] || {
    echo "Error: the tool is arm64 macOS only; build it on Apple Silicon." >&2
    exit 1
  }
  command -v cargo >/dev/null 2>&1 || {
    echo "Error: cargo not found (install the Rust toolchain)." >&2
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

echo ">> Building bluefin-vm $version (release)"
run_cmd cargo build --release --manifest-path cli/Cargo.toml

echo ">> Packaging $outdir/$asset"
run_cmd mkdir -p "$outdir"
# -C into the binary's dir so the tarball holds a bare `bluefin-vm`, which the
# formula's `bin.install` expects -- no nested path to strip.
run_cmd tar -czf "$outdir/$asset" -C "$bindir" bluefin-vm

# Sidecar checksum with the bare filename (cd in so the path isn't embedded),
# so the formula's sha256 and `brew fetch` verification line up.
echo ">> Writing $outdir/$asset.sha256"
if [ -n "$dryrun" ]; then
  printf '+ %s\n' "cd $outdir && shasum -a 256 $asset > $asset.sha256"
else
  (cd "$outdir" && shasum -a 256 "$asset" >"$asset.sha256")
  sha=$(awk '{print $1}' "$outdir/$asset.sha256")
  echo ">> Done: $outdir/$asset"
  echo "   sha256: $sha"
fi
