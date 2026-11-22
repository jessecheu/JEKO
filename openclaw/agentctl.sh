#!/usr/bin/env bash
# Run JEKO's OpenClaw agents as background jobs through the Gateway (FR30).
#
#   openclaw/agentctl.sh up                 start the Gateway in the background
#   openclaw/agentctl.sh run AGENT MESSAGE  start one agent run, detached; prints its log
#   openclaw/agentctl.sh jobs               list runs and whether each is still going
#   openclaw/agentctl.sh logs [AGENT]       follow the newest log (of AGENT)
#   openclaw/agentctl.sh status             Gateway status
#   openclaw/agentctl.sh down               stop the Gateway
#
# Why the Gateway: `openclaw agent --local` takes an exclusive lock on OpenClaw's
# state database, so only one run can go at a time. With the Gateway running, every
# `openclaw agent` call is sent to it and runs can overlap.
#
# Keys: the Gateway, and so every agent on it, gets the environment of the shell that
# ran `up`. Export CEREBRAS_API_KEY there only if a job needs it (the dialogue refresh
# does; monsters and playtest don't), and `down` the Gateway when done.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
state="${JEKO_AGENT_RUNS:-$root/.agent-runs}"
mkdir -p "$state"
oc="${OPENCLAW_BIN:-openclaw}"

die() { echo "agentctl: $*" >&2; exit 1; }

gateway_pid() { [ -f "$state/gateway.pid" ] && kill -0 "$(cat "$state/gateway.pid")" 2>/dev/null && cat "$state/gateway.pid"; }

case "${1:-}" in
up)
  if pid=$(gateway_pid); then echo "Gateway already running (pid $pid)"; exit 0; fi
  if pgrep -f "openclaw.*(agent --local|gateway)" >/dev/null 2>&1; then
    echo "note: another OpenClaw process is running; if it is a --local run the Gateway cannot start until it ends" >&2
  fi
  setsid nohup "$oc" gateway > "$state/gateway.log" 2>&1 < /dev/null &
  echo $! > "$state/gateway.pid"
  # Wait for it to answer, up to 60 s.
  for _ in $(seq 60); do
    "$oc" gateway status 2>/dev/null | grep -qiE "running|reachable|listening" && { echo "Gateway up (pid $(cat "$state/gateway.pid")), log $state/gateway.log"; exit 0; }
    kill -0 "$(cat "$state/gateway.pid")" 2>/dev/null || die "Gateway exited; see $state/gateway.log"
    sleep 1
  done
  die "Gateway did not come up in 60 s; see $state/gateway.log"
  ;;
run)
  agent="${2:-}"; [ -n "$agent" ] || die "usage: run AGENT MESSAGE"
  shift 2; msg="$*"; [ -n "$msg" ] || die "usage: run AGENT MESSAGE"
  gateway_pid >/dev/null || die "Gateway is not running; start it with: $0 up"
  log="$state/$agent-$(date +%Y%m%d-%H%M%S).log"
  # No --local: the call goes to the Gateway, so runs of different agents overlap.
  setsid nohup "$oc" agent --agent "$agent" --message "$msg" > "$log" 2>&1 < /dev/null &
  echo $! > "${log%.log}.pid"
  echo "started $agent (pid $!), log $log"
  ;;
jobs)
  shopt -s nullglob
  for p in "$state"/*.pid; do
    [ "$p" = "$state/gateway.pid" ] && continue
    if kill -0 "$(cat "$p")" 2>/dev/null; then s=running; else s=done; fi
    printf '%-8s %s\n' "$s" "$(basename "${p%.pid}")"
  done
  ;;
logs)
  pat="${2:-}"
  latest=$(ls -t "$state"/"$pat"*.log 2>/dev/null | grep -v gateway.log | head -1 || true)
  [ -n "$latest" ] || die "no logs"
  echo "== $latest"; tail -f "$latest"
  ;;
status) "$oc" gateway status ;;
down)
  if pid=$(gateway_pid); then kill "$pid"; echo "stopped Gateway (pid $pid)"; else echo "Gateway not running"; fi
  rm -f "$state/gateway.pid"
  ;;
*) sed -n '2,15p' "${BASH_SOURCE[0]}"; exit 2 ;;
esac
