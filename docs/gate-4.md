# Gate 4: Release

Spec §14.1: a notarised `.dmg` installs and runs on a clean macOS 14 VM and on the latest macOS.

## CI (§15.3)

`.github/workflows/ci.yml`, actions pinned to commit SHAs:
- **Every PR (Linux):** core tests and clippy (`-D warnings`), the TypeScript check, Playwright, `npm audit`.
- **main and on demand (macOS):** app tests and clippy, a release build of the `.app` and `.dmg`, then the permissions audit. It's kept off PRs because the repo is private, where macOS minutes cost more.
- Dependabot checks the two Cargo crates, npm and the actions weekly.

## Bundle

- The executable is now `Strays` (it was `app`), so debug builds are at `app/src-tauri/target/debug/Strays`.
- `Info.plist` now sets `LSUIElement` (no Dock icon, even before the app starts up) and the minimum macOS to **14.0** (it said 10.13).
- Bundles are the `.app` and `.dmg`. The release `.app` is 9.9 MiB and the `.dmg` 4.9 MiB, Apple Silicon only so far.
- Building the `.dmg` locally needs `CI=true`. Otherwise the Finder AppleScript that lays out its window times out (the terminal isn't allowed to control Finder).

## Permissions audit (§9, §14.2)

`tools/permissions-audit.sh path/to/Strays.app` fails if the minimum OS isn't 14.0, `LSUIElement` is missing, any permission-prompt key other than Focus status is present, or any entitlement is outside the allowlist (sandbox, network client, user-selected read-only files, app-scope bookmarks, Focus status). Checked against a copy with a camera prompt, an audio-input entitlement and 10.13 planted: all three caught.

## Not done yet

| Item | Blocked on |
|---|---|
| App Sandbox + security-scoped bookmarks for local folders | adding `NSURL`/`NSData` to `objc2-foundation` (a dependency change). The sandbox can't be switched on before bookmarks, or watched folders stop working after a restart. |
| Launch at login (`SMAppService`) | adding `objc2-service-management` |
| Focus status | a Developer ID signed app (the entitlement needs a provisioning profile) |
| Signing and notarisation | Apple Developer Program enrolment, a Developer ID certificate and a notarisation key |
| Opt-in updater | signing (the update manifests are signed); then `tauri-plugin-updater` |
| Universal binary (Intel) | installing the `x86_64-apple-darwin` Rust target |
| Clean macOS 14 VM test | UTM and a macOS 14 restore image (spec §16) |
