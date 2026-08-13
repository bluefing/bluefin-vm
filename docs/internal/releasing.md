# Releasing

How a `bluefin-vm` version reaches users. Two artefacts ship independently:

- **The tool** — a pre-built arm64 binary attached to a GitHub Release, which
  the Homebrew tap's formula points at. That is what this page covers.
- **The disk image** — built by `build-arm-image.yml` and published to R2 under
  a stable name. It carries no version and no tag. Releasing the tool does not
  rebuild it. When the image layer changes, dispatch that workflow.

## One-time setup

`release.yml` pushes to `bluefing/homebrew-tap`, which the job's own token
cannot write to, so it needs a personal access token:

1. GitHub → Settings → Developer settings → Personal access tokens →
   Fine-grained tokens → Generate new token.
2. Resource owner `bluefing`, repository access **only** `homebrew-tap`,
   permission **Contents: read and write**. Nothing else.
3. In `bluefin-vm` → Settings → Secrets and variables → Actions → New
   repository secret, name it `TAP_PUSH_TOKEN`.

Rotate it by repeating those steps. The workflow reads the secret by name, so
nothing else changes.

## Cutting a release

Versions follow semver, pre-1.0, so a breaking change is a minor bump.

1. **Bump the crate.** Set `version` in `cli/Cargo.toml`, build once so
   `Cargo.lock` follows, and open a PR. `main` is protected, so this is the
   only way in. The tag is checked against this value, and a mismatch fails
   the release.
2. **Tag the merge commit.** On an up-to-date `main`:

        just release X.Y.Z

   The recipe refuses unless the branch is `main`, the tree is clean, `main`
   matches `origin/main`, `cli/Cargo.toml` declares that version, and the tag
   is unused. Add `-n` to run every check and stop before tagging. Tags
   matching `v*` are protected against updates and deletion, so a typo means
   cutting a new version rather than moving the tag.
3. **Wait for `release.yml`.** It builds on an Apple Silicon runner, publishes
   the Release with the tarball and its `.sha256`, then rewrites the tap
   formula's `url` and `sha256` and pushes. The run summary carries both
   values, ready to paste if the tap step fails.
4. **Write the release notes.** The workflow leaves a one-line placeholder
   describing the artefact. In the repo's Releases page, open the new tag and
   edit its body to say what changed, leading with anything breaking.
5. **Verify as a user.**

        brew update && brew upgrade bluefin-vm
        bluefin-vm --version

## When it goes wrong

- **Release fails on the version guard.** The tag and `cli/Cargo.toml`
  disagree. Fix the crate version through a PR and tag the new merge commit.
  The bad tag stays where it is.
- **The tap step fails.** The Release itself is already published and valid.
  Copy the `url` and `sha256` from the run summary into
  `Formula/bluefin-vm.rb` and push the tap by hand, then fix the cause. It is
  usually an expired `TAP_PUSH_TOKEN`, or a formula that no longer has a `url`
  or `sha256` line for the workflow to rewrite.
- **`brew upgrade` reports a 404.** The formula points at a release asset that
  is not public. Check the repository's visibility, and that the asset name in
  the formula matches the one on the Release.
