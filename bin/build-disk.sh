#!/usr/bin/env bash
set -euo pipefail

# Build a bootable Bluefin ARM64 (aarch64) image from a bootc container image
# via bootc-image-builder. Same entrypoint locally (Apple Silicon, Docker/Colima)
# and in CI (ARM64 Linux, Podman), so builds are reproducible in both places.
#
# The build is two steps (pull, then build) because the builder reads the image
# from container storage instead of pulling it; that story and the per-OS engine
# selection are documented in README "How the build works".
#
# Deliberately NOT passed to the builder:
# - --target-arch: experimental upstream; trips "cannot build iso for different
#   target arches yet". Build ARM on an ARM host instead.
# - any pull-avoidance flag: none exists -- the builder always reads local
#   storage, which is exactly why the pull step above it is needed.

# No default image here -- the default lives in the Justfile, keeping the
# plumbing environment-free.
image=""
format="qcow2"
builder="quay.io/centos-bootc/bootc-image-builder:latest"
store="bootc-store" # docker named volume for container storage
platform="linux/arm64"
dryrun="" # -n: print the commands instead of running them

usage() {
  cat <<EOF
Usage: $(basename "$0") -i IMAGE [-f FORMAT] [-n] [-h]

Build a bootable Bluefin ARM64 image from a bootc container.

Options:
  -i IMAGE    Source bootc container image (required).
  -f FORMAT   Output format (default: ${format}). One of: qcow2, raw, iso, vmdk
              qcow2/raw = pre-installed disks that boot straight into the OS;
              iso = installer.
  -n          Dry run: print the pull/build commands without executing them.
  -h          Show this help and exit.

Output lands in ./output (e.g. ./output/qcow2/disk.qcow2).

Engine is auto-selected: Podman on Linux (CI), Docker otherwise (macOS/Colima).
On Linux the Podman path needs rootful storage -- run via 'sudo $(basename "$0")'.
EOF
}

while getopts ":f:i:nh" opt; do
  case "$opt" in
    f) format="$OPTARG" ;;
    i) image="$OPTARG" ;;
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

case "$format" in
  qcow2 | raw | iso | vmdk) ;;
  *)
    echo "Error: unsupported format '$format'." >&2
    usage
    exit 1
    ;;
esac

if [ -z "$image" ]; then
  echo "Error: -i IMAGE is required (source container image)." >&2
  usage
  exit 1
fi

# Print the command (dry run) or execute it. Word-split $config_mount happens
# at the call site, so both branches see the same argv.
run_cmd() {
  if [ -n "$dryrun" ]; then
    printf '+ %s\n' "$*"
  else
    "$@"
  fi
}

# Use an interactive TTY only when we actually have one (not in CI).
tty=""
[ -t 0 ] && [ -t 1 ] && tty="-it"

# localhost/ refs (e.g. the derived image from image/Containerfile) exist only
# in the container store -- there is no registry to pull from, so skip the
# pull step and let the builder find them locally.
skip_pull=""
case "$image" in localhost/*) skip_pull=1 ;; esac

mkdir -p output

# Optional image customization (e.g. larger root filesystem) auto-read from
# /config.toml. Mounted only if present. NOTE: assumes no spaces in $PWD.
config_mount=""
[ -f config.toml ] && config_mount="-v $(pwd)/config.toml:/config.toml:ro"

if [ "$(uname -s)" = "Linux" ] && command -v podman >/dev/null 2>&1; then
  if [ -z "$skip_pull" ]; then
    echo ">> [podman] pulling $image into host container storage..."
    run_cmd podman pull "$image"
  fi

  echo ">> [podman] building $format from $image..."
  # shellcheck disable=SC2086 # $config_mount word-split is deliberate (see above)
  run_cmd podman run --rm $tty --privileged \
    --security-opt label=type:unconfined_t \
    -v "$(pwd)/output:/output" \
    -v /var/lib/containers/storage:/var/lib/containers/storage \
    $config_mount \
    "$builder" \
    --type "$format" \
    "$image"
else
  if [ -z "$skip_pull" ]; then
    echo ">> [docker] pulling $image into the '$store' volume (initializes storage)..."
    run_cmd docker run --rm --privileged \
      --platform "$platform" \
      -v "$store:/var/lib/containers/storage" \
      --entrypoint /usr/bin/podman \
      "$builder" \
      pull "$image"
  fi

  echo ">> [docker] building $format from $image..."
  # shellcheck disable=SC2086 # $config_mount word-split is deliberate (see above)
  run_cmd docker run --rm $tty --privileged \
    --platform "$platform" \
    -v "$(pwd)/output:/output" \
    -v "$store:/var/lib/containers/storage" \
    $config_mount \
    "$builder" \
    --type "$format" \
    "$image"
fi

echo ">> Done. Output in ./output/ (e.g. ./output/${format}/)"
