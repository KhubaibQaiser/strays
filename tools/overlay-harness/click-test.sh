#!/usr/bin/env bash
# Gate 2 interaction test on the real app: plain clicks pass through a pet; ⌥-hover outlines it;
# ⌥-click opens its menu; ⌥⇧-click quick-syncs it; ⌥ on the transparent margin still passes through.
# Usage: click-test.sh   (needs Accessibility for the calling app; build first with
#   npx tauri build --debug --no-bundle --features devtools   in app/)
# Pets are throwaway (STRAYS_SPAWN, temp data folder). Syncs go to the fake GitHub server.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
APP="$HERE/../../app/src-tauri/target/debug/Strays"
H="$HERE/harness"
OUT=$(mktemp -d)
trap 'pkill -f "$APP" 2>/dev/null || true; kill $TPID $FPID 2>/dev/null || true; "$H" opt up' EXIT

python3 "$HERE/../fake-github/server.py" >"$OUT/fake.log" &
FPID=$!
for _ in $(seq 50); do grep -q PORT "$OUT/fake.log" && break; sleep 0.1; done
PORT=$(sed -n 's/PORT //p' "$OUT/fake.log")

STRAYS_DATA_DIR="$OUT/data" STRAYS_SPAWN=1 STRAYS_CAPTURABLE=1 STRAYS_TOKEN=github_pat_fake \
  STRAYS_FAKE_GITHUB="http://127.0.0.1:$PORT" "$APP" >"$OUT/app.log" 2>&1 &
# wait for the pet to load and the first sync to settle, so the layout is final before clicking
for _ in $(seq 100); do grep -q "^LOADED" "$OUT/app.log" 2>/dev/null && grep -q "Last synced" "$OUT/app.log" && break; sleep 0.2; done
sleep 1.5
read -r ID X Y S <<<"$(grep '^LAYOUT' "$OUT/app.log" | tail -1 | cut -d' ' -f2-)"
CX=$(awk "BEGIN{print int($X+$S*0.5)}"); CY=$(awk "BEGIN{print int($Y+$S*0.62)}")
MX=$(awk "BEGIN{print int($X+6)}"); MY=$(awk "BEGIN{print int($Y+6)}")

# The target window may become the frontmost window once clicked, and pets keep out of the middle 60%
# of the frontmost window (§7.1). So it extends left of the pet, putting the pet in its outer 20%.
TX=$(awk "BEGIN{print int($X-4*$S)}"); TY=$(awk "BEGIN{print int($Y-$S)}")
"$H" target "$TX" "$TY" "$((5 * ${S%.*}))" "$((2 * ${S%.*}))" 60 >"$OUT/target.log" 2>&1 &
TPID=$!
for _ in $(seq 50); do grep -q "TARGET READY" "$OUT/target.log" 2>/dev/null && break; sleep 0.1; done
sleep 0.5

count() { grep -c "$1" "$2" || true; }
fail=0
check() { # name expected actual
  if [ "$2" = "$3" ]; then echo "PASS $1"; else echo "FAIL $1 (expected $2, got $3)"; fail=1; fi
}

# 1. plain click on the pet goes through to the window underneath
"$H" move "$CX" "$CY"; sleep 0.4
"$H" click "$CX" "$CY"; sleep 0.5
check "plain click passes through" 1 "$(count 'TARGET CLICK' "$OUT/target.log")"
check "plain click not caught by pet" 0 "$(count '^CLICK' "$OUT/app.log")"

# 2. ⌥ over the pet: hover reported, click opens the pet's menu
"$H" opt down; "$H" move "$((CX + 1))" "$CY"; sleep 0.6
check "⌥-hover detected" 1 "$(count "HOVER Some($ID) held=true" "$OUT/app.log")"
"$H" click "$CX" "$CY" opt; sleep 0.8
check "⌥-click caught by pet" 1 "$(count "^CLICK pet=$ID shift=false" "$OUT/app.log")"
check "⌥-click not passed through" 1 "$(count 'TARGET CLICK' "$OUT/target.log")"
# close the menu by clicking away from it on the target window (Escape leaves the next click
# swallowed: a known AppKit pop-up issue, fixed by the info card in Gate 2b). Clicking another app's
# window would raise it over the target, so stay on the target and count from here.
"$H" opt up; "$H" click "$((${TX%.*} + 20))" "$((${TY%.*} + 20))"; sleep 0.4; "$H" opt down; "$H" move "$CX" "$CY"; sleep 0.6
BASE=$(count 'TARGET CLICK' "$OUT/target.log")

# 3. ⌥⇧-click quick-syncs
"$H" click "$CX" "$CY" optshift; sleep 2
check "⌥⇧-click syncs this pet" 1 "$(count "^CLICK pet=$ID shift=true" "$OUT/app.log")"
check "sync reached the fake server" 1 "$(grep -c 'Last synced' "$OUT/app.log" | awk '{print ($1 >= 1)}')"

# 4. ⌥ held over the transparent margin still passes through
"$H" move "$MX" "$MY"; sleep 0.6
"$H" click "$MX" "$MY" opt; sleep 0.5
check "⌥-click on margin passes through" $((BASE + 1)) "$(count 'TARGET CLICK' "$OUT/target.log")"

# 5. release ⌥: back to pass-through
"$H" move "$CX" "$CY"; "$H" opt up; sleep 0.6
"$H" click "$CX" "$CY"; sleep 0.5
check "after ⌥ release, click passes through" $((BASE + 2)) "$(count 'TARGET CLICK' "$OUT/target.log")"
check "pet caught exactly two clicks" 2 "$(count '^CLICK' "$OUT/app.log")"
check "pet stayed put" 1 "$(grep "^LAYOUT $ID" "$OUT/app.log" | sort -u | wc -l | tr -d ' ')"

echo "--- app log"; grep -v "^LAYOUT" "$OUT/app.log" | head -30
exit $fail
