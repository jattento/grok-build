#!/bin/sh
# Mirror one native subagent into a Herdr pane running its live session.
#
#   herdr-subagent-pane.sh start   < SubagentStart payload
#   herdr-subagent-pane.sh stop    < SubagentStop payload
#   herdr-subagent-pane.sh worker <subagentId> <cwd> <parentSessionId>
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
  psid=$4
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

  # The layout is: the parent keeps the whole left half, subagents stack down
  # the right one. So the first subagent splits the parent to the right, and
  # every later one splits the newest live watcher downwards. Placement is
  # serialized, or two subagents starting together would both see an empty
  # right column and open two of them.
  mkdir -p "$dir"
  i=0
  while ! mkdir "$dir/.lock" 2>/dev/null; do
    i=$((i + 1))
    [ "$i" -lt 60 ] || break
    sleep 1
  done

  anchor=
  for f in $(ls -t "$dir" 2>/dev/null); do
    case $f in .*) continue ;; esac
    p=$(cat "$dir/$f" 2>/dev/null)
    [ -n "$p" ] || continue
    live=$(herdr pane get "$p" | jq -r '.result.pane.pane_id // empty')
    [ -n "$live" ] || continue
    anchor=$p
    break
  done

  if [ -n "$anchor" ]; then
    pane=$(herdr pane split --pane "$anchor" --direction down --cwd "$cwd" \
      --no-focus | jq -r '.result.pane.pane_id // empty')
  elif [ -n "$parent" ]; then
    pane=$(herdr pane split --pane "$parent" --direction right --ratio 0.5 \
      --cwd "$cwd" --no-focus | jq -r '.result.pane.pane_id // empty')
  else
    # Last resort: Herdr's focused pane.
    pane=$(herdr pane split --direction right --ratio 0.5 --cwd "$cwd" \
      --no-focus | jq -r '.result.pane.pane_id // empty')
  fi
  [ -n "$pane" ] && printf %s "$pane" >"$dir/$sid"
  rmdir "$dir/.lock" 2>/dev/null
  [ -n "$pane" ] || exit 0
  # Tail, not head: a UUIDv7 starts with a timestamp, so two subagents spawned
  # in the same millisecond share their leading hex and would collide on a name
  # Herdr requires to be unique among live agents.
  name=sub-$(printf %s "$sid" | tr -d - | tail -c 8)
  # `pane split` returns before the new pane's shell reaches its prompt, and
  # starting into it fails with agent_pane_busy until it does.
  i=0
  while [ "$i" -lt 30 ]; do
    herdr agent start "$name" --kind grok --pane "$pane" \
      -- --resume "$sid" >/dev/null 2>&1 && break
    i=$((i + 1))
    sleep 1
  done

  # From here the worker guards the pane. `SubagentStop` closes it on a normal
  # finish, but that hook never fires for an interrupted turn, for a turn the
  # provider killed, and it cannot fire at all if the parent dies. All three
  # leave an orphan, so watch for them. A cancelled subagent writes
  # outcome=cancelled into its own events.jsonl. A subagent whose provider
  # gave up writes a retry_state of type `failed` into updates.jsonl — the
  # same record type as `retrying`, which is why the type has to be matched
  # and not just the word; a 401 or a rejected tool schema ends there and
  # nothing else is ever written. Neither marker appears in a session that
  # completed (checked across 200 local sessions). A dead parent drops out of
  # the active-session roster.
  i=0
  while [ "$i" -lt 10800 ]; do
    sleep 2
    i=$((i + 1))
    [ -f "$dir/$sid" ] || exit 0
    herdr pane get "$pane" | jq -e '.result.pane.pane_id' >/dev/null 2>&1 ||
      { rm -f "$dir/$sid"; exit 0; }

    gone=
    for f in "$HOME"/.grok/sessions/*/"$sid"/events.jsonl; do
      [ -f "$f" ] && grep -q '"outcome":"cancelled"' "$f" && gone=1
    done
    for f in "$HOME"/.grok/sessions/*/"$sid"/updates.jsonl; do
      [ -f "$f" ] && jq -e 'select(.params.update.sessionUpdate == "retry_state"
        and .params.update.type == "failed")' "$f" >/dev/null 2>&1 && gone=1
    done
    if [ -z "$gone" ] && [ -n "$psid" ]; then
      ppid=$(jq -r --arg s "$psid" \
        '.[] | select(.session_id == $s) | .pid' \
        "$HOME/.grok/active_sessions.json" 2>/dev/null | head -1)
      { [ -n "$ppid" ] && kill -0 "$ppid" 2>/dev/null; } || gone=1
    fi
    [ -n "$gone" ] || continue

    [ -n "${GROK_HERDR_KEEP_SUBAGENT_PANES:-}" ] ||
      herdr pane close "$pane" >/dev/null 2>&1
    rm -f "$dir/$sid"
    exit 0
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
  psid=$(printf '%s' "$payload" | jq -r '.sessionId // empty')
  python3 -c 'import subprocess,sys; subprocess.Popen(sys.argv[1:], start_new_session=True)' \
    /bin/sh "$0" worker "$sid" "$cwd" "$psid" </dev/null >/dev/null 2>&1
  ;;
stop)
  [ -f "$dir/$sid" ] || exit 0
  [ -n "${GROK_HERDR_KEEP_SUBAGENT_PANES:-}" ] ||
    herdr pane close "$(cat "$dir/$sid")" >/dev/null 2>&1
  rm -f "$dir/$sid"
  ;;
esac
exit 0
