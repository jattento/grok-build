#!/usr/bin/env bash
# Install a pre-push hook that runs overlay/scripts/overlay-diff.sh.
#
# Safe for git worktrees: resolves the common git dir so the hook lands in the
# shared hooks directory. Idempotent: re-running refreshes our hook in place.
# Will not clobber an unrelated pre-push hook without saying so.
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

# Worktrees share hooks via the common git directory.
git_common=$(git rev-parse --git-common-dir)
# Resolve relative common-dir paths against repo root / git-dir.
case "$git_common" in
  /*) ;;
  *) git_common="$repo_root/$git_common" ;;
esac

hooks_dir="$git_common/hooks"
mkdir -p "$hooks_dir"
hook_path="$hooks_dir/pre-push"

marker="# coinor/grok-build overlay-diff pre-push hook"

hook_body=$(cat <<'HOOK'
#!/usr/bin/env bash
# coinor/grok-build overlay-diff pre-push hook
# Installed by overlay/scripts/install-hooks.sh — safe to re-run that script.
set -euo pipefail

# Resolve the worktree / repo root for this push (not the common dir).
repo_root=$(git rev-parse --show-toplevel)
script="$repo_root/overlay/scripts/overlay-diff.sh"

if [ ! -x "$script" ] && [ ! -f "$script" ]; then
  echo "overlay pre-push: missing $script — skipping gate" >&2
  exit 0
fi

echo "overlay pre-push: running overlay/scripts/overlay-diff.sh"
if ! bash "$script"; then
  echo >&2
  echo "overlay pre-push: blocked. Fix the gates above, then push again." >&2
  echo "  emergency bypass (use sparingly): git push --no-verify" >&2
  exit 1
fi
HOOK
)

if [ -f "$hook_path" ] || [ -L "$hook_path" ]; then
  if grep -qF "$marker" "$hook_path" 2>/dev/null || grep -qF "coinor/grok-build overlay-diff pre-push hook" "$hook_path" 2>/dev/null; then
    printf '%s\n' "$hook_body" > "$hook_path"
    chmod +x "$hook_path"
    echo "updated existing overlay pre-push hook:"
    echo "  $hook_path"
  else
    echo "error: $hook_path already exists and is not the overlay-diff hook." >&2
    echo "  Inspect it, then either remove it or merge the overlay check by hand." >&2
    echo "  Refusing to clobber an unrelated hook." >&2
    exit 1
  fi
else
  printf '%s\n' "$hook_body" > "$hook_path"
  chmod +x "$hook_path"
  echo "installed pre-push hook:"
  echo "  $hook_path"
fi

echo
echo "The hook runs overlay/scripts/overlay-diff.sh before every push and"
echo "blocks on Gate 1/2/3 failure."
echo "Emergency bypass: git push --no-verify"
echo "Re-run this script any time; it is idempotent."
