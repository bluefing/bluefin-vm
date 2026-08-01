# disk build — `bin/build-disk.sh`

Turns a container image -- upstream, or the patched `localhost/` image from
`image-build.md` -- into a bootable disk via `bootc-image-builder`. Same
entrypoint locally (Docker/Colima) and in CI (`ubuntu-24.04-arm`, Podman
pre-installed): `.github/workflows/build-arm-image.yml` just calls this
script with its inputs as flags.

```mermaid
flowchart TD
    START["bin/build-disk.sh -i IMAGE [-f FORMAT]"]
    LOCAL{"IMAGE starts with localhost/ ?"}
    PULL["Pull IMAGE into container storage"]
    CFG{"config.toml present?"}
    MOUNT["Mount it read-only at /config.toml\n(20 GiB root, the baked bluefin test login)"]
    NOMOUNT["No mount -- builder's own defaults apply"]
    ENGINE{"Host: Linux with podman?"}
    PODMAN["podman run --privileged bootc-image-builder\n--type FORMAT IMAGE\n(host's /var/lib/containers/storage mounted directly)"]
    DOCKER["docker run --privileged bootc-image-builder\n--type FORMAT IMAGE\n(bootc-store volume; needs its own pull step above\nsince Docker has no shared host storage)"]
    OUT["Output: output/FORMAT/ (e.g. output/image/disk.raw)"]

    START --> LOCAL
    LOCAL -- "yes: store-only, no registry" --> CFG
    LOCAL -- no --> PULL --> CFG
    CFG -- yes --> MOUNT --> ENGINE
    CFG -- no --> NOMOUNT --> ENGINE
    ENGINE -- yes --> PODMAN --> OUT
    ENGINE -- "no (macOS/Docker)" --> DOCKER --> OUT
```

`FORMAT` is one of `raw` (what Tart imports), `qcow2` (thin-provisioned,
generic), `iso` (installer), or `vmdk`. On Linux the Podman path needs
rootful storage for loop devices (run via `sudo`); on macOS, Colima needs
headroom (`colima start --cpu 4 --memory 8`) and ~20 GB free disk for a raw
build.
