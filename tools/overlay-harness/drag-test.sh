#!/usr/bin/env bash
# ⌥-drag test on the real app (§7.2): a pet ⌥-dragged to the middle of the screen snaps into a grid cell
# there, keeps it after a restart, and a plain ⌥-click still opens its card; clicks pass through again
# after the drop. Usage: drag-test.sh   (needs Accessibility for the calling app; build first with
#   npx tauri build --debug --no-bundle --features devtools   in app/)
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
APP="$HERE/../../app/src-tauri/target/debug/Strays"
H="$HERE/harness"
OUT=$(mktemp -d)
trap 'pkill -f "$APP" 2>/dev/null || true; kill $TPID 2>/dev/null || true; "$H" opt up' EXIT
TPID=

start() { # log file
  STRAYS_DATA_DIR="$OUT/data" STRAYS_SPAWN=${SPAWN:-2} STRAYS_CAPTURABLE=1 STRAYS_IGNORE_FRONT=1 "$APP" >"$1" 2>&1 &
  for _ in $(seq 100); do [ "$(grep -c '^LOADED' "$1" 2>/dev/null)" -ge 1 ] && break; sleep 0.2; done
  sleep 2
}
fail=0
check() { # name expected actual
  if [ "$2" = "$3" ]; then echo "PASS $1"; else echo "FAIL $1 (expected $2, got $3)"; fail=1; fi
}

start "$OUT/app.log"
read -r ID X Y S <<<"$(grep '^LAYOUT' "$OUT/app.log" | tail -1 | cut -d' ' -f2-)"
CX=$(awk "BEGIN{print int($X+$S*0.5)}"); CY=$(awk "BEGIN{print int($Y+$S*0.62)}")
# the middle of the main display
read -r W HH <<<"$("$H" screen)"
DX=$((W / 2)); DY=$((HH / 2))

"$H" opt down; "$H" move "$CX" "$CY"; sleep 0.6
"$H" drag "$CX" "$CY" "$DX" "$DY"; sleep 1.2
"$H" opt up; sleep 0.4
check "drop reported with a home spot" 1 "$(grep -c "^DROP pet=$ID .*home=Some" "$OUT/app.log" || true)"
check "a drag is not a click" 0 "$(grep -c '^CLICK' "$OUT/app.log" || true)"
read -r _ NX NY _ <<<"$(grep "^LAYOUT $ID" "$OUT/app.log" | tail -1 | cut -d' ' -f2-)"
# the pet's body (lower middle of its canvas) lands within a cell of where it was let go
near=$(awk "BEGIN{dx=$NX+$S*0.5-$DX; dy=$NY+$S*0.62-$DY; print (dx*dx+dy*dy < ($S*0.8)^2)}")
check "pet settled in a cell near the drop ($NX,$NY)" 1 "$near"

# clicks pass through again where the pet was (no target window: the overlay just mustn't catch it)
"$H" click "$CX" "$CY"; sleep 0.4
check "plain click after the drop isn't caught" 0 "$(grep -c '^CLICK' "$OUT/app.log" || true)"

# a ⌥-click without moving is still a click
PX=$(awk "BEGIN{print int($NX+$S*0.5)}"); PY=$(awk "BEGIN{print int($NY+$S*0.62)}")
"$H" opt down; "$H" move "$PX" "$PY"; sleep 0.6
"$H" click "$PX" "$PY" opt; sleep 0.8
"$H" opt up; "$H" esc; sleep 0.3
check "⌥-click on the moved pet opens its card" 1 "$(grep -c "^CLICK pet=$ID shift=false" "$OUT/app.log" || true)"

# the home spot survives a restart
pkill -f "$APP"; sleep 1
SPAWN=0 start "$OUT/app2.log"
read -r _ RX RY _ <<<"$(grep "^LAYOUT $ID" "$OUT/app2.log" | tail -1 | cut -d' ' -f2-)"
check "same cell after a restart" "$NX,$NY" "$RX,$RY"

echo "--- app log"; grep -v "^LAYOUT\|^STATE\|^HOVER" "$OUT/app.log" | head -20
exit $fail
