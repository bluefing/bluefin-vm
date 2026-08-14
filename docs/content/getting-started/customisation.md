# Customising the VM

`bluefin-vm up` uses sensible defaults. Anything you change is saved to `~/.config/bluefin-vm/config.toml` and read by
later commands. Change it either with:

```bash
bluefin-vm tui
```

or by editing the file directly. Its keys, units, and precedence are covered in
[VM profiles](../reference/configuration.md).

The settings fall into three groups:

- **Account** — the username, which ssh public key is installed, whether `sudo` prompts for the login password, and
    whether ssh accepts password login at all.
- **Share** — the host directory mounted into the guest as `bluefin-share`, and whether the guest may write to it.
- **Resources** — virtual cores, memory, display resolution, desktop scale, and Refit.

In the TUI, pressing Enter on the last row saves and brings the VM up in one step.

## When a change takes effect

The account and the resources are fixed when the VM is created, so editing them applies to the next VM you create rather
than the one you have. Inside the VM, change what you like with the usual tools; those changes stay in the VM rather
than in its profile. The share is passed on every boot, so it applies immediately.

## Display

Refit, on by default, lets Tart resize the guest to follow the window — convenient, but the resolution is then whatever
the window happens to be, so a fixed resolution and a guest scale cannot hold. The form only offers Display and Scale
with Refit off.

With Refit off the guest runs at the resolution you chose and applies the scale at first login, snapped to the nearest
value that display supports. The supported values are a per-mode set that only the running desktop knows, so a 150%
target can land on 133%.

## More than one VM

Profiles are named, and `Bluefin` is the default name used when you do not specify one. To create another machine, use
another name.

```bash
bluefin-vm tui --name work    # creates the profile
bluefin-vm up --name work     # creates the VM
```

If a profile with the given name does not exist it will be created using default values. Each profile has its own
account, share, and resources. The disk is cached by checksum, so a second VM reuses the download rather than repeating
it.
