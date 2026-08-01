# Architecture and flows

!!! note "Migration in progress"
    The build and boot flow diagrams (image build, disk build, VM import, `up`,
    and the boot states) are being migrated from the repo's `docs/diagrams/`
    into this page. Tracked in `internal/planning/migration.md`.

The pipeline turns an upstream Bluefin bootc image into a running, personalised
VM in stages, each its own subcommand: **download → extract → import → provision
→ run**. A built disk boots into one of three states — *unpatched* (the OS image
plus a baked test account), *patched* (adds ssh, the host share, clipboard, and
a dormant first-boot provisioner), and *provisioned* (adds your account, ssh
key, and display scale).
