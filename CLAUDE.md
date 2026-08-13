# CLAUDE.md — bluefin-vm

Agent-facing context. **What the project is and how to install it live in
[README.md](README.md); how the build works, which image to use, and the build
gotchas live in [docs/content/just/build.md](docs/content/just/build.md) (`just build
help`).** Read them; don't restate here. This file holds the guardrails, a
pointer to the file map, and the style mandate.

## Guardrails (don't regress — docs/content/just/build.md explains the why of each)

- Image: always an `*-arm64` / `lts` tag (amd64 forces slow emulation).
  *Which* tag currently boots changes — see docs/content/just/build.md.
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

Each file documents itself in its own header, so read the file for its scope,
purpose, and rationale. [docs/content/reference/repo-structure.md](docs/content/reference/repo-structure.md) (`just
help`) is only the map: what lives where and how the pieces relate.

## Verify a change

- Fast: `just test` (bats + cli tests) + `just lint` (pre-commit).
  `bin/build-disk.sh -n` inspects build commands without running them.
- A built disk's boot isn't checked by `just test` — confirm in a VM:
  `just tart up` (or `up-patched` for the derived image).

## Style mandate (docs, comments, responses)

Write plainly and durably. Say the essential thing once, then stop.

- **Verify; don't assume.** Every claim must be true and checked against the
  code or reality before you write it — treat prior text (and your own
  rephrasing) as unverified until confirmed. Never write what merely sounds
  right (e.g. don't call VM-specific glue an "upstream fix").
- **Cut what will age.** No "currently / today / for now" snapshots of upstream
  state, no justifying a volatile choice. State what's durable; keep operational
  current-state only where it's used (the build guide), never in strategy docs
  or config comments.
- **No cross-references unless a reader can't proceed without one.** No
  section-title quotes, no BL numbers, no "see docs/X". Each file documents
  itself — state the fact where it belongs instead of signposting a fuller
  version. Naming a mechanism (a recipe, variable, unit) is fine.
- **Human voice, full sentences.** Open with a subject and verb, not a headline
  fragment. Ban the "X, not Y" antithesis, the phrase "single source of truth",
  and spelling out the obvious. If a line reads like AI, cut or rewrite it.
- **Comments say why, not what** — and only where the code isn't self-evident.
- **Lead with scope, then purpose, then detail** when explaining a file or recipe.
- **Record decisions, not options;** mark guesses and preferences as proposals.
- **Impersonal, public repo:** no personal names, chat quotes, or session
  narrative in docs or commits.
- **British spelling** (customise, colour, behaviour).
- **Shell naming:** uppercase for environment variables only; script-local
  variables lowercase (usage metavars like `-i IMAGE` stay uppercase).

## Commit messages

The subject is a conventional commit (commitlint enforces the format). The
body opens with a sentence or two of why; when the change has several parts,
list them as bullets rather than chaining clauses — colon-spliced prose is
hard to parse. Aim between overexplained and telegraphic: enough that a
reader six months out understands the change without the diff, and nothing
they could infer from it.
