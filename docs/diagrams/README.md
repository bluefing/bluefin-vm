# diagrams — the build and boot flows

Mermaid diagrams of what the scripts and the `bluefin-vm` CLI actually do,
stage by stage, including the decisions each one makes. Each file covers one
flow; read `docs/content/just/build.md` and `docs/PROVISIONING.md` for the prose
version of the same ground.

- [`image-build.md`](image-build.md) — `bin/build-image.sh`: `image/Containerfile`
  into a `localhost/` image.
- [`disk-build.md`](disk-build.md) — `bin/build-disk.sh`: an image into a
  bootable disk.
- [`vm-import.md`](vm-import.md) — `bluefin-vm import`: a disk into a Tart VM.
- [`vm-up.md`](vm-up.md) — `bluefin-vm up`: the disk pipeline plus first-boot
  provisioning, end to end.
- [`states.md`](states.md) — the three boot states (unpatched / patched /
  provisioned), each built by one step (config.toml → Containerfile →
  provisioning), and what each gives you.

## How the pieces chain

`image/Containerfile`'s `FROM` is the same upstream image `build-disk.sh` can
also consume directly — layering the Containerfile first (patching) is
optional, not a required step in the chain:

```mermaid
flowchart TD
    BASE["Upstream: ghcr.io/projectbluefin/bluefin:lts-testing-arm64\n(image/Containerfile ARG BASE, overridable with -b)"]
    CF["image/Containerfile"]
    IMG["localhost/bluefin-vm-patched\n(build-image.sh output)"]
    DISK["disk.raw / .qcow2 / iso\n(build-disk.sh output)"]
    VM["Tart VM\n(bluefin-vm import)"]
    BOOT["Booted + provisioned VM\n(tart run / bluefin-vm up)"]

    BASE -->|FROM| CF
    CF -->|build-image.sh| IMG
    BASE -->|build-disk.sh -i BASE, unpatched| DISK
    IMG -->|build-disk.sh -i IMG, patched| DISK
    DISK --> VM
    VM --> BOOT
```

## A full local build, in order

The sequence a fresh `just tart up-patched` actually runs -- build the patched
image, build a disk from it, import into Tart, then start it:

```mermaid
sequenceDiagram
    participant Dev
    participant BuildImage as build-image.sh
    participant BuildDisk as build-disk.sh
    participant Import as bluefin-vm import
    participant Tart

    Dev->>BuildImage: build the patched image from BASE
    BuildImage-->>Dev: image in the container store
    Dev->>BuildDisk: build a raw disk from the patched image
    BuildDisk-->>Dev: output/image/disk.raw
    Dev->>Import: import the disk as VM Bluefin
    Import->>Tart: delete existing VM if present, create linux VM
    Import->>Tart: clone disk in, set cpu, memory, display
    Tart-->>Import: VM ready
    Dev->>Tart: run the VM with the share attached
    Tart-->>Dev: window open, guest booting
```

`bluefin-vm up` (see [`vm-up.md`](vm-up.md)) covers the same ground plus
downloading a published disk instead of building one, and first-boot
provisioning.
