import '../_config.just'
import '../_common.just'

# Shared builder: build IMAGE as TYPE (iso|qcow2|raw). Private -- use iso/qcow2/raw.
[private]
[no-cd]
[no-exit-message]
create type image=default_image:
  ./bin/build-disk.sh -f {{type}} -i "{{image}}"

# Build an ISO installer from IMAGE (-i/--image)
[arg('image', long, short='i')]
[group('image-build')]
iso image=default_image: (create "iso" image)

# Build a qcow2 disk from IMAGE (-i/--image)  -- generic; no in-repo consumer
[arg('image', long, short='i')]
[group('image-build')]
qcow2 image=default_image: (create "qcow2" image)

# Build a raw disk from IMAGE (-i/--image)  -- used by Tart
[arg('image', long, short='i')]
[group('image-build')]
raw image=default_image: (create "raw" image)

# Build the derived image (image/Containerfile) into the container store.
# Passing the configured default image as BASE keeps the layer tracking one
# source of truth instead of the Containerfile's own ARG default.
[doc('Build the derived image (image/Containerfile) into the container store')]
[group('image-build')]
[no-cd]
image:
  ./bin/build-image.sh -t "{{patched_image}}" -b "{{default_image}}"
