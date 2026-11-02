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

## App Sandbox, folder bookmarks and launch at login

- **Sandbox (§9):** `app/src-tauri/Entitlements.plist` turns on the App Sandbox with only network client, user-selected read-only files and app-scoped bookmarks. Bundles are signed ad hoc (`signingIdentity: "-"`) with the hardened runtime until a Developer ID exists. The permissions audit passes on the release bundle.
- **Folder bookmarks:** picking a folder (welcome, Settings or adoption) saves a read-only security-scoped bookmark in `bookmarks.json`, keyed by path. At launch every bookmark is opened and access kept for the run; a stale one is replaced. A watched folder's bookmark also covers the repos inside it. A folder that moved keeps working under its old path only (logged).
- **Data moves into the container:** sandboxed, the data folder is `~/Library/Containers/com.khubaibqaiser.strays/Data/Library/Application Support/…`. Earlier unsandboxed test data isn't migrated, and folders added before this change need adding again.
- **Paths still show as `~/…`:** in the sandbox `HOME` is the container, so `tilde` cuts it back to the real home.
- **Launch at login:** Settings → Privacy & Permissions has a toggle using `SMAppService.mainApp`. macOS keeps the state (System Settings → General → Login Items), so nothing is stored, and "turned off in System Settings" is shown when the user disabled it there. If macOS refuses, the box unticks and the error shows.
- **Checked:** a sandboxed debug bundle starts, draws pets and writes its data in the container, with no sandbox denials in the system log. Tests: a real bookmark resolves after a restart and stale ones are replaced and saved; `tilde` inside the container; Playwright for the toggle and a refusal. 12 app, 44 core, 17 Playwright; clippy clean.
- **Folder dialog, checked 2026-11-02:** in the sandboxed debug bundle, Settings → Add folder… opened the real dialog; the picked folder's two repos were found and the bookmark saved. After a restart a repo added while the app was closed was found. With `bookmarks.json` removed, a newly added repo was not found; with it restored, it was. No sandbox denials logged.
- **Reopen:** opening Strays again (Finder, Spotlight) shows Settings. On a full menu bar the notch hides the status icon, and this was the only way in.
- **User guide:** `docs/user-guide.md` covers install, first run, the GitHub token, folders, everyday use, the hidden-icon case, privacy and removal. Its UI labels were checked against the code.
- **Apple Silicon only (decided 2026-11-02):** Strays ships for Apple Silicon (M1 and later) and does not run on Intel Macs. On an Intel Mac, macOS refuses to open it and says it isn't supported on this Mac. CI checks the binary is arm64 with `lipo`. See [Intel Macs](#intel-macs).
- **Not checked:** launch at login from an installed `/Applications/Strays.app`; saving a token in the Keychain from the sandboxed app.

## Not done yet

| Item | Blocked on |
|---|---|
| Focus status | a Developer ID signed app (the entitlement needs a provisioning profile) |
| Signing and notarisation | Apple Developer Program enrolment, a Developer ID certificate and a notarisation key |
| Opt-in updater | signing (the update manifests are signed); then `tauri-plugin-updater` |
| Clean macOS 14 VM test | UTM and a macOS 14 restore image (spec §16) |

## Intel Macs

**Not supported.** Strays is built for Apple Silicon only.

**Why:**
- **Apple is phasing Intel out.** At WWDC 2025 Apple said macOS 26 Tahoe is the last macOS for Intel Macs. The Intel audience is shrinking and will stop getting new macOS releases.
- **It can't be tested here.** There is no Intel Mac to test on, and an Apple Silicon Mac can't run Intel macOS in a VM at a usable speed. Shipping an Intel build that was never run would be guessing.
- **Smaller download.** A universal build carries both versions and roughly doubles the size.

**Can it be added later?** Yes. The code has nothing Apple Silicon-specific, and a universal build was made and checked on 2026-11-02: it built, passed the permissions audit, and its Intel half started under Rosetta. To add it back:
1. `rustup target add x86_64-apple-darwin`.
2. Build with `npx tauri build --target universal-apple-darwin` (in CI: add `targets: x86_64-apple-darwin` to the Rust toolchain step, point the audit at `target/universal-apple-darwin/release/bundle/…`, and check `lipo -verify_arch x86_64 arm64`).

**What it costs:**

| | Apple Silicon only | Universal (measured 2026-11-02, v0.1.0) |
|---|---|---|
| App | 9.9 MiB | 20.7 MiB |
| DMG download | 4.9 MiB | 9.6 MiB |
| Release build time | one compile | two compiles, then merged (roughly double; not timed) |
| Testing | this Mac | also needs a real Intel Mac, borrowed or bought |
| Signing and notarisation | covered by the Developer Program | same, no extra fee |

