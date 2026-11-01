# Gate 2b — windows, local folders and nudges (2026-11-01)

Gate 2b adds the app's standard windows (spec §8), local folders as projects (§5.4, §5.9), and nudges (§5.10). With it, Strays is usable end to end without the agent's debug hooks: connect GitHub or pick folders, adopt pets, look after them, and change every setting.

## What's in it

| Area | Where | Notes |
|---|---|---|
| Info card (§8.3) | `src/ui/card.ts`, `windows.rs` | ⌥-click opens a small panel beside the pet. It shows mood with the days idle and days until the next mood, what the pet tracks with source badges, the last activity, and the goal with a tickable checklist. It has Sync, Snooze, Mark release, Pause/Resume/Wake, Open on GitHub, Reveal in Finder, Graduate (needs a second click) and pet settings (name, pace). It closes on Escape or when it loses focus. **It replaces the pop-up menu, so the swallowed click after Escape (a Gate 2a known issue) is gone.** |
| Settings (§8.4) | `src/ui/settings.ts` | Pets, Moods (thresholds with validation, presets, weekends), Display (liveliness with measured CPU cost, visible counts, rotation, full-screen behaviour, Presentation, Hide all), Nudges, Interaction (⌥, ⌃, ⇧ or ⌥⇧), Syncing, Privacy & Permissions, About. Every change saves straight away; the core validates it again. |
| Privacy & Permissions (§8.5) | `settings.ts` | GitHub card (can / can't, account, expiry, Test, Replace token, Disconnect); project folders; network card (requests today, activity log, go fully offline); screen capture; your data (export JSON, open the data folder, delete everything after typing DELETE). |
| GitHub setup (§6.2) | `src/ui/setup.ts`, `commands.rs` | Guided steps and a button that opens GitHub's token page. You paste the token yourself; the clipboard is never read. It's checked first, the result is shown (login, repo count, expiry, what it can't read), and only then saved to the Keychain. The token goes into the app and never comes back out to a window. |
| Adoption (§8.2) | `src/ui/adopt.ts` | A GitHub repo, a git folder from the watched folders, or any folder (picker); species; colour from curated looks or any colour, with shade, highlight and outline derived from it and a live Rive preview; name (random suggestion); goal; pace. A local folder is linked to its GitHub repo when its remote matches exactly one. |
| First run (§8.1) | `src/ui/welcome.ts` | Welcome, then "where are your projects?" (GitHub, folders or both), then comfort settings (nudges per day, quiet hours, pets on screen), then adoption. Opens automatically on a fresh install. |
| Local folders (§5.4, §5.9) | `state.rs`, `sync.rs` | Pets with a folder pick up its newest local commit and remote-tracking dates (gix, no `git`, no fetch). Watched folders are scanned at launch, every 6 hours, and 2 s after FSEvents reports a change. Repos that appear after a folder is first watched are offered as new projects. |
| Nudges (§5.10) | core `nudge.rs` | Only hungry or gloomy pets on screen speak. Per-pet and daily caps, at least an hour apart, never in quiet hours, Presentation mode or over a full-screen app, style Off respected. When every pet is hungry, a "taking a break?" prompt appears at most weekly. Shown as an 8-second bubble. |
| Network log (§8.5) | core `github.rs`, `state.rs` | Every GitHub request: time, host, path (repo ids only), status, reason (scheduled, manual, validation). The last 1,000 are kept. |
| Offline mode | `sync.rs` | "Go fully offline" stops all network, GitHub included. |

**Permissions:** the web content gets no plugin permissions. Opening links (only a fixed set), revealing folders and the folder picker all run in Rust commands.

## Results

- **Window tests** (`npm run test:ui`, Playwright in Chromium with the app's commands mocked): **7/7.** They cover:
  - the info card content, checklist saving, the two-click Graduate, and Escape closing it
  - settings saving, threshold validation and liveliness
  - the privacy page and its activity log, and the delete guard
  - token check → result → save, and a refused token
  - adoption validation and every adoption field
  - the three welcome steps
- **Screenshots reviewed:** info card, settings (Pets, Display, Privacy), setup, adoption with the live recoloured preview, welcome. In the real app, the info card opens beside the pet.
- **Rust:** 42 core tests (new: nudges, quiet hours, network log) and 7 app tests (new: watched-folder rule). Clippy reports no warnings.
- **OS click test** on the real app: 11/11. One finding along the way: the test sometimes started clicking before the app's first sync had settled (2 of 9 runs failed). It now waits for the first sync and for the target window. Since then: 3/3 passes, plus 4 earlier passes without the wait.

## Not covered or not built

- **Sound** (§8.4 Sound), **launch at login** and **Focus status**: there's no audio yet, and the other two need the signed app (Gate 4). The Privacy page says so.
- **Only sync on Wi-Fi**: still not built (shown disabled).
- **Per-owner tokens** (§6.1 "one per resource owner"): one token for now.
- **Token expiry reminders** (§6.4) appear in the Privacy card as the expiry date. The 7-day and 1-day reminder bubbles aren't built yet.
- **⌥-drag, home spots, petting, per-display windows, waking and feeding animations**: these need the Gate 3 art, or are best built alongside it.
- **App Sandbox:** local folders are stored as plain paths. Security-scoped bookmarks arrive with the sandbox in Gate 4.
- **Front-end tests use mocks:** the real windows were only spot-checked (the info card).
