#!/usr/bin/env bash
set -euo pipefail

# Build the derived VM image (image/Containerfile) into the container store
# that bootc-image-builder reads, selecting the engine the same way build-disk.sh
# does: Podman on Linux uses host storage directly (rootful -- run via sudo,
# like build-disk.sh); macOS/Docker builds with the builder image's bundled podman
# inside the shared store volume. The result is a localhost/ ref: store-only,
# nothing is pushed anywhere.

tag=""
base=""
context="image"
builder="quay.io/centos-bootc/bootc-image-builder:latest"
store="bootc-store" # docker named volume for container storage
platform="linux/arm64"
dryrun="" # -n: print the command instead of running it

usage() {
  cat <<EOF
Usage: $(basename "$0") -t TAG [-b BASE] [-n] [-h]

Build image/Containerfile into the container store as TAG (a localhost/ ref).

Options:
  -t TAG    Tag for the built image (required),
            e.g. localhost/bluefin-vm-patched:latest
  -b BASE   Base image to build FROM (optional; overrides the Containerfile's
            default so the layer tracks the configured source image).
  -n        Dry run: print the build command without executing it.
  -h        Show this help and exit.

Assumes CWD = repo root (build context: ./${context}).
Engine is auto-selected: Podman on Linux (run via sudo), Docker otherwise.
EOF
}

while getopts ":t:b:nh" opt; do
  case "$opt" in
    t) tag="$OPTARG" ;;
    b) base="$OPTARG" ;;
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

if [ -z "$tag" ]; then
  echo "Error: -t TAG is required." >&2
  usage
  exit 1
fi

# Word-split $build_arg happens at the call site, so both engines see the
# same argv (mirrors build-disk.sh's config_mount handling). BASE is the
# Containerfile's ARG name, not a shell variable.
build_arg=""
[ -n "$base" ] && build_arg="--build-arg BASE=$base"

run_cmd() {
  if [ -n "$dryrun" ]; then
    printf '+ %s\n' "$*"
  else
    "$@"
  fi
}

if [ "$(uname -s)" = "Linux" ] && command -v podman >/dev/null 2>&1; then
  echo ">> [podman] building $tag from $context/Containerfile..."
  # shellcheck disable=SC2086 # $build_arg word-split is deliberate (see above)
  run_cmd podman build $build_arg -t "$tag" "$context"
else
  echo ">> [docker] building $tag in the '$store' volume..."
  # shellcheck disable=SC2086 # $build_arg word-split is deliberate (see above)
  run_cmd docker run --rm --privileged \
    --platform "$platform" \
    -v "$store:/var/lib/containers/storage" \
    -v "$(pwd)/$context:/build:ro" \
    --entrypoint /usr/bin/podman \
    "$builder" \
    build $build_arg -t "$tag" /build
fi

echo ">> Done: $tag (store-only; build a disk from it with: ./bin/build-disk.sh -i $tag)"
