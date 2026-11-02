#!/usr/bin/env bash
# Gate 0 performance sample: CPU and memory of the app, the WebKit processes it spawns,
# and the extra WindowServer CPU it causes (compositing transparent windows).
# Usage: [STRAYS_FPS=30] perf.sh A|B [seconds]   (release build)
set -euo pipefail
MODE=${1:-A}
SECS=${2:-60}
HERE=$(cd "$(dirname "$0")" && pwd)
APP=${APP:-"$HERE/../../app/src-tauri/target/release/Strays"}
WS=$(pgrep -x WindowServer)
webkit() { pgrep -f 'com.apple.WebKit' | sort; }

# average CPU of the given pids over N seconds (top's first sample has no delta, so drop it)
avg_cpu() {
  local secs=$1; shift
  local args; args=$(for p in "$@"; do printf -- '-pid %s ' "$p"; done)
  top -l $((secs / 2 + 1)) -s 2 -stats pid,cpu $args |
    awk '/^PID/ { s++; next } s > 1 && $1 ~ /^[0-9]+$/ { c[s] += $2 } END { for (i in c) { t += c[i]; k++ } printf "%.2f", t / k }'
}

ws_base=$(avg_cpu 20 "$WS")
before=$(webkit)
STRAYS_MODE=$MODE "$APP" >/dev/null 2>&1 &
APID=$!
trap 'kill $APID 2>/dev/null || true' EXIT
sleep 10 # let pets load and settle
mine=($APID $(comm -13 <(echo "$before") <(webkit)))

app_cpu=$(avg_cpu "$SECS" "${mine[@]}")
ws_run=$(avg_cpu 20 "$WS")
mem=$(ps -o rss= -p "$(IFS=,; echo "${mine[*]}")" | awk '{ t += $1 } END { printf "%.0f", t / 1024 }')
awk -v m="$MODE" -v f="${STRAYS_FPS:-30}" -v a="$app_cpu" -v b="$ws_base" -v r="$ws_run" -v mem="$mem" 'BEGIN {
  d = r - b; if (d < 0) d = 0
  printf "mode=%s fps=%s app_cpu=%.1f%% windowserver_extra=%.1f%% total=%.1f%% mem=%sMB\n", m, f, a, d, a + d, mem }'
