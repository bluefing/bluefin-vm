# Default source image: the arm64 build that currently boots. Why this testing
# tag, and when to promote back to stable: README "Which image?" (BL-8 tracks
# it). Override on any build recipe with -i/--image.
default_image := "ghcr.io/ublue-os/bluefin:lts-testing-50-arm64"

# Default VM label -- the display/registration name only. This is NOT the image;
# pick the source image with -i/--image on the build/vm recipes.
default_name := "Bluefin"

default_ref := "ghcr.io/projectbluefin/bluefin-vm:latest"

# Host folder shared into the VM -- the durable tier: data written
# here survives VM re-seed/reset and is visible to macOS + Time Machine.
# Deliberately NOT under ~/Documents: that folder is iCloud-synced on many
# Macs, and evicted "dataless" files would stall guest reads.
default_share := env("TART_SHARE_DIR", env("HOME") / "bluefin-share")

# The derived image built from image/Containerfile. localhost/ = store-only
# (bin/build-disk.sh skips its pull step for these refs).
patched_image := "localhost/bluefin-vm-patched:latest"
