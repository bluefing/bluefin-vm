# Architecture

This project builds from the upstream Bluefin bootc image, to avoid sitting through an ISO installer and then repeating
the same guest setup — sshd, the host share, clipboard — for every VM created.

Patching happens in CI, before you download: the guest setup is baked into the disk and a provisioner waits, dormant,
for your details. The disk is generic, the same one for everyone. Provisioning happens on your machine, at first boot,
and is what makes it yours.

```mermaid
flowchart LR
    IMG["Bluefin bootc image"] -->|CI builds| DISK["Published disk"]
    DISK -->|download, import| VM["Tart VM"]
    VM -->|provision, boot| DESK["Your desktop"]
```

`bluefin-vm up` is the right-hand half: download the disk, import it into Tart, stage your account, boot. Each of those
is also its own subcommand, so a failed run can be picked up or inspected a step at a time.

## Boot states

The VM boots into one of two states.

- **Patched**
    - the downloaded disk, with
        - sshd,
        - the virtiofs share mounted as `~/Shared`,
        - clipboard sharing,
        - a dormant provisioner
- **Provisioned**
    - the patched disk after the provisioner applied your profile

The `bluefin` account is baked into every disk and survives provisioning; your account is added beside it rather than
replacing it.

## Provisioning

The host writes your account details into the share before the VM starts for the first time, and the guest applies them
on first boot. One published disk then serves everyone, and nothing has to ssh into the VM afterwards to finish the job.

## Shared directory

The share is the VM's durable tier, with its data on the host. Replacing the VM leaves it untouched. Anything kept
inside the guest goes with it.

## Disk cache

A downloaded disk is stored under the checksum published alongside it. A new build has a new checksum, so it lands
beside the old one rather than overwriting it, and `up` reuses a cached disk only when it matches the build it is
fetching. The VM gets its own copy at import, so the cache can be purged at any time.
