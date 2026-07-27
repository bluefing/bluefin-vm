import '../_config.just'
import '../_common.just'

[private]
mod build '../build/'

# --- lifecycle: the everyday verbs ------------------------------------------

# One shot for the derived image (image/Containerfile): build the patched
# container, build a fresh raw disk from it, re-import, then hand off to
# `up` for the detached start. Unlike plain `up`, this ALWAYS rebuilds and
# REPLACES the VM's disk/state -- it is the explicit "fresh patched VM" verb.
[doc('Rebuild from image/Containerfile and start a fresh patched VM (replaces VM state)')]
[group('lifecycle')]
up-patched name=default_name: (build::image) (build::raw patched_image) (import name) (up name)

# Bring the VM up, doing only the missing work: build the raw disk if absent,
# re-import when the disk is newer than the VM's copy (this replaces VM state),
# then start detached -- the terminal comes back; startup failures still fail
# the recipe. Force a fresh disk with `just build raw` -- up then picks it up.
# NAME = label, -i = image (used only when a build actually happens).
[doc('Bring the VM up (build/import only what is missing or stale), then start detached')]
[arg('image', long, short='i')]
[group('lifecycle')]
[no-cd]
[script]
up name=default_name image=default_image:
  disk=output/image/disk.raw
  vmdisk="${TART_HOME:-$HOME/.tart}/vms/{{name}}/disk.img"
  if [ -f "$disk" ]; then
    echo ">> Reusing $disk (force a rebuild first: just build raw)"
  else
    ./bin/build-disk.sh -f raw -i "{{image}}"
  fi
  if [ ! -f "$vmdisk" ] || [ "$disk" -nt "$vmdisk" ]; then
    ./bin/create-vm.sh -d "$disk" -N "{{name}}"
  else
    echo ">> Tart VM '{{name}}' already has this disk"
  fi
  log="${TMPDIR:-/tmp}/tart-{{name}}.log"
  mkdir -p "{{default_share}}"
  nohup tart run "{{name}}" --dir="bluefin-share:{{default_share}}" >"$log" 2>&1 &
  pid=$!
  sleep 3
  # kill -0 alone would pass for a zombie (this shell is still the parent), so
  # check the process state -- startup errors must fail loudly, not vanish.
  case "$(ps -o stat= -p "$pid" 2>/dev/null || true)" in
    "" | Z*)
      echo ">> tart run died at startup; log ($log):" >&2
      cat "$log" >&2
      exit 1
      ;;
  esac
  echo ">> VM '{{name}}' running detached (window open). Log: $log"
  echo ">> Stop: just tart stop -- attached run: just tart start"

# SSH into a running Tart VM (user: the baked test login; see BL-6).
# --resolver arp because the default DHCP-lease lookup returns nothing on
# macOS 26; ARP answers for any running VM.
[doc('SSH into a running Tart VM as the baked test login')]
[group('lifecycle')]
ssh name=default_name:
  ssh "${TART_SSH_USER:-bluefin}@$(tart ip "{{name}}" --resolver arp)"

# Stop a running Tart VM
[group('lifecycle')]
stop name=default_name:
  -tart stop "{{name}}"

# --- attached: watch-it / debug variants ------------------------------------

# Start a Tart VM in a window (attached; shares the durable folder)
[group('attached')]
start name=default_name:
  mkdir -p "{{default_share}}"
  tart run "{{name}}" --dir="bluefin-share:{{default_share}}"

# Start a Tart VM headless (no window; shares the durable folder)
[group('attached')]
start-headless name=default_name:
  mkdir -p "{{default_share}}"
  tart run "{{name}}" --no-graphics --dir="bluefin-share:{{default_share}}"

# --- plumbing: called by the verbs above, or rarely by hand -----------------

# Import the built raw disk (output/image/disk.raw) into a Tart VM
[no-cd]
[group('plumbing')]
import name=default_name:
  ./bin/create-vm.sh -d output/image/disk.raw -N "{{name}}"

# Push a Tart VM to an OCI registry (override ref for your org)
[group('plumbing')]
push name=default_name ref=default_ref:
  tart push "{{name}}" "{{ref}}"
