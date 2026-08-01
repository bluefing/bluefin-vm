# image build — `bin/build-image.sh`

Layers `image/Containerfile` onto its upstream base and lands the result in
the container store as a `localhost/` ref (never pushed anywhere) --
store-only, so `build-disk.sh` skips pulling it.

```mermaid
flowchart TD
    START["bin/build-image.sh -t TAG [-b BASE]"]
    BASE["image/Containerfile ARG BASE\n(default: ghcr.io/projectbluefin/bluefin:lts-testing-arm64,\noverridden by -b)"]
    ID["Stamp build identity:\nBUILD_REF/BUILD_SHA from CI env, else git HEAD"]
    ENGINE{"Host: Linux with podman?"}
    PODMAN["podman build --build-arg BUILD_REF/BUILD_SHA[/BASE]\n-t TAG image/"]
    DOCKER["docker run --privileged bootc-image-builder\n--entrypoint podman build -t TAG /build\n(image/ mounted read-only; store = bootc-store volume)"]
    OUT["Output: TAG, a localhost/ ref, store-only"]
    NEXT["Ready for: bin/build-disk.sh -i TAG"]

    START --> BASE --> ID --> ENGINE
    ENGINE -- yes --> PODMAN --> OUT
    ENGINE -- "no (macOS/Docker)" --> DOCKER --> OUT
    OUT --> NEXT
```

`-b BASE` overrides the Containerfile's own default, so the patched layer
tracks whichever upstream image the caller configured. On Linux the Podman
path needs rootful storage (run via `sudo`, same as `build-disk.sh`).
