# VM import — `bluefin-vm import`

Imports a built disk into a Tart VM, replacing the VM if it exists. One
implementation, in `cli/src/core/tart.rs`; the `just tart import` / `up`
recipes call it rather than reimplementing it. The input must be a raw GPT
disk (identified by content, not filename), and it's validated *before* the
destructive delete+recreate — a bad input must never cost a working VM.

```mermaid
flowchart TD
    START["bluefin-vm import --disk DISK --name NAME"]
    QCOWCHK{"qcow2 header at byte 0?"}
    ERR_QCOW["Error, qcow2 not supported, raw only"]
    GPTCHK{"GPT signature EFI PART at byte 512?"}
    ERR_NEITHER["Error, not a raw GPT disk"]
    RESOLVE["Resolve VmSpec from the saved profile: cpu, memory, display, refit; defaults where unset"]
    DELETE["tart delete NAME if present, then tart create linux NAME"]
    VMDIR{"Tart VM dir exists?"}
    ERR_DIR["Error, expected VM dir not found"]
    CLONE["Clone disk to vmdir disk.img, APFS clone, falls back to a full copy"]
    REFIT{"refit on?"}
    SETON["tart set cpu memory display --display-refit"]
    SETOFF["tart set cpu memory display (no refit: fixed resolution)"]
    OUT["Tart VM NAME ready to start"]

    START --> QCOWCHK
    QCOWCHK -- yes --> ERR_QCOW
    QCOWCHK -- no --> GPTCHK
    GPTCHK -- no --> ERR_NEITHER
    GPTCHK -- yes --> RESOLVE --> DELETE --> VMDIR
    VMDIR -- no --> ERR_DIR
    VMDIR -- yes --> CLONE --> REFIT
    REFIT -- yes (default) --> SETON --> OUT
    REFIT -- no --> SETOFF --> OUT
```

Resources come solely from the VM's saved profile (`bluefin-vm tui`); the
built-in defaults are 4 vCPUs, 4096 MiB, a 1920×1200 display, and refit on.
With refit on, `--display-refit` lets the guest resolution follow the Tart
window; with it off, the display stays fixed, which is what lets a chosen
resolution and guest scale hold (the scale itself is applied guest-side at
first boot — see [`vm-up.md`](vm-up.md)).
