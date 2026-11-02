#!/bin/sh
# Permissions audit (spec §9, §14.2): checks a built Strays.app's Info.plist and entitlements against an allowlist.
# Usage: tools/permissions-audit.sh path/to/Strays.app
set -eu
APP=${1:?usage: $0 path/to/Strays.app}
PLIST="$APP/Contents/Info.plist"
fail=0
bad() { echo "FAIL: $*"; fail=1; }

[ "$(plutil -extract LSMinimumSystemVersion raw "$PLIST")" = "14.0" ] || bad "minimum macOS is not 14.0"
[ "$(plutil -extract LSUIElement raw "$PLIST" 2>/dev/null)" = "true" ] || bad "LSUIElement is not set (Dock icon would show)"

# usage descriptions are what macOS shows in permission prompts; only Focus status may ever be asked for
for key in $(plutil -convert xml1 -o - "$PLIST" | sed -n 's#.*<key>\(NS[A-Za-z]*UsageDescription\)</key>.*#\1#p'); do
  [ "$key" = NSFocusStatusUsageDescription ] || bad "asks for a permission: $key"
done

ENT=$(mktemp)
trap 'rm -f "$ENT"' EXIT
codesign -d --entitlements - --xml "$APP" > "$ENT" 2>/dev/null && [ -s "$ENT" ] || bad "can't read the entitlements"
ALLOWED="com.apple.security.app-sandbox com.apple.security.network.client com.apple.security.files.user-selected.read-only com.apple.security.files.bookmarks.app-scope com.apple.developer.focus-status"
for key in $(plutil -convert xml1 -o - "$ENT" 2>/dev/null | sed -n 's#.*<key>\([^<]*\)</key>.*#\1#p'); do
  case " $ALLOWED " in *" $key "*) ;; *) bad "entitlement not on the allowlist: $key" ;; esac
done
# an app without these would still pass the allowlist, so require them
for key in com.apple.security.app-sandbox com.apple.security.network.client; do
  [ "$(/usr/libexec/PlistBuddy -c "Print :$key" "$ENT" 2>/dev/null)" = true ] || bad "required entitlement missing: $key"
done

[ $fail = 0 ] && echo "permissions audit: ok"
exit $fail
