# Design — command surface

What `bluefin-vm` (`bv`) exposes as commands, how config is created and edited, and where the TUI fits. The
account/credential model is in `access.md`; this is about the CLI/TUI shape. Decision recorded here; implementation is a
separate branch.

## Principle: the TUI is optional, end to end

Everything is doable from the CLI. The TUI is a convenience front-end over the same UI-agnostic core (`core/tart.rs`,
`core/provision.rs`, `core/config.rs`), never the only path. A user who never opens the TUI can create a config, edit it
(by hand or via `bv config`), and run the whole pipeline. This is already largely true — the pipeline stages are all
subcommands today; the changes below make the config step explicit and rename the TUI entry point.

## Config — `bv config`

Per-VM settings live in *named profiles* in `~/.config/bluefin-vm/config.toml` (XDG-aware, `config.rs::path()`). The
`config` subcommand:

- **`bv config init`** — scaffold the file with a default profile. **Never clobbers:** if a config already exists, print
    its path and exit without writing (a `--force` opt-in to overwrite). Today the file is written implicitly by the TUI
    / `up`; `init` makes it explicit for the config-first, CLI-first workflow.
- **`bv config path`** — print the resolved config path.
- **`bv config show`** — print the current config (the resolved profile(s)).

## The TUI — `bv tui`

`bv tui` launches the interactive front-end: edit a profile and kick off the VM's provision/up. Renamed from `setup` —
once the TUI can launch VMs (see the backlog), "setup" undersells it. Pre-release, so a straight rename with no alias.

## Pipeline commands (unchanged)

`bv up` runs the whole pipeline; its stages are also individual subcommands — `download`, `extract`, `import`,
`provision` — for debugging and scripting. These are what already make the CLI complete without the TUI, and they don't
change.

## The change set (for the implementation branch)

- Rename the `Setup` command to `Tui`.
- Add a `Config` subcommand with `init` (no-clobber, `--force`), `path`, `show`.
- Document the end-to-end-without-TUI guarantee.
