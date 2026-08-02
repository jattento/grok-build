#!/usr/bin/env bash
# Absorb a new upstream snapshot: refresh the mirror branch, tag a rollback
# point, and rebase our delta on top.
#
# Topology: `main` carries our commits, `upstream` mirrors xai-org/main exactly.
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

if [ -n "$(git status --porcelain)" ]; then
  echo "error: working tree is dirty. Commit or stash first." >&2
  exit 1
fi

branch=$(git rev-parse --abbrev-ref HEAD)
if [ "$branch" != "main" ]; then
  echo "error: run this from main (currently on '$branch')." >&2
  exit 1
fi

git remote get-url upstream >/dev/null 2>&1 ||
  git remote add upstream https://github.com/xai-org/grok-build.git

git config rerere.enabled true
git fetch upstream --tags

old=$(git rev-parse upstream/main@{1} 2>/dev/null || echo "")
new=$(git rev-parse upstream/main)
if git merge-base --is-ancestor "$new" HEAD; then
  echo "Already on top of upstream/main ($(git rev-parse --short "$new")). Nothing to sync."
  git branch -f upstream upstream/main
  exit 0
fi

tag="pre-sync/$(date +%Y-%m-%d)"
suffix=1
while git rev-parse --verify --quiet "refs/tags/$tag" >/dev/null; do
  suffix=$((suffix + 1))
  tag="pre-sync/$(date +%Y-%m-%d).$suffix"
done
git tag "$tag" main
echo "Rollback point: $tag -> $(git rev-parse --short main)"

git branch -f upstream upstream/main
echo "Mirror branch 'upstream' now at $(git rev-parse --short upstream/main)"
[ -n "$old" ] && echo "Upstream moved: $(git rev-parse --short "$old") -> $(git rev-parse --short "$new")"

echo
echo "Rebasing our delta onto upstream/main..."
if ! git rebase upstream/main; then
  cat >&2 <<'MSG'

Rebase stopped on a conflict. Resolve it, then `git rebase --continue`.
To abort entirely: `git rebase --abort` (main is also saved at the tag above).
MSG
  exit 1
fi

cat <<MSG

Rebase clean. Now verify, from overlay/TOUCHPOINTS.md:
  cargo check -p overlay-core && cargo test -p overlay-core
  cargo check -p xai-grok-pager-bin
  cargo build -p xai-grok-pager-bin
  overlay/scripts/overlay-diff.sh

Then publish the rewritten history:
  git push --force-with-lease origin main
  git push origin upstream
MSG
