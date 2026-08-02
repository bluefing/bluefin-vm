# Docs migration checklist

Tracks the move from the old flat `docs/*.md` layout to the Zensical site
(`docs/content/` published, `docs/internal/` not). Each item is enough context to
pick up cold.

## Done (this branch)

- Site scaffold: `zensical.toml`, `pyproject.toml`, `.gitignore` at `docs/`;
  `content/` and `internal/` trees created.
- `README.md` essence → `content/index.md` + `content/getting-started/install.md`
  (README kept at repo root as the GitHub front page).
- `docs/modules/{build,cli,tart}.md` → `content/just/*.md`, still served by
  `just <module> help` via the repointed `.just/*/help.md` symlinks.
- `docs/modules/root.md` → `content/reference/repo-structure.md` (root `help.md`
  symlink repointed).
- `docs/DESIGN.md` → `internal/design/access.md`.
- New: `internal/requirements/problem-statement.md`, `.../open-questions.md`
  (captures the no-extra-framework decision + the open sudo / hook questions).

## To migrate (prose-heavy — incremental)

- **`docs/PROVISIONING.md`** → split: audience-facing how-it-works into
  `content/guide/provisioning.md` (currently a stub); decision rationale folded
  into `internal/design/access.md`. Reframe disposable → daily-driver in the
  process. Blocked on the drop-autologin realignment landing so the prose matches
  the code.
- **`docs/diagrams/*.md`** → `content/guide/architecture.md` (currently a stub).
  Verify Zensical/Material renders the mermaid blocks; reframe autologin/disposable
  language.
- **`docs/ROADMAP.md`** → fold decided items into `open-questions.md`; live stories
  into `internal/planning/backlog.md`. Note it still narrates the pre-realignment
  autologin/passwordless model as current — supersede that against
  `../design/access.md` when folding.
- **`docs/BACKLOG.md`** → merge into `internal/planning/backlog.md` (which now
  exists, holding the TUI/CLI items from design review); fix its stale
  `docs/modules/*` path references, and drop the delivered autologin/gdm items,
  as part of the move.
- Consider `internal/requirements/cross-cutting.md` (testing/lint/CI conventions)
  if it earns its place.

## Wiring still to do

- Update any remaining `docs/modules/*` / `docs/PROVISIONING.md` references once
  those files move (grep before deleting the old paths).
- Decide publishing (currently local-only, no CI): add a Pages workflow when the
  repo goes public again.
- Verify a local build: `cd docs && uv run zensical build` (uv/zensical not yet
  installed on this host).
