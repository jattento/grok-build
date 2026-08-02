#!/usr/bin/env bash
# Report the fork's delta against upstream and flag undocumented touchpoints.
#
# A touchpoint is any file outside overlay/ that differs from upstream/main.
# Each one must have an entry in overlay/TOUCHPOINTS.md, written as a heading
# containing the path in backticks.
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

base=${1:-upstream/main}
git rev-parse --verify --quiet "$base" >/dev/null || {
  echo "error: '$base' not found. Run: git fetch upstream" >&2
  exit 1
}

# Portable to macOS's bash 3.2, which has no `mapfile`.
changed=()
while IFS= read -r line; do
  changed+=("$line")
done < <(git diff --name-only "$base"...HEAD -- . ':!overlay' ':!AGENTS.md')

echo "== overlay files (ours, conflict-free) =="
git diff --stat "$base"...HEAD -- overlay AGENTS.md | tail -n 1
echo
echo "== touchpoints in upstream files =="
if [ ${#changed[@]} -eq 0 ]; then
  echo "(none)"
  exit 0
fi

undocumented=0
for file in "${changed[@]}"; do
  lines=$(git diff --numstat "$base"...HEAD -- "$file" | awk '{print "+"$1" -"$2}')
  if grep -qF "\`$file\`" overlay/TOUCHPOINTS.md; then
    printf '  %-70s %s\n' "$file" "$lines"
  else
    printf '  %-70s %s  [UNDOCUMENTED]\n' "$file" "$lines"
    undocumented=1
  fi
done

if [ "$undocumented" -eq 1 ]; then
  echo
  echo "error: add the files marked UNDOCUMENTED to overlay/TOUCHPOINTS.md" >&2
  exit 1
fi
