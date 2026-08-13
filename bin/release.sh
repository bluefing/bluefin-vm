#!/usr/bin/env bash
set -euo pipefail

# Tag a release and push the tag. Nothing else -- this script never commits,
# bumps a version, moves a branch, or force-pushes. The crate bump reaches
# main through a PR, so the checks below refuse to tag when it has not.
#
# Pushing the tag is what starts .github/workflows/release.yml, which builds
# the binary, publishes the Release, and pushes the formula bump to the
# Homebrew tap. Tags matching v* are protected against updates and deletion,
# so a wrong tag is cut again as a new version rather than moved.
# Assumes CWD = repo root.

dryrun="" # -n: print the commands instead of running them

usage() {
  cat <<EOF
Usage: $(basename "$0") [-n] [-h] VERSION

Tag vVERSION on main and push it, which starts the release workflow.

Options:
  -n          Dry run: print the commands without executing them.
  -h          Show this help and exit.

Refuses unless, in order:
  1. VERSION is bare semver X.Y.Z.
  2. The current branch is main -- releases are cut from trunk.
  3. The working tree is clean.
  4. main is in sync with origin/main, so the tag names the commit
     consumers fetch.
  5. cli/Cargo.toml declares VERSION, which the workflow checks again.
  6. Tag vVERSION does not exist locally or on origin.
EOF
}

die() {
  echo "Error: $1" >&2
  exit 1
}

# Print the command instead of running it under -n, so a dry run exercises
# every check and stops only at the two mutating calls.
run_cmd() {
  if [ -n "$dryrun" ]; then
    printf '+ %s\n' "$*"
  else
    "$@"
  fi
}

while getopts ":nh" opt; do
  case "$opt" in
    n) dryrun=1 ;;
    h)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 1
      ;;
  esac
done
shift $((OPTIND - 1))

version="${1:-}"
[ -n "$version" ] || {
  usage >&2
  exit 1
}
tag="v${version}"

# Each part is 0 or a number without a leading zero, as semver requires.
[[ $version =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]] ||
  die "'$version' is not a bare semver version (X.Y.Z)"

branch="$(git branch --show-current)"
[ "$branch" = "main" ] ||
  die "releases are cut from main (currently on '$branch')"

# Tracked changes only: a tag names a commit, so untracked files are none of
# this script's business.
if ! git diff --quiet || ! git diff --cached --quiet; then
  die "the working tree has uncommitted changes"
fi

# Fetch first: being on main by name is not the same as being at the commit
# origin serves, and the tag must name what consumers fetch.
git fetch --quiet origin main
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] ||
  die "main and origin/main differ -- push or pull before tagging"

crate="$(sed -n 's/^version = "\(.*\)"/\1/p' cli/Cargo.toml | head -n1)"
[ "$crate" = "$version" ] ||
  die "cli/Cargo.toml declares $crate, not $version -- bump it through a PR first"

git rev-parse -q --verify "refs/tags/$tag" >/dev/null &&
  die "tag $tag already exists locally"
[ -z "$(git ls-remote --tags origin "refs/tags/$tag")" ] ||
  die "tag $tag already exists on origin"

echo ">> tagging $tag at $(git rev-parse --short HEAD) on main"
run_cmd git tag "$tag"
run_cmd git push origin "$tag"
echo "OK: $tag pushed -- release.yml takes it from here"
