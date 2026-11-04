# Strays: review findings and fix spec (draft for alignment, 2026-11-04)

**Sources:**
- Five read-only reviews: requirements, security, architecture and performance, UX, and error handling.
- The 3D performance discussion.
- What I found while building drag-to-grid.

**Verification:** I checked every item marked ✔ in the code myself. The other items are what a reviewer reported, with file and line evidence. "(suspected)" means nobody has confirmed it yet.

**Legend:**
- Severity: **P0** before 3D ships · **P1** soon · **P2** hardening and tidying · **P3** nice to have.
- Size: **S** under an hour · **M** half a day · **L** a day or more (my estimates).
- "Decide" means I need your answer before I start.

---

## A. 3D rendering: performance and architecture

| ID | Sev | Size | Finding | Proposed fix |
|---|---|---|---|---|
| A1 ✔ | P0 | S | **The app-wide frame cap (`throttle.ts`) for the Rive pets also throttles the 3D pets.** It uses one shared budget of 24/s (15/s on Lively) across all pets. The FPS table in `rig.ts:16` never applies, frames come from timers rather than the display refresh (judder), and the CPU figures measured so far are too low. | Give 3D its own display-locked loop on the native rAF. Re-measure CPU. |
| A2 | P0 | M | **Frame policy.** Agreed direction: lock to the display refresh. Draw at 60 fps only during fast motion (head snaps, actions, panting), 30 fps for breathing and sway, ~10 fps while asleep. Under load, cut fur layers and resolution before frames. | **Decide (Q1).** |
| A3 ✔ | P0 | L | **Every pet has its own WebGL context, reflection map and shader compile** (`pet.ts:348`). That means a stall for every new pet, a few MB each (estimate), and a ceiling of about 16 contexts in WebKit (from memory, not checked) against up to 20 slots. A dropped context silently turns a pet into its Rive drawing. | One shared renderer that draws each pet in turn and copies the result into the pet's small canvas. Measure that copy in WebKit first. **Decide (Q2).** |
| A4 ✔ | P0 | S | `dispose()` never calls `forceContextLoss()`, so old contexts linger. | Call it in cleanup. |
| A5 ✔ | P1 | S | **A full-screen app empties the frame, so every pet is destroyed and rebuilt** (`overlay.rs` `place`). That churns contexts and stalls again. | Hide and pause the pets instead of removing them. |
| A6 | P1 | M | **The 4 meshes are rebuilt on the main thread at first spawn**, a stall of about 0.5 s. They are deterministic. | Build them when the app is built and ship them as binary assets. |
| A7 | P1 | S | **Each frame is expensive.** Rendering is at 2× Retina, with 4× antialiasing, 20–29 fur layers and `discard`. That is about 3M fragments per frame per cat (estimate). | Cap the pixel ratio at 1.5. Turn off antialiasing for the fur layers. Use fewer layers while a pet is small or idle. |
| A8 | P1 | S | **Nothing pauses when the screen locks or the display sleeps.** | Listen for sleep and lock notifications in `macos.rs`, and add a pause flag to `Frame`. |
| A9 | P2 | S | `setShells` replaces an instance buffer without disposing of the old one (`pet.ts:469`). | Dispose of it. |
| A10 | P2 | S | **The resize check rounds while three.js floors**, so a fractional size redraws on every frame message (suspected). | Use the same rounding in both places. |
| A11 | P2 | S | **The Settings text "Calm … about 1% CPU" was measured on Rive.** 3D at Calm draws continuously. | Rewrite it from the A1 and A2 measurements. |

## B. Robustness and concurrency

| ID | Sev | Size | Finding | Proposed fix |
|---|---|---|---|---|
| B1 ✔ | P0 | S | **The sync loop reads the Keychain while holding the app lock** (`sync.rs:169`). A Keychain prompt, or a keychain locked after wake, would freeze the menu and every window. | Read it before taking the lock, the way startup already does. |
| B2 | P1 | S | **`pet_click` holds the app lock while it builds the card window** (`lib.rs:57-77`). | Copy what it needs, release the lock, then open the window. |
| B3 | P1 | S | **The overlay's resize can fail without being retried.** The new screen is recorded anyway, so pets can end up off-screen. | Record the screen only once the resize succeeds, and raise an issue on failure. |
| B4 | P1 | S | **A failed network-log read makes the next append overwrite the file** (`state.rs:772-799`), losing its history. | Append instead of rewriting. |
| B5 | P2 | S | **The cursor poller takes the overlay lock three times per tick.** | Take it once. |
| B6 | P2 | M | **Data backups (spec §9) were never built.** There are atomic writes but no rotating copies and no migrations. | Keep 3 rotating backups, plus a migration hook keyed on `schema_version`. **Decide (Q6).** |

## C. Errors and the log (your rule: never suppress; always show and log)

| ID | Sev | Size | Finding | Proposed fix |
|---|---|---|---|---|
| C1 ✔ | P0 | S | **In release builds every overlay error is thrown away.** `pet_log` does nothing outside debug builds (`lib.rs:105`). That covers WebGL failures, Rive load failures, slow-GPU warnings and JS exceptions. | Add an `overlay_problem(key, text)` command that logs the error and raises an issue (length-capped, with the key prefixed `overlay:`). Keep `pet_log` for debug-only traces. |
| C2 | P0 | S | **3D and Rive failures are only logged:** no WebGL, a context loss with fallback, software rendering, frame-rate cuts, a Rive load error. | Send each through C1 so it shows in the menu and Settings. |
| C3 | P1 | S | **Shader compile errors have no handler**, so the pet draws nothing (suspected). | Set `renderer.debug.onShaderError`, report the error through C1, and fall back to Rive. |
| C4 | P1 | S | **If a `listen` call fails, the overlay never asks for its first frame**, so no pets ever appear. | Catch the failure, report it, and ask for the frame anyway. |
| C5 | P1 | S | **Errors from webview commands are shown but never logged** (save settings, token save, login item, export, and others). | Log every `Err` in one place: a wrapper on the command result. |
| C6 | P1 | S | **Global JS errors in the UI windows go only to a temporary notice.** | Log them too, and raise an issue. |
| C7 | P1 | M | **If startup fails, the app just disappears.** That covers `App::load`, the store, the tray, and `data_dir`'s `expect` before the log exists. | Show a native alert explaining what failed and where the log is, then exit. |
| C8 | P1 | S | **Click-through and resize failures are log-only.** Click-through failing can block desktop clicks. | Raise an issue from the poller. |
| C9 | P1 | S | **"Test sound" reports success with no audio device**, and the real error is dropped (`audio.rs:74`). | Return the error. |
| C10 | P1 | S | **When the log file can't be opened, only `eprintln!` reports it**, and "Show log" then says "Nothing logged yet". | Raise an issue. |
| C11 | P2 | S | **Update-check failures are never shown.** | Raise an issue after it fails several days in a row (offline days don't count). **Decide (Q5).** |
| C12 | P2 | S | **Silent but harmless:** window routing (`windows.rs:12`), skipped broken git refs, store temp-file cleanup, the folder-watch callback, `primary_monitor` errors. | Log each one. |
| C13 | P2 | S | **`warn` logs before removing duplicates**, so a persistent issue writes a line every pass and pushes history out of the 1 MB cap. | Log only when the issue first appears or its text changes, plus one line when it resolves. |
| C14 | P2 | S | **The log has no startup header** (app version, macOS, architecture). | Write one each launch. |
| C15 | P1 | M | **Sendable log.** Today there is only "Show log" in Finder: no bundle, no scrubbing, and `.log.1` is left out. | "Save log for a bug report": one file containing the header, both log files, `crash.txt` and the last network-log lines, with names, repos, the login and folder paths replaced. Then reveal it, with an optional prefilled GitHub issue link. **Decide (Q4)** how it's sent. |

## D. Security and privacy (no critical or high findings)

| ID | Sev | Size | Finding | Proposed fix |
|---|---|---|---|---|
| D1 ✔ | P1 | S | **Error reports can leak folder names.** `scrub` blanks only words containing `/`, so a name like `My Secret App` leaks, and macOS errors quote file names. | Send a category plus an error code, never free text, or add every folder component to the private words. **Decide (Q5).** |
| D2 | P1 | S | **PostHog geolocates each report by IP.** | Add `$geoip_disable: true`. |
| D3 ✔ | P1 | M | **Signing secrets are exposed to every npm script and `build.rs` in the release job.** | Build unsigned in one job, then sign and notarise the artifact in a separate job. |
| D4 | P2 | S | **The overlay can emit events to other windows** (it can re-route Adopt), and shift-click syncs can be spammed. | Grant the overlay listen-only event permissions, and debounce click syncs. |
| D5 | P2 | S | **`close_window` closes any window by label.** | Let it close only the calling window. |
| D6 | P2 | S | **`adopt` trusts any folder string.** | Require the folder to be under a bookmarked or watched folder. |
| D7 | P2 | S | **The PostHog host check can be bypassed:** `https://evil.com?.posthog.com` passes. It needs repo-config access. | Parse the URL and compare the host. |
| D8 | P2 | S | **The Keychain item uses default attributes and the legacy keychain.** The token isn't zeroised in memory. | Use the data-protection keychain with ThisDeviceOnly, and `Zeroizing<String>`. |
| D9 | P3 | S | **GitHub response bodies have no size cap.** | Cap them at a few MB. |
| D10 | P2 | S | **Delete everything may miss `Library/HTTPStorages`, `Caches/<id>` and the prefs plist** (suspected). | Remove them too. |
| D11 | P3 | S | **Nothing checks that the token is really read-only.** | Warn if `/user` reports write permissions. |

## E. UX

| ID | Sev | Size | Finding | Proposed fix |
|---|---|---|---|---|
| E1 ✔ | P0 | S | **Folder-only users see "⚠ No GitHub token yet" on every launch and wake.** | With no account, skip the GitHub sync quietly and show "Folders only". |
| E2 | P1 | S | **⌥-click is never taught inside the app.** | Add a line to the last welcome screen and the adoption result, plus a disabled hint in the menu. |
| E3 | P1 | S | **The card is reachable only by ⌥-clicking**, so it isn't keyboard- or VoiceOver-accessible. | Add "Open card…" to each pet's submenu. |
| E4 | P1 | S | **Adopt shows "No GitHub repos" during the first sync.** | Redraw on state changes, and show "Fetching your repos…" meanwhile. |
| E5 | P1 | S | **Error items have nothing to click to fix them.** | Add "Paste a new token…" when the token is refused, show "moods are on hold", a neutral (not ⚠) offline state, and the rate-limit reset time. |
| E6 | P1 | S | **"Mark release" has no confirm or undo.** | Add a few seconds of Undo (same pattern as Let go). |
| E7 | P2 | S | **The card doesn't say what to do.** | Add a one-line next action: "A commit or push cheers them up." |
| E8 | P2 | S | **Some copy leans towards guilt** ("Still here, still waiting by the window", "Hungry 😿"). | Reword it. **Decide (Q7).** |
| E9 | P2 | M | **Little can be changed after adoption.** Colour and the tracked repo are fixed, and "Let go" and "Hide for today" are only in the menu. | Add those to the card. |
| E10 | P2 | S | **Screen readers and errors expose raw setting keys** ("maxVisible must be…"). | Use the visible label in both. |
| E11 | P2 | S | **Stale or misleading copy:** the welcome screen says "Sound and launch at login arrive later", About leaves out the opt-in error reports, and the "Focus" and "Only on Wi-Fi" placeholder rows are still there. | Fix the text and hide the placeholders. |
| E12 | P3 | S | **Inconsistent names:** "Parrot (macaw)", "parrot", "Parrots"; swatches labelled "Colour 1–4". | Use one name per species, and name the colours. |
| E13 | P3 | S | **Primary-button contrast is about 3.6:1**, and the "Click again" confirm isn't announced. | Darken the blue, and add `aria-live`. |
| E14 | P3 | S | **Reduce Transparency is never checked.** | Make the bubbles opaque when it's on. |

## F. 3D parity with the Rive pets

| ID | Sev | Size | Finding | Proposed fix |
|---|---|---|---|---|
| F1 ✔ | P0 | M | **Only 3 of the 5 moods are visible.** Hungry draws the same as Gloomy, and Peckish the same as Content (`pet.ts:500`). | Give each mood its own pose and motion. |
| F2 | P0 | L | **No props in 3D:** bowl, rain cloud, bed, Zzz, postcard, confetti. | Model them the way the stand perch is done. **Decide (Q3).** |
| F3 | P0 | M | **Graduates get no accessory**, which breaks the reward. | Model the accessories (e.g. cap, scarf, bow). **Decide (Q3).** |
| F4 | P1 | M | **The adoption preview is the 2D budgie and Rive art.** | Render the preview with `Pet3DRig`. |
| F5 | P2 | M | **Exiting is a shrink, not walking off-screen**, and the celebration is the same for every species except the macaw's wing flap. | Add a walk to the nearest edge, and per-species celebrations. |
| F6 | P2 | S | **The hit ellipse was tuned to the Rive cat** (`overlay.rs` `hit`). | Re-fit it per 3D species. |

## G. Planned features that were never built

| ID | Sev | Size | Feature | Status |
|---|---|---|---|---|
| G1 | P1 | M | **⌥-drag onto a screen grid.** Home spots kept across restarts and resolution changes, and a faint grid with the target cell lit. | **Built, uncommitted** on `feature/drag-grid`. Rust tests pass. Real-app test: 5 of 6 checks pass (the failure is in my test script). Includes a "Put pets back in line" menu item. **Decide (Q8).** |
| G2 | P2 | M | Petting gesture (⌥ hold 1 s) | Not built |
| G3 | P2 | S | ⌥-right-click context menu | Not built |
| G4 | P3 | M | Big moves (walk or stretch every 2–5 h) | Not built |
| G5 | P2 | S | Custom snooze length, a pause-until date, a release version label | Not built |
| G6 | P2 | S | UI for per-pet mood thresholds (the field already exists) | Not built |
| G7 | P2 | S | A "slot opened, adopt?" prompt, and multi-select adoption on first run | Not built |
| G8 | P3 | M | One token per GitHub owner | Not built |
| G9 | P3 | S | Wi-Fi-only sync, the ⌃⌥S global shortcut | Not built |
| G10 | P3 | S | Folder-only pets default to Slow-burn, and expiry bubbles at 7 days and 1 day | Not built |
| G11 | P3 | S | Increase Contrast support, `cargo deny` in CI | Not built |

## H. Dead code, duplication and naming

| ID | Sev | Size | Finding | Proposed fix |
|---|---|---|---|---|
| H1 | P2 | S | **Two frame systems:** a global rAF patch for Rive and a separate policy in each 3D rig, plus optional `continuous` / `setLiveliness` extras on `Rig`. | One `setPolicy` on `Rig`, with the patch only around Rive (comes with A1). |
| H2 | P2 | S | **`native-overlay/` (the Swift fallback from Gate 0) is unused.** | Delete it, or move it to `tools/` and document it. **Decide (Q8).** |
| H3 | P2 | S | **`proto3d.html` and `src/proto3d` are debug-only** (used for CPU runs). | Move them under `tools/` or keep them and document them. |
| H4 | P2 | S | **Naming:** the `cat` field and variable now hold any species (`rig.ts:19`, `main.ts:210`), and `sdf.ts` is still headed "prototype". | Rename to `pet`. |
| H5 | P2 | S | **The grid cell maths exists in both Rust and TS** (new in G1). | Have Rust send the taken cells, so TS only draws them. |
| H6 | P3 | S | **Budgie leftovers:** `art/parrot/make.py`, `art/STYLE.md`, the audio credits and `make.py`, `gate-3.md`. | Rename to macaw where it's still live; leave historical records alone. |
| H7 | P3 | S | `state.rs` (1,375 lines) is fine for now. | Split out the folder scan and the network log if it grows. |

## I. Tests

| ID | Sev | Size | Finding | Proposed fix |
|---|---|---|---|---|
| I1 | P1 | M | **pet3d has no unit tests:** none for the mesher, frame policy or mood mapping. | Add vitest-style tests (needs your OK for a dev dependency) or Playwright page tests. **Decide (Q8).** |
| I2 | P2 | S | **The overlay Playwright tests use fixed waits**, and there is still the known flaky graduate test. | Wait on events instead. |
| I3 | P2 | S | **The sync tests need `python3`.** | Skip them with a message when it's missing. |
| I4 | P2 | S | **The new `drag-test.sh` uses Finder to find the screen size, which timed out.** | Ask the app for the size instead. |

## J. Stale docs

**J1 (P2, M):**
- **spec.md:** still says "no telemetry", Rive as the renderer, budgie, and that a missing repo becomes Let go automatically.
- **README:** the roadmap, architecture section and hero image are from the Rive era.
- **User guide:** the accessory claim, where "Show log" is, and the 7-day and 1-day reminders.
- **gate-2b:** "expiry reminder not built".

**Proposed fix:** update all of these in the same PR as the code each one describes.

## K. Deferred by your earlier decision (for reference)

- M4: multiple displays, Focus, deferred wakes, hover-safe rotation, summary bubble.
- Apple signing and notarisation.
- The clean macOS 14 VM test.
- The game world.

---

## Decisions (2026-11-04)

- **Q1, frame rate (A2):**
  - Locked to the display refresh, following Liveliness:
    - Calm: 30 fps moving, 20 fps idle.
    - Gentle: 60 moving, 30 idle.
    - Lively: 60 moving, 60 idle.
    - About 10 fps asleep in every mode.
  - The default becomes **Gentle** (it was Calm).
  - Liveliness becomes a per-pet override on the card: "App default", or one of the three.
  - The Settings CPU text gets rewritten from new measurements (A11).
- **Q2, contexts (A3):**
  - Keep one WebGL context per pet and patch it: A4 (release contexts) and A5 (hide pets for full-screen apps instead of rebuilding them).
  - Above about 12 visible 3D pets, the extra pets draw in 2D, and that is shown as an issue.
  - No shared renderer.
- **Q3, props (F2/F3):** build every prop and accessory in 3D.
- **Q4, sending the log (C15):**
  - "Save log for a bug report" writes one scrubbed file, reveals it in Finder, and offers a prefilled GitHub issue.
  - Also a "Send to developer" button that uploads the scrubbed log (last ~500 lines) to PostHog. It's available once the PostHog project exists.
  - Assumption to confirm: before uploading, the button shows exactly what will be sent.
- **Q5, error reports (D1):**
  - Automatic reports carry a **category and error code only**, never free text.
  - `$geoip_disable` is set (D2).
  - The log upload in Q4 happens only when the user chooses to send it.
- **Q6, backups (B6):** not now, after v1.
- **Q7, copy tone (E8):** keep it as it is. E7 (the next-action hint on the card) still stands.
- **Q8, housekeeping:**
  - Keep drag-to-grid as built (G1).
  - Delete `native-overlay/` (H2).
  - Add vitest as a dev dependency (I1). You install it, because the agent guard blocks dependency installs.
  - Order of work as proposed below.

## What already holds up (no change proposed)

- **Token:** never reaches a webview; requests are pinned to `api.github.com` with redirects off.
- **No processes:** no shell or git process is ever started.
- **Debug switches:** the debug environment variables are compiled out of release builds.
- **Permissions:** each window has its own capabilities.
- **No HTML sinks:** the UI is built with `el()` and `textContent`.
- **CI:** actions pinned to SHAs, no secrets on fork PRs.
- **Sync loop:** one thread that drops the lock for I/O, with backoff and rate-limit handling.
- **Fairness:** a failed sync never makes a mood worse.
- **Store:** atomic, and corrupt files are set aside.
- **Privacy:** everything is opt-in, with the payload shown.
- **Comfort settings:** nudge caps and Reduce Motion are respected.
- **Screen capture:** pets are excluded from it.

## Proposed order of work, once you approve (one small PR each, stacked on #62)

1. **Make 3D shippable:** A1+A2+H1, then A3+A4+A5, then A6+A7+A8, then a CPU re-measure with numbers.
2. **Errors and log:** B1–B4, then C1–C10, C13, C14, then C15.
3. **3D parity:** F1, then F3, then F2, then F4.
4. **Drag:** G1 (finish, tidy H5, fix I4, update the docs).
5. **UX:** E1–E6.
6. **Security:** D1–D3, then D4–D11.
7. **The rest of P2 and P3**, in the order you pick.

**Merging:** #61 and #62 stay unmerged until step 1 is done and measured.
