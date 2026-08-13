# `bluefin-vm up` — the end-to-end pipeline

The one command that brings the VM up. An existing VM holds the user's state,
so it simply boots; the pipeline — download → extract → import → provision →
run — runs when the VM is missing or `--replace` asks for a fresh one. Each
pipeline step is also its own subcommand
(`bluefin-vm download`/`extract`/`import`/`provision`) for debugging; `up`
chains them with the skip-if-already-done checks each step owns.

```mermaid
sequenceDiagram
    participant User
    participant CLI as bluefin-vm up
    participant Cache as local seed cache
    participant Config as config.toml
    participant Tart
    participant Guest as guest first boot

    User->>CLI: bluefin-vm up
    alt VM exists and no --replace
        CLI->>Tart: run detached with the share attached
        Tart-->>User: the existing VM boots, state intact
    end
    CLI->>Cache: download the seed zip, resumable
    alt already the full size
        Cache-->>CLI: already downloaded
    else partial or missing
        Cache-->>CLI: fetched, resumed or fresh
    end
    CLI->>Cache: extract disk.raw from the zip
    alt disk.raw already matches the zip entry size
        Cache-->>CLI: already extracted
    else
        Cache-->>CLI: inflated fresh
    end
    CLI->>Config: load the profile, account, share, resources
    CLI->>Tart: import the disk with the resolved spec
    Tart-->>CLI: VM created, disk cloned, cpu memory display set
    opt provisioning not skipped
        CLI->>CLI: resolve account, flag then saved profile then host default
        CLI->>Cache: write username, authorized keys, sudo/ssh flags, scale into the share
        CLI->>Config: save the resolved account and resources back
    end
    CLI->>Tart: run detached with the share attached
    Tart->>Guest: boot
    opt share carries a pending username
        Guest->>Guest: run the provisioning oneshot, create the account, install the key, set password == username, apply the sudo/ssh flags, write monitors.xml for the scale
        Guest->>Cache: clear the pending share, consumed once
    end
    Guest-->>User: greeter login (password == username), or the baked test login
```

Precedence throughout is **flag > saved profile > built-in default** (`up`'s
own flags, or a VM's profile from `bluefin-vm tui`). Provisioning happens on a
fresh VM's first boot only, so account and resource changes in the profile
apply through `up --replace`; the share settings are passed on every boot and
follow the profile immediately.
