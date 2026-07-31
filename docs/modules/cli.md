# cli — the bluefin-vm tool

The `cli` recipes wrap the `bluefin-vm` Rust binary — the front door a user
installs from Homebrew. `just cli` lists the verbs; this page covers what the
tool does and how the recipes drive it.

## What the tool does

`bluefin-vm up` is the whole pipeline:

- downloads the published disk archive (resumable, optionally checksum-verified),
- extracts the raw disk,
- imports it into Tart,
- provisions your first-boot account,
- then boots.

The other subcommands expose each step on its own for debugging:

- `download`
- `extract`
- `import`
- `provision`

`provision` writes the first-boot account data into the host share:

- your username (host `$USER` by default),
- your ssh *public* key (auto-detected from `~/.ssh/*.pub`),
- and the autologin flag.

Only public material crosses the share; the guest applies it on first boot and
clears it. This is the same writer `up` calls, and the same one
`just tart up-provisioned` drives for the local build loop.

## Running it

The recipes wrap `cargo` so the crate is driven like the rest of the repo:

- **`run`** builds and runs in debug — fine for small commands like `provision`.
- **`run-release`** uses the optimised binary — real work (`up`, `extract`)
  decompresses a multi-GiB disk, which is slow in a debug build.
- **`package`** wraps `bin/package-cli.sh` to produce the Homebrew tarball
  (`output/*.tar.gz` + `.sha256`). The release workflow runs that same script,
  so a local package matches a released one.

`check` is the crate's gate — `fmt --check`, `clippy -D warnings`, and tests —
and the pre-commit hook runs it, so it defines what a clean crate means.

## How it's built

`src/core/` is UI-agnostic; `src/main.rs` is the clap front-end. The split lets
a future ratatui TUI drive the same core — customising the account, ssh key,
autologin, and resources interactively — without rewriting the operations.
