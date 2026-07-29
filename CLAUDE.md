# CLAUDE.md — bluefin-vm

Agent-facing context. **What the project is and how to install it live in
[README.md](README.md); how the build works, which image to use, and the build
gotchas live in [docs/BUILDING.md](docs/BUILDING.md)** — the single sources of
truth. Read them; don't restate here. This file holds only the guardrails, a
file map pointer, and how to respond.

## Guardrails (don't regress — docs/BUILDING.md explains the why of each)

- Image: always an `*-arm64` / `lts` tag (amd64 forces slow emulation).
  *Which* tag currently boots changes — see docs/BUILDING.md "Which image".
- Never pass `--target-arch`; there is no `--local` flag. Build ARM on an
  ARM host.
- Keep `config.toml` (20 GiB root, auto-read at `/config.toml`) — without it
  the build fails on ostree `min-free-space`.
- Every build is two-step: pull the image into container storage, then
  build. `localhost/` images skip the pull (store-only).
- `config.toml` owns disk-build concerns only; everything OS-side belongs in
  `image/Containerfile`.
- Scripts assume CWD = repo root; invoke them from root, as the recipes do.

## Files

File map + per-file reference: [docs/FILES.md](docs/FILES.md).

## Verify a change

- Fast: `just test` (bats + cli tests) + `just lint` (pre-commit).
  `bin/build-disk.sh -n` inspects build commands without running them.
- A built disk's boot isn't checked by `just test` — confirm in a VM:
  `just tart up` (or `up-patched` for the derived image).

## Working style (how to respond)

- Explaining a file/recipe: lead with **scope** (universal, or specific to
  the build step / the runtime / the cli tool / one format?), then **purpose**
  (one line), then detail only if needed.
- Calibrate the prose: clear and complete — neither bloated nor cryptic
  shorthand.
- Comments in scripts/config say **why, not what** — and only where the code
  doesn't make it clear. Each explanation lives in one place (usually the
  README); elsewhere, point to it.
- Shell: uppercase names are for environment variables only; script-local
  variables are lowercase. (Usage-text metavars like `-i IMAGE` stay
  conventional uppercase.)
- Never restate a single source of truth in docs — it drifts. A script's
  arguments live in its `-h`/getopts, build constraints in the tool that
  enforces them, defaults in the porcelain. Docs describe scope, purpose,
  behaviour, and *why*; they point at the source, never duplicate it.
- Record only what was actually decided; mark guesses and preferences as
  proposals, not decisions.
- Public repo: no personal names, chat quotes, or session narrative in docs
  or commits — state decisions and findings impersonally.
- British spelling (customise, colour, behaviour).
