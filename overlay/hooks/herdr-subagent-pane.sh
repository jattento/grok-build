#!/bin/sh
# Mirror one native subagent into a Herdr pane running its live session.
#
#   herdr-subagent-pane.sh start   < SubagentStart payload
#   herdr-subagent-pane.sh stop    < SubagentStop payload
#   herdr-subagent-pane.sh worker <subagentId> <cwd>
#
# `start` returns immediately: the hook dispatcher awaits the hook, so anything
# slow here would stall the parent's update loop. The real work runs in
# `worker`, which must live in a session of its own — the runner killpg's the
# hook's process group on return, and that reaches plain background children.
set -u

mode=${1:-}
dir=${TMPDIR:-/tmp}/grok-subagent-panes

# Detached path: no payload on stdin, everything arrives as arguments.
if [ "$mode" = worker ]; then
  sid=$2
  cwd=$3
  # Not HERDR_PANE_ID: under `use_leader` the hook runs in the leader process,
  # which lives in whatever pane it was first started from — not in the pane
  # holding the client you are typing into. The env, the process tree and the
  # focused pane all point somewhere else. What Herdr does know is which panes
  # host a grok, so pick the one whose cwd matches this session and that is not
  # one of our own watchers (those carry a sub-* name).
  parent=$(herdr agent list |
    jq -r --arg c "$cwd" '.result.agents[]
      | select(.agent == "grok" and .cwd == $c and .name == null)
      | .pane_id' | head -1)
  if [ -n "$parent" ]; then
    pane=$(herdr pane split --pane "$parent" --direction right --cwd "$cwd" \
      --no-focus | jq -r '.result.pane.pane_id // empty')
  else
    # Last resort: Herdr's focused pane.
    pane=$(herdr pane split --direction right --cwd "$cwd" \
      --no-focus | jq -r '.result.pane.pane_id // empty')
  fi
  [ -n "$pane" ] || exit 0
  mkdir -p "$dir"
  printf %s "$pane" >"$dir/$sid"
  # Tail, not head: a UUIDv7 starts with a timestamp, so two subagents spawned
  # in the same millisecond share their leading hex and would collide on a name
  # Herdr requires to be unique among live agents.
  name=sub-$(printf %s "$sid" | tr -d - | tail -c 8)
  # `pane split` returns before the new pane's shell reaches its prompt, and
  # starting into it fails with agent_pane_busy until it does.
  i=0
  while [ "$i" -lt 30 ]; do
    if herdr agent start "$name" --kind grok --pane "$pane" \
      -- --resume "$sid" >/dev/null 2>&1; then
      exit 0
    fi
    i=$((i + 1))
    sleep 1
  done
  exit 0
fi

[ -n "${HERDR_PANE_ID:-}" ] || exit 0
command -v herdr >/dev/null 2>&1 || exit 0

payload=$(cat)
sid=$(printf '%s' "$payload" | jq -r '.subagentId // empty')
[ -n "$sid" ] || exit 0

case $mode in
start)
  cwd=$(printf '%s' "$payload" | jq -r '.cwd // empty')
  [ -n "$cwd" ] || cwd=$PWD
  python3 -c 'import subprocess,sys; subprocess.Popen(sys.argv[1:], start_new_session=True)' \
    /bin/sh "$0" worker "$sid" "$cwd" </dev/null >/dev/null 2>&1
  ;;
stop)
  [ -f "$dir/$sid" ] || exit 0
  [ -n "${GROK_HERDR_KEEP_SUBAGENT_PANES:-}" ] ||
    herdr pane close "$(cat "$dir/$sid")" >/dev/null 2>&1
  rm -f "$dir/$sid"
  ;;
esac
exit 0
