#!/usr/bin/env bash
# Gate 0 interaction test: plain clicks pass through a pet; ⌥-click hits the pet.
# Usage: click-test.sh A|B   (needs Accessibility for the calling app; app must be built)
set -euo pipefail
MODE=${1:-A}
HERE=$(cd "$(dirname "$0")" && pwd)
APP="$HERE/../../app/src-tauri/target/debug/app"
H="$HERE/harness"
OUT=$(mktemp -d)
trap 'pkill -f "$APP" 2>/dev/null || true; kill $TPID 2>/dev/null || true' EXIT

STRAYS_MODE=$MODE "$APP" >"$OUT/app.log" 2>&1 &
for _ in $(seq 50); do grep -q "LOADED pet=0" "$OUT/app.log" 2>/dev/null && break; sleep 0.2; done
IFS=, read -r X Y W Hh <<<"$(sed -n 's/.*rects=\([^;]*\).*/\1/p' "$OUT/app.log")"
CX=$(awk "BEGIN{print int($X+$W*0.5)}"); CY=$(awk "BEGIN{print int($Y+$Hh*0.6)}")
# a corner of the pet's square that is outside its hit area (transparent margin)
MX=$(awk "BEGIN{print int($X+6)}"); MY=$(awk "BEGIN{print int($Y+6)}")

"$H" target "$X" "$Y" "$W" "$Hh" 30 >"$OUT/target.log" 2>&1 &
TPID=$!
sleep 1

count() { grep -c "$1" "$2" || true; }
fail=0
check() { # name expected actual
  if [ "$2" = "$3" ]; then echo "PASS $1"; else echo "FAIL $1 (expected $2, got $3)"; fail=1; fi
}

# 1. plain click on the pet goes through to the window underneath
"$H" move "$CX" "$CY"; sleep 0.4
"$H" click "$CX" "$CY"; sleep 0.5
check "plain click passes through" 1 "$(count 'TARGET CLICK' "$OUT/target.log")"
check "plain click not caught by pet" 0 "$(count 'CLICK pet=' "$OUT/app.log")"

# 2. hold ⌥ over the pet: hover state turns on, click lands on the pet
"$H" opt down; "$H" move "$((CX + 1))" "$CY"; sleep 0.6
check "⌥-hover detected" 1 "$(count 'HOVER Some(0)' "$OUT/app.log")"
"$H" click "$CX" "$CY" opt; sleep 0.5
check "⌥-click caught by pet" 1 "$(count 'CLICK pet=0' "$OUT/app.log")"
check "⌥-click not passed through" 1 "$(count 'TARGET CLICK' "$OUT/target.log")"

# 3. ⌥ held over the transparent margin of the pet window still passes through
"$H" move "$MX" "$MY"; sleep 0.6
"$H" click "$MX" "$MY" opt; sleep 0.5
check "⌥-click on margin passes through" 2 "$(count 'TARGET CLICK' "$OUT/target.log")"

# 4. release ⌥: back to pass-through
"$H" move "$CX" "$CY"; "$H" opt up; sleep 0.6
"$H" click "$CX" "$CY"; sleep 0.5
check "after ⌥ release, click passes through" 3 "$(count 'TARGET CLICK' "$OUT/target.log")"
check "pet caught exactly one click" 1 "$(count 'CLICK pet=' "$OUT/app.log")"

echo "--- app log"; cat "$OUT/app.log"
exit $fail
