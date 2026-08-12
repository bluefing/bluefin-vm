# First-boot provisioning

!!! note "Migration in progress"
    This page will hold the audience-facing account and provisioning model. The
    prose is being migrated and reframed (daily-driver, not disposable) from the
    repo's `docs/PROVISIONING.md`; the design rationale lives in the internal
    `design/access.md`. Tracked in `internal/planning/migration.md`.

At first boot a downloaded disk provisions your account from details the host
wrote to the durable share: it creates the user in `wheel`, installs your ssh
public key, and sets the login password; your chosen display scale is applied at
first login, snapped to the nearest value the guest display supports. With no
details present the step is skipped and the baked test login stays the way in.
