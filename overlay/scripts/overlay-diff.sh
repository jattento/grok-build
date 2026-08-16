#!/usr/bin/env bash
# Report the fork's delta against upstream and enforce the overlay gates.
#
# Distinguishes two kinds of delta against the base (default: upstream/main):
#
#   A  whole files we authored. Must live under overlay/. Outside overlay/
#      the only exception is a sanctioned adapter that satisfies three
#      conditions together (see overlay/adapters-outside-overlay.txt):
#        1. basename starts with overlay_
#        2. path is listed in adapters-outside-overlay.txt
#        3. path has a line budget in delta-budget.tsv
#   M  upstream files we modified. Real merge cost. Each must be documented
#      in overlay/TOUCHPOINTS.md and stay within overlay/delta-budget.tsv.
#
# The line budget covers every M file plus every sanctioned adapter (A).
#
# AGENTS.md is excluded: it is fork policy that lives at the repo root by
# convention, not an upstream touchpoint and not crates/ code.
#
# Portable to macOS's bash 3.2 (no mapfile, no associative arrays).
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: overlay/scripts/overlay-diff.sh [options] [base]

Report the fork's delta against base (default: upstream/main) and enforce:

  Gate 1  Whole files outside overlay/ only as sanctioned adapters
          (basename overlay_*, listed, budgeted)
  Gate 2  Every modified upstream file is documented in TOUCHPOINTS.md
  Gate 3  Per-file and total changed-line budgets in delta-budget.tsv
          (M files + sanctioned adapters)

Options:
  --update-budget           Rewrite overlay/delta-budget.tsv from the current
                            tree. Refuses to raise any file or the total.
  --allow-growth <reason>   With --update-budget, permit raising budgets.
                            Appends reason and date as a comment in the file.
  -h, --help                Show this help.

Examples:
  overlay/scripts/overlay-diff.sh
  overlay/scripts/overlay-diff.sh --update-budget
  overlay/scripts/overlay-diff.sh --update-budget --allow-growth "theme migration"
EOF
}

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

update_budget=0
allow_growth=0
growth_reason=""
base=""

while [ $# -gt 0 ]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    --update-budget)
      update_budget=1
      shift
      ;;
    --allow-growth)
      if [ $# -lt 2 ]; then
        echo "error: --allow-growth requires a reason string" >&2
        echo "  example: --update-budget --allow-growth \"shrink theme via overlay package\"" >&2
        exit 2
      fi
      allow_growth=1
      growth_reason=$2
      shift 2
      ;;
    --allow-growth=*)
      allow_growth=1
      growth_reason=${1#--allow-growth=}
      if [ -z "$growth_reason" ]; then
        echo "error: --allow-growth requires a non-empty reason" >&2
        exit 2
      fi
      shift
      ;;
    --)
      shift
      break
      ;;
    -*)
      echo "error: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      if [ -n "$base" ]; then
        echo "error: unexpected argument: $1" >&2
        usage >&2
        exit 2
      fi
      base=$1
      shift
      ;;
  esac
done

if [ "$allow_growth" -eq 1 ] && [ "$update_budget" -eq 0 ]; then
  echo "error: --allow-growth only makes sense with --update-budget" >&2
  exit 2
fi

base=${base:-upstream/main}
git rev-parse --verify --quiet "$base" >/dev/null || {
  echo "error: '$base' not found (local ref missing)." >&2
  echo "  This repo expects a local upstream/main. Do not invent a remote fetch;" >&2
  echo "  use the existing ref or pass an explicit base." >&2
  exit 1
}

adapters_file=overlay/adapters-outside-overlay.txt
budget_file=overlay/delta-budget.tsv
touchpoints_file=overlay/TOUCHPOINTS.md

# Load sanctioned adapter paths (one per line; # comments and blanks ignored).
adapter_paths=()
if [ -f "$adapters_file" ]; then
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
      ''|\#*) continue ;;
    esac
    line=${line%$'\r'}
    adapter_paths+=("$line")
  done < "$adapters_file"
fi

in_adapters() {
  _p=$1
  _i=0
  while [ "$_i" -lt "${#adapter_paths[@]}" ]; do
    if [ "${adapter_paths[$_i]}" = "$_p" ]; then
      return 0
    fi
    _i=$((_i + 1))
  done
  return 1
}

# Basename starts with overlay_ (before any extension).
is_overlay_named() {
  _base=${1##*/}
  case "$_base" in
    overlay_*) return 0 ;;
    *) return 1 ;;
  esac
}

# Load budget: parallel arrays budget_paths / budget_lines, plus budget_total.
budget_paths=()
budget_lines=()
budget_total=-1
budget_loaded=0

load_budget() {
  budget_paths=()
  budget_lines=()
  budget_total=-1
  budget_loaded=0
  if [ ! -f "$budget_file" ]; then
    return 0
  fi
  budget_loaded=1
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
      ''|\#*) continue ;;
    esac
    line=${line%$'\r'}
    b_lines=${line%%$'\t'*}
    b_path=${line#*$'\t'}
    if [ "$b_lines" = "$line" ] || [ -z "$b_path" ]; then
      echo "error: malformed budget line (want <lines><TAB><path>): $line" >&2
      exit 1
    fi
    case "$b_lines" in
      *[!0-9]*)
        echo "error: non-integer budget count in $budget_file: $line" >&2
        exit 1
        ;;
    esac
    if [ "$b_path" = "TOTAL" ]; then
      budget_total=$b_lines
      continue
    fi
    budget_paths+=("$b_path")
    budget_lines+=("$b_lines")
  done < "$budget_file"
}

budget_for() {
  _p=$1
  _i=0
  while [ "$_i" -lt "${#budget_paths[@]}" ]; do
    if [ "${budget_paths[$_i]}" = "$_p" ]; then
      echo "${budget_lines[$_i]}"
      return 0
    fi
    _i=$((_i + 1))
  done
  return 1
}

# Collect delta outside overlay/ (and outside AGENTS.md).
a_paths=()
a_lines=()
m_paths=()
m_lines=()
m_total_lines=0
a_total_lines=0
a_count=0
m_count=0

while IFS= read -r line || [ -n "$line" ]; do
  [ -z "$line" ] && continue
  st=${line%%$'\t'*}
  path=${line##*$'\t'}
  [ "$path" = "AGENTS.md" ] && continue
  case "$path" in
    overlay|overlay/*) continue ;;
  esac

  code=${st:0:1}
  ns=$(git diff --numstat "$base"...HEAD -- "$path" | head -n 1 || true)
  if [ -z "$ns" ]; then
    added=0
    deleted=0
  else
    added=${ns%%$'\t'*}
    rest=${ns#*$'\t'}
    deleted=${rest%%$'\t'*}
    case "$added" in *[!0-9]*) added=0 ;; esac
    case "$deleted" in *[!0-9]*) deleted=0 ;; esac
  fi
  changed=$((added + deleted))

  if [ "$code" = "A" ]; then
    a_paths+=("$path")
    a_lines+=("$changed")
    a_count=$((a_count + 1))
    a_total_lines=$((a_total_lines + changed))
  else
    m_paths+=("$path")
    m_lines+=("$changed")
    m_count=$((m_count + 1))
    m_total_lines=$((m_total_lines + changed))
  fi
done < <(git diff --name-status "$base"...HEAD -- . ':!overlay' ':!AGENTS.md')

# Budgeted set for totals / --update-budget: every M + every A currently in
# the delta (adapters are A). TOTAL = sum of those line counts.
budgeted_total_lines=$((m_total_lines + a_total_lines))

# ---- --update-budget mode -------------------------------------------------
if [ "$update_budget" -eq 1 ]; then
  load_budget

  grew=0
  growth_report=""

  if [ "$budget_loaded" -eq 1 ]; then
    i=0
    while [ "$i" -lt "$m_count" ]; do
      path=${m_paths[$i]}
      cur=${m_lines[$i]}
      if old=$(budget_for "$path"); then
        if [ "$cur" -gt "$old" ]; then
          delta=$((cur - old))
          growth_report="${growth_report}  ${path}: ${cur} > ${old} (+${delta})"$'\n'
          grew=1
        fi
      fi
      i=$((i + 1))
    done
    i=0
    while [ "$i" -lt "$a_count" ]; do
      path=${a_paths[$i]}
      cur=${a_lines[$i]}
      if old=$(budget_for "$path"); then
        if [ "$cur" -gt "$old" ]; then
          delta=$((cur - old))
          growth_report="${growth_report}  ${path}: ${cur} > ${old} (+${delta})"$'\n'
          grew=1
        fi
      fi
      i=$((i + 1))
    done
    if [ "$budget_total" -ge 0 ] && [ "$budgeted_total_lines" -gt "$budget_total" ]; then
      delta=$((budgeted_total_lines - budget_total))
      growth_report="${growth_report}  TOTAL: ${budgeted_total_lines} > ${budget_total} (+${delta})"$'\n'
      grew=1
    fi
  fi

  if [ "$grew" -eq 1 ] && [ "$allow_growth" -eq 0 ]; then
    echo "error: --update-budget refuses to raise any budget number." >&2
    echo >&2
    echo "These entries grew:" >&2
    printf '%s' "$growth_report" >&2
    echo >&2
    echo "Either shrink the delta back under budget, or re-run with:" >&2
    echo "  overlay/scripts/overlay-diff.sh --update-budget --allow-growth \"<why this growth is unavoidable>\"" >&2
    exit 1
  fi

  tmp=$(mktemp)
  {
    echo "# overlay/delta-budget.tsv"
    echo "#"
    echo "# Per-file changed-line budget for the costly delta against upstream:"
    echo "#   - every modified upstream file (git status M)"
    echo "#   - every whole file outside overlay/ (git status A), including"
    echo "#     sanctioned adapters listed in adapters-outside-overlay.txt"
    echo "# changed-lines = added + deleted from: git diff --numstat <base>...HEAD"
    echo "# Sorted by path. TOTAL is the sum of the file budgets."
    echo "#"
    echo "# The budget only ratchets down. overlay-diff.sh --update-budget rewrites"
    echo "# this file from the current tree but refuses to raise any number unless"
    echo "# you pass --allow-growth \"<reason>\", which appends a dated comment."
    echo "#"
    if [ "$allow_growth" -eq 1 ] && [ "$grew" -eq 1 ]; then
      echo "# GROWTH $(date +%Y-%m-%d): $growth_reason"
      printf '%s' "$growth_report" | while IFS= read -r gl || [ -n "$gl" ]; do
        [ -z "$gl" ] && continue
        echo "#   $gl"
      done
      echo "#"
    fi
    echo "# Format: <changed-lines><TAB><path>"
    echo "#"

    {
      i=0
      while [ "$i" -lt "$m_count" ]; do
        printf '%s\t%s\n' "${m_lines[$i]}" "${m_paths[$i]}"
        i=$((i + 1))
      done
      i=0
      while [ "$i" -lt "$a_count" ]; do
        printf '%s\t%s\n' "${a_lines[$i]}" "${a_paths[$i]}"
        i=$((i + 1))
      done
    } | LC_ALL=C sort -t "$(printf '\t')" -k2,2

    printf '%s\t%s\n' "$budgeted_total_lines" "TOTAL"
  } > "$tmp"

  mv "$tmp" "$budget_file"
  echo "wrote $budget_file"
  echo "  M files: $m_count"
  echo "  A files outside overlay/: $a_count"
  echo "  total changed lines (M+A): $budgeted_total_lines"
  if [ "$allow_growth" -eq 1 ] && [ "$grew" -eq 1 ]; then
    echo "  growth allowed: $growth_reason"
  elif [ "$grew" -eq 0 ]; then
    echo "  no budget number increased"
  fi
  exit 0
fi

# ---- normal gate mode -----------------------------------------------------

load_budget

echo "== overlay / AGENTS.md (ours, conflict-free) =="
stat_line=$(git diff --stat "$base"...HEAD -- overlay AGENTS.md | tail -n 1)
if [ -n "$stat_line" ]; then
  echo "$stat_line"
else
  echo "(no changes)"
fi
echo

# Gate 1: whole files outside overlay/ — only sanctioned adapters
echo "== Gate 1: whole files outside overlay/ (status A; sanctioned adapters only) =="
gate1_fail=0
if [ "$a_count" -eq 0 ]; then
  echo "(none)"
else
  i=0
  while [ "$i" -lt "$a_count" ]; do
    path=${a_paths[$i]}
    lines=${a_lines[$i]}
    base_name=${path##*/}

    if ! is_overlay_named "$path"; then
      printf '  %-70s +%s  [WRONG NAME]\n' "$path" "$lines"
      echo "      basename must start with overlay_ (got: $base_name)"
      echo "      rename to overlay_<role>.rs and put real logic in an overlay-* crate;"
      echo "      listing a non-overlay_* path in $adapters_file is not enough"
      gate1_fail=1
    elif ! in_adapters "$path"; then
      printf '  %-70s +%s  [NOT LISTED]\n' "$path" "$lines"
      echo "      add this path to $adapters_file only if it is a thin adapter that"
      echo "      cannot leave the upstream module tree (closed match / private types);"
      echo "      otherwise move the file under overlay/"
      gate1_fail=1
    elif [ "$budget_loaded" -eq 0 ] || ! budget_for "$path" >/dev/null; then
      printf '  %-70s +%s  [NO BUDGET]\n' "$path" "$lines"
      echo "      sanctioned adapters need a line budget so they cannot grow silently:"
      echo "        overlay/scripts/overlay-diff.sh --update-budget"
      gate1_fail=1
    else
      old=$(budget_for "$path")
      if [ "$lines" -gt "$old" ]; then
        delta=$((lines - old))
        printf '  %-70s +%s  [SANCTIONED, OVER BUDGET: %s > %s (+%s)]\n' \
          "$path" "$lines" "$lines" "$old" "$delta"
        echo "      shrink the adapter, or raise deliberately:"
        echo "        overlay/scripts/overlay-diff.sh --update-budget --allow-growth \"<reason>\""
        gate1_fail=1
      else
        headroom=$((old - lines))
        printf '  %-70s +%s  [SANCTIONED] budget %s / %s (headroom %s)\n' \
          "$path" "$lines" "$lines" "$old" "$headroom"
      fi
    fi
    i=$((i + 1))
  done
fi

# Listed adapters that no longer appear as A must be removed (list must not rot).
i=0
while [ "$i" -lt "${#adapter_paths[@]}" ]; do
  ap=${adapter_paths[$i]}
  still=0
  j=0
  while [ "$j" -lt "$a_count" ]; do
    if [ "${a_paths[$j]}" = "$ap" ]; then
      still=1
      break
    fi
    j=$((j + 1))
  done
  if [ "$still" -eq 0 ]; then
    printf '  %-70s  [STALE — remove this line from %s]\n' "$ap" "$adapters_file"
    echo "      listed adapters that left the delta must be deleted from the list"
    gate1_fail=1
  fi
  i=$((i + 1))
done
echo

# Gate 2: every M file documented in TOUCHPOINTS.md
echo "== Gate 2: modified upstream files (status M) must be in TOUCHPOINTS.md =="
gate2_fail=0
if [ "$m_count" -eq 0 ]; then
  echo "(none)"
else
  if [ ! -f "$touchpoints_file" ]; then
    echo "error: missing $touchpoints_file" >&2
    gate2_fail=1
  fi
  i=0
  while [ "$i" -lt "$m_count" ]; do
    path=${m_paths[$i]}
    lines=${m_lines[$i]}
    if [ -f "$touchpoints_file" ] && grep -qF "\`$path\`" "$touchpoints_file"; then
      printf '  %-70s %s\n' "$path" "$lines"
    else
      printf '  %-70s %s  [UNDOCUMENTED]\n' "$path" "$lines"
      gate2_fail=1
    fi
    i=$((i + 1))
  done
fi
echo

# Gate 3: per-file and total line budget (M + A)
echo "== Gate 3: changed-line budget (overlay/delta-budget.tsv; M + adapters) =="
gate3_fail=0
if [ "$budget_loaded" -eq 0 ]; then
  echo "error: missing $budget_file" >&2
  echo "  generate the initial budget from today's tree:" >&2
  echo "    overlay/scripts/overlay-diff.sh --update-budget" >&2
  gate3_fail=1
else
  i=0
  while [ "$i" -lt "$m_count" ]; do
    path=${m_paths[$i]}
    cur=${m_lines[$i]}
    if old=$(budget_for "$path"); then
      if [ "$cur" -gt "$old" ]; then
        delta=$((cur - old))
        printf '  %s: %s > %s (+%s)  [OVER BUDGET]\n' "$path" "$cur" "$old" "$delta"
        echo "      shrink the edit, or if growth is unavoidable:"
        echo "        overlay/scripts/overlay-diff.sh --update-budget --allow-growth \"<reason>\""
        gate3_fail=1
      else
        headroom=$((old - cur))
        printf '  %-70s %s / %s  (headroom %s)\n' "$path" "$cur" "$old" "$headroom"
      fi
    else
      printf '  %-70s %s  [NO BUDGET ENTRY]\n' "$path" "$cur"
      echo "      new touchpoints need a deliberate budget line:"
      echo "        overlay/scripts/overlay-diff.sh --update-budget"
      echo "      (and document the file in $touchpoints_file)"
      gate3_fail=1
    fi
    i=$((i + 1))
  done

  # A paths also appear in the budget; Gate 1 already flagged missing/over for
  # adapters, but still print their rows here so the budget view is complete.
  i=0
  while [ "$i" -lt "$a_count" ]; do
    path=${a_paths[$i]}
    cur=${a_lines[$i]}
    if old=$(budget_for "$path"); then
      if [ "$cur" -gt "$old" ]; then
        delta=$((cur - old))
        printf '  %s: %s > %s (+%s)  [OVER BUDGET]\n' "$path" "$cur" "$old" "$delta"
        gate3_fail=1
      else
        headroom=$((old - cur))
        printf '  %-70s %s / %s  (headroom %s)  [adapter]\n' "$path" "$cur" "$old" "$headroom"
      fi
    else
      # Already reported under Gate 1 as NO BUDGET when named+listed; still mark Gate 3.
      printf '  %-70s %s  [NO BUDGET ENTRY]\n' "$path" "$cur"
      gate3_fail=1
    fi
    i=$((i + 1))
  done

  if [ "$budget_total" -lt 0 ]; then
    echo "error: $budget_file has no TOTAL row" >&2
    echo "  re-run: overlay/scripts/overlay-diff.sh --update-budget" >&2
    gate3_fail=1
  elif [ "$budgeted_total_lines" -gt "$budget_total" ]; then
    delta=$((budgeted_total_lines - budget_total))
    printf '  TOTAL: %s > %s (+%s)  [OVER BUDGET]\n' \
      "$budgeted_total_lines" "$budget_total" "$delta"
    echo "      shrink the overall delta, or raise with --update-budget --allow-growth"
    gate3_fail=1
  else
    total_head=$((budget_total - budgeted_total_lines))
    printf '  %-70s %s / %s  (headroom %s)\n' \
      "TOTAL" "$budgeted_total_lines" "$budget_total" "$total_head"
  fi
fi
echo

# Summary
echo "== summary =="
echo "  A files outside overlay/: $a_count  (sanctioned list: ${#adapter_paths[@]} entries)"
echo "  M files (upstream touchpoints): $m_count"
echo "  total M changed lines: $m_total_lines"
echo "  total A changed lines: $a_total_lines"
echo "  total budgeted lines (M+A): $budgeted_total_lines"
if [ "$budget_loaded" -eq 1 ] && [ "$budget_total" -ge 0 ]; then
  echo "  total budget: $budget_total"
  if [ "$budgeted_total_lines" -le "$budget_total" ]; then
    echo "  headroom: $((budget_total - budgeted_total_lines))"
  fi
fi
echo

fail=0
if [ "$gate1_fail" -eq 1 ]; then
  echo "FAIL Gate 1: whole files outside overlay/ must be overlay_*-named, listed in $adapters_file, and budgeted — or remove STALE list lines." >&2
  fail=1
fi
if [ "$gate2_fail" -eq 1 ]; then
  echo "FAIL Gate 2: add every file marked UNDOCUMENTED to $touchpoints_file (heading with the path in backticks)." >&2
  fail=1
fi
if [ "$gate3_fail" -eq 1 ]; then
  echo "FAIL Gate 3: restore files under budget, or update $budget_file deliberately via --update-budget." >&2
  fail=1
fi

if [ "$fail" -eq 1 ]; then
  exit 1
fi

echo "OK: delta did not grow past budget; all touchpoints documented; sanctioned adapters within budget."
exit 0
