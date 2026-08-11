import '../_config.just'
import '../_common.just'

[private]
mod build '../build/'

# up-provisioned stages the account through the real host-side writer, so the
# dev loop drives the same code `bluefin-vm up` ships rather than a stub.
[private]
mod cli '../cli/'

# --- lifecycle: the everyday verbs ------------------------------------------

# One shot for the derived image (image/Containerfile): build the patched
# container, build a fresh raw disk from it, re-import, then hand off to
# `up` for the detached start. Unlike plain `up`, this ALWAYS rebuilds and
# REPLACES the VM's disk/state -- it is the explicit "fresh patched VM" verb.
[doc('Rebuild from image/Containerfile and start a fresh patched VM (replaces VM state)')]
[group('lifecycle')]
up-patched name=default_name: (build::image) (build::raw patched_image) (import name) (up name)

# up-patched, but stage first-boot provisioning into the share first (via the
# real `bluefin-vm provision`), so the freshly built disk boots THROUGH
# provisioning into your own account rather than the baked test login -- the
# inner loop for changing image/provision.sh. The account is host $USER with an
# autodetected ssh key; for other accounts or flags, run `just cli run provision
# ...` yourself before an `up`. Like up-patched it always rebuilds and REPLACES
# the VM.
[doc('Stage provisioning, then rebuild and boot a fresh VM through it (replaces VM state)')]
[group('lifecycle')]
up-provisioned name=default_name: (cli::run "provision" "--share" default_share) up-patched

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
    cargo run --manifest-path cli/Cargo.toml --release -- import --disk "$disk" --name "{{name}}"
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

# SSH into a running Tart VM. Defaults to your own host account -- the one
# provisioning creates -- so a provisioned VM needs no flags; pass --user/-u
# for the baked bluefin login or any other account. The default is the literal
# $USER, expanded by the shell at run time.
[doc('SSH into a running Tart VM (--user; default: your host account)')]
[arg('user', long, short)]
[group('lifecycle')]
ssh name=default_name user='$USER':
  ssh "{{user}}@$(just tart ip "{{name}}")"

# The default DHCP-lease lookup returns nothing on macOS 26, so this resolves by
# ARP, which answers for any running VM. ssh and smoke call it, keeping that
# rationale in one place.
[doc('Print the IP of a running Tart VM (via arp)')]
[group('lifecycle')]
ip name=default_name:
  tart ip "{{name}}" --resolver arp

# Run the guest smoke test in a running VM: deliver tests/e2e/guest-checks.sh
# through the share, run it over ssh with a host-generated run id, then assert
# the guest wrote that exact file back -- which proves the share round-trip
# deterministically (host picks the name, so seeing it isn't coincidence). The
# recipe's exit code is the guest's (checks pass/fail). The VM must already be
# up with the share attached (just tart up / start). Host-key checking is off:
# test VMs are disposable and re-keyed each rebuild. Defaults to the baked
# bluefin login -- smoke validates baked patches on a fresh disk, where a
# provisioned account may not exist; pass --user/-u for a provisioned VM.
[doc('Run the guest smoke test in a running VM; assert the share round-trip')]
[arg('user', long, short)]
[no-cd]
[group('lifecycle')]
[script]
smoke name=default_name user='bluefin':
  run_id="$(date +%Y%m%d-%H%M%S)"
  expected="{{default_share}}/${run_id}-guest-checks.log"
  mkdir -p "{{default_share}}"
  cp tests/e2e/guest-checks.sh "{{default_share}}/"
  guest_rc=0
  ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
    "{{user}}@$(just tart ip "{{name}}")" \
    "bash ~/Shared/guest-checks.sh $run_id" || guest_rc=$?
  echo ">> share round-trip: expecting $expected"
  if [ -f "$expected" ]; then
    echo ">> PASS: guest wrote the run's log and the host can see it"
  else
    echo ">> FAIL: that file is not visible on the host -- share round-trip broken" >&2
    exit 1
  fi
  exit "$guest_rc"

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

# Import the built raw disk (output/image/disk.raw) into a Tart VM via the
# bluefin-vm CLI -- the single import implementation (validation, VM replace,
# resource set all live in cli/src/core/tart.rs).
[no-cd]
[group('plumbing')]
import name=default_name:
  cargo run --manifest-path cli/Cargo.toml --release -- import --disk output/image/disk.raw --name "{{name}}"

# Push a Tart VM to an OCI registry (override ref for your org)
[group('plumbing')]
push name=default_name ref=default_ref:
  tart push "{{name}}" "{{ref}}"
