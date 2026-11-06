# Gate 2a — app shell and overlay (2026-10-31)

Gate 2 is split. **2a** (this branch) connects the core to the Tauri app and builds the overlay, the menu bar and the sync loop. **2b** adds the windows: info card, settings and token setup, first run and adoption, plus local folders and the remaining overlay gestures. Spec §14.1 lists Gate 2 as overlay-only, but the UI screens in §8 had no gate, so 2b covers them.

## What's in it

| Area | Where | Notes |
|---|---|---|
| Token in the Keychain (§6.1) | `src-tauri/src/token.rs` | Debug builds also read `~/.config/strays/dev-token`, or `STRAYS_TOKEN` in tests. One token (personal account) until the setup screen exists. |
| Background loop (§6.3, §11) | `sync.rs` | Draws pets at launch before the first sync. Then: syncs on launch, on the chosen interval and after waking (spotted from a gap between loop passes); backs off after failures; waits out rate limits; skips scheduled syncs in Low Power Mode; ends due snoozes and holidays; refreshes at least once a minute. Network calls run without holding the state lock (core `fetch` / `apply`). |
| App state and actions | `state.rs` | Adopt (from a GitHub repo, or into the shelter), snooze (1 h, 4 h, until 09:00 tomorrow, 3 days, until woken), wake, pause, resume, mark release, graduate, let go, bring back. Vacation mode pauses everyone and resumes only the pets it paused. |
| Who's on screen (§5.7) | core `display.rs` | Neediest first, at most one Gloomy pet, ties go to the pet seen longest ago, rotation every N hours, and a menu-bar badge counting hidden needy pets. |
| Menu bar (§7.5) | `menu.rs` | Sync status and "Sync now"; pets grouped by state, each with its actions (Graduate and Let go need a confirming second click); new projects (adopt as a species, add to the shelter, or ignore); "Adopt a project" (the 20 most recently pushed untracked repos); Presentation, Vacation and Hide-all toggles; Quit. |
| Overlay placement (§7.1) | `overlay.rs` | One transparent window per display; new pets start on the main display, a pet ⌥-dragged onto another display lives there, and returns to the main one while its display is unplugged. Ground pets line up along the bottom from the right; the parrot perches top right. Pets keep out of the middle 60% of the frontmost window when it reaches the bottom, and hide when an app is full screen. Rechecked every 2 s, but never while ⌥ is held. |
| Native window rules | `macos.rs` | `sharingType = none`, all Spaces, stationary, full-screen auxiliary, out of ⌘-Tab. Frontmost window bounds from `CGWindowListCopyWindowInfo` (bounds only, no Screen Recording). Reduce Motion and Low Power Mode from AppKit. |
| Interaction (§7.2, §7.3) | `overlay.rs`, `src/main.ts` | Polls cursor and ⌥ (no permission). Holding ⌥ freezes every pet and outlines the one under the cursor with a tooltip (name · repo · mood; no repo in Presentation mode). ⌥-click opens that pet's menu; ⌥⇧-click syncs and shows ✓ or ⚠. |
| Liveliness (§7.4) | `src/main.ts` | Calm: a 1 s moment every 20–60 s. Gentle: every 5–15 s. Lively: continuous at 15 fps. Moments play at 24 fps, and rendering stops completely between them. Reduce Motion forces Calm. |

## Results

**Click test** (`tools/overlay-harness/click-test.sh`, real OS events): **11/11 checks, 3 runs in a row.** It checks:
- a plain click passes through a pet
- ⌥-hover is detected
- ⌥-click opens the menu and doesn't pass through
- ⌥⇧-click syncs, and the sync reaches the fake server
- a ⌥-click on the transparent margin passes through
- after ⌥ is released, clicks pass through again
- the pet doesn't move during the test

**Capture exclusion:** a screenshot of the pet's area shows the pet only when the debug flag `STRAYS_CAPTURABLE=1` re-allows capture. With the default setting, the area is empty.

**CPU and memory** (3 pets, debug build, fake GitHub; app + WebKit processes + extra WindowServer time):

| Setup | Sample | CPU | Memory |
|---|---|---|---|
| No pets | 90 s | 0.1% app + 0.6% WindowServer | 130 MB |
| Calm, first version (1.5 s moments at 30 fps) | 180 s | 1.9% | 169 MB |
| Calm (1 s at 24 fps) | 180 s | 0.9% | 146 MB |
| Calm | 10 min | 1.2%* | 79 MB* |
| Gentle | 10 min | 3.1%* | 85 MB* |
| Lively, first run | 60 s | 11.3% | 239 MB |
| Lively | 3 min | 2.5%* | 120 MB* |

\* **Not reliable.** These runs happened while the Mac was in use. Pets move aside for the frontmost window and hide over full-screen apps, so part of each run probably measured hidden pets; Lively coming out below Gentle shows it. **Budget decision (user, 2026-11-01):** Calm's target is now **≤ 1.5%**, set from these measurements: 0.9% over 3 minutes and 1.2% over 10 minutes. The original < 1% wasn't reached reliably. `STRAYS_IGNORE_FRONT=1` (debug only) stays available for repeatable runs.

The test pets are added by `STRAYS_SPAWN`, which exists only in debug builds, so these figures come from an unoptimised build. The Rust share is small (the no-pet run), so a release build should be about the same or slightly lower.

## Known issues

- **Escape on the pet menu:** after Escape closes a pet's ⌥-click menu, AppKit keeps the menu's tracking loop waiting, so the next click is swallowed. Closing the menu with a click works normally. Two fixes failed: waking the event loop, and briefly activating the app (which also left Strays frontmost). The info card in 2b replaces this menu.
- **Menu-bar icon space:** with a full menu bar, macOS hides extra items behind the notch, so the 🐾 icon may not show. The user hasn't confirmed it yet.
- **Not implemented yet:** "Only sync on Wi-Fi" (macOS has no simple API for it without Network.framework bindings).
- Every species renders as the cat until Gate 3.

## Moved to Gate 2b

Info card (§8.3) · settings window (§8.4) and token setup (§6.2) · first run and adoption flow (§8.1, §8.2) · local folders with FSEvents (§5.9) · ⌥-drag and home spots · petting · ⌥-right-click menu · nudges and speech bubbles (§5.10) · waking-up sequence · feeding · per-display windows (⌥-drag a pet between displays) · Network activity view.
