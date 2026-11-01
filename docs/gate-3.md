# Gate 3 — content

Gate 3 lands one species per PR (user decision). Each one needs the user's sign-off (spec §14.1).

## Cat (Persian) — signed off by the user, 2026-11-01

**Rig:** `art/cat/make.py` generates `art/cat/scene.rml`, using shared helpers in `art/tools/rml.py`. It follows `art/STYLE.md`:
- 400 × 400 artboard; the `PetLook` view model holds the four coat colours plus `mood` and `action`.
- Four ambient loops run under every pose: breathing, blinking, tail sway, ear twitch.
- A pose layer plays one of 5 mood poses or 10 clips. 215 transitions connect them, each conditioned on the two numbers.

| Mood poses | |
|---|---|
| Joyful | happy eyes, tail up, a little hop |
| Content | calm rest |
| Peckish | the empty bowl appears; it glances at it |
| Hungry | lies low beside the bowl, eyes half closed, ears down, sighs |
| Gloomy | sad brows, downcast eyes, small frown, ears flat, rain cloud |

| Clips (`action`) | |
|---|---|
| 1 eat | the bowl fills, it nibbles, the bowl empties, happy eyes |
| 2 petted | happy eyes, head tilt, ears soften, hearts rise |
| 3 celebrate | zoomies, then sits proudly; confetti |
| 4 exit / 5 enter | scoots off to the right / in from the right |
| 6 fall asleep | yawn, lies down, eyes close, the bed appears |
| 7 sleep | loop with Zzz |
| 8 wake | eyes open, stretch, yawn, the bed goes |
| 9 postcard | walks in carrying a postcard, then sets it down |
| 10 walk | scoots in place |

**Props in the rig:** bowl (empty or full), bed, rain cloud, postcard, confetti, Zzz, hearts.

**Colours:** shade, highlight and outline are derived from the coat colour in OKLCH (§10.4). The outline reaches at least 3:1 contrast for every curated look and for 500 random colours (`tests/look.spec.ts`).

**In the app:**
- The overlay sets `mood` from the pet's mood.
- The core queues cues, which the overlay plays as clips:
  - **eat:** on new activity, the next time the pet is on screen
  - **celebrate:** on a release, then **exit** if it goes on holiday
  - **fall asleep:** on snooze, then fade out
  - **sleep → wake:** when a snooze ends
  - **postcard:** back from holiday
- A leaving pet stays on screen until its clip ends.
- With Reduce Motion, only fades play.

**Review:**
- `art/review/cat/*.png`: 12-frame sheets of every pose and clip, made by `python3 art/tools/review.py art/cat`.
- `celebrate.gif` and `joyful.gif`.
- `moods-in-app.png`: three cats in the real overlay page.

**Tests:**
- `tests/overlay.spec.ts` runs the real overlay page with the Rive runtime. A release cue celebrates, then the pet leaves once its clip ends; with Reduce Motion it only fades.
- All 13 Playwright tests pass.
- 42 core tests and 7 app tests pass; clippy reports no warnings.

**Not covered:**
- Petting: the ⌥ press-and-hold gesture isn't built, so the clip exists but nothing plays it yet.
- Sounds: they come in the audio PR.
- Exits and entrances always go right; walking to the nearest edge comes with home spots.
- The cat faces the viewer while it scoots.

## Dog (Pembroke Welsh Corgi) — signed off by the user, 2026-11-01

**Rig:** `art/dog/make.py` writes `art/dog/scene.rml`. The poses, clips and props come from `art/tools/pet.py`, which was factored out of the cat script. The cat's output didn't change; I checked it by rendering three frames.

**Drawing:**
- Fox-like face with tall pointed ears.
- White blaze, muzzle, bib and socks. These are fixed white, because they're part of the breed, so they don't follow the coat colour.
- Short legs, shaded haunches, and a short fluffy tail on a 3-bone chain.
- The head sits a little lower than the cat's, and the ears barely flop except when hungry or gloomy.

**Celebration (§10.3):** two quick spins (flips with hops), then a play bow with a wagging tail, then the shared proud sit with confetti.

**Moods and clips:** the same five poses and ten clips as the cat, fitted to the corgi's measurements.

**In the app:** `src/art.ts` maps each species to its rig, for both the overlay and the adoption preview. The hamster and parrot still fall back to the cat.

**Review:** `art/review/dog/*.png` (every pose and clip), `celebrate.gif`, `joyful.gif`, and `with-cat-in-app.png` (a cat and a corgi in the real overlay page).

**Tests:** the overlay test draws a cat and a corgi together, and both rigs load. Playwright passes 14/14. The app build bundles both `.riv` files.

## Hamster (Syrian)

**Rig:** `art/hamster/make.py` writes `art/hamster/scene.rml`, using the shared `art/tools/pet.py`.

**Drawing:**
- A round body with the head sitting right on it (no neck), and small round ears.
- Cream belly, muzzle and cheek pouches, plus whiskers. The cream is fixed, like the corgi's white, so it doesn't follow the coat colour.
- Pink feet and little hands held at the chest.
- A tiny stub tail on the shared 3-bone chain, mostly hidden behind the body.

**Celebration (§10.3):** stuffs its cheeks in three gulps (the pouches grow to 1.38×), hops four times with full cheeks, then the shared proud sit with confetti. The cheeks empty at the end.

**Moods and clips:** the same five poses and ten clips, fitted to the hamster's measurements. Its eating reach is longer than the cat's, so the lower head still meets the bowl.

**In the app:** `src/art.ts` maps `hamster` to its rig. Only the parrot still falls back to the cat.

**Review:** `art/review/hamster/*.png` (every pose and clip), `celebrate.gif`, `joyful.gif`, and `with-cat-and-corgi-in-app.png` (all three in the real overlay page).

**Tests:** the overlay test now draws a cat, a hamster and a corgi together, and all three rigs load. Playwright passes 14/14.

## Parrot (budgerigar)

**Rig:** `art/parrot/make.py` writes `art/parrot/scene.rml`, using the shared `art/tools/pet.py`.

**Drawing:**
- A green budgie, with the coat colour on the body, wings and tail.
- Fixed parts: a yellow face mask, a barred crown, blue cheek patches, throat spots, a hooked horn-coloured beak and a blue cere.
- Barred wings and a long tail on the 3-bone chain. The tail is three rigid feather segments, one per bone, hanging down behind the body.
- It sits on its own perch, which is part of the rig and stays put when it flies.

**Fly and perch (§10.2):** the parrot keeps the same action ids, but four clips are flights instead of walks:
- **Enter (5):** flies in from the right and lands on the perch with a squash.
- **Exit (4):** crouches, takes off and flies out.
- **Postcard (9):** flies in carrying the postcard in its beak.
- **Walk (10):** two fluttering hops in place.

**Other differences:**
- The shared ear poses drive the wings, so joyful lifts them slightly and gloomy tucks them in.
- It sleeps fluffed up on the perch, so the bed never shows.
- Its tail sway is gentler than the other pets'.

**Celebration (§10.3):** takes off, flies a full loop (turning with the loop, so it stays in frame), lands, fluffs its feathers twice, then the shared proud sit with confetti.

**In the app:** `src/art.ts` maps `parrot` to its rig. All four species now have their own art, and the adoption page's "still being drawn" note is gone. Placement already perches the parrot top-right under the menu bar (Gate 2a).

**Review:** `art/review/parrot/*.png` (every pose and clip), `celebrate.gif`, `joyful.gif`, `fly-in.gif`, and `all-four-in-app.png`.

**Tests:** the overlay test draws all four species together, and all four rigs load. Playwright passes 14/14. The app build bundles all four `.riv` files.

## Fix: linear keys were holding (all species)

`interpolationType` on a Rive keyframe defaults to **hold**. `art/tools/rml.py` wrote "linear" keys with no type, so they held:
- Exit and enter stood still and then jumped, instead of walking across.
- The rain drops and Zzz didn't drift.

The parrot's loop flight showed it. `rml.py` now writes `interpolationType="linear"`. I regenerated all four rigs and re-rendered the affected sheets (exit, enter, postcard, gloomy, sleep, walk) for the cat, corgi and hamster.

## Accessories (graduation rewards)

**Decision (user, 2026-11-01):** the graduating pet wears its accessory. No other pet does in v1. Recorded in spec §5.8.

**Art:** five accessories in `art/tools/pet.py`, drawn in the fixed palette and fitted to each species with new `Fit` measurements (`crown`, `chin`, `head_w`, `chest`):
- graduation cap with tassel
- scarf
- bow
- star badge
- flower crown

A new `accessory` view-model number (0–5) drives its own state-machine layer, so an accessory shows with any pose or clip. `art/review/accessories.png` shows every pet wearing every accessory.

**Core:** `Pet::graduate` sets `accessory`, picked from the pet's id, so it's fixed and never changes on later loads (`ACCESSORIES` in `pet.rs`).

**App:**
- A new `Cue::Graduate { accessory }` is sent when you graduate a pet, and when sync graduates one because its repo was archived.
- The overlay sets the rig's `accessory`, plays celebrate, then exit, and removes the pet. With Reduce Motion it fades out wearing the accessory.
- The menu-bar Graduates list adds the accessory to each graduate's name (for example "🐈 Miso · 🎓 graduation cap").

**Fix:** the overlay gave the celebration 4.7 s, which cut off the end of the corgi's (5.0 s) and parrot's (5.5 s) celebrations. It now waits 5.8 s.

**Tests:**
- Rust: a core test checks the accessory is set and stays the same for the same pet; an app test checks the cue's accessory number and JSON shape.
- Playwright: a graduate puts on its accessory, celebrates and leaves (`art/review/graduate-in-app.png`). 15/15 pass.

## Icons and writing

**Icons (§10.2 UI):** drawn in RML like the pets (`art/icons/make.py`) and rendered by `art/icons/render.sh`. `rive --screenshot` always writes an opaque background, so the script turns brightness into alpha with ffmpeg.
- **App icon:** a ginger cat face on a teal rounded square, on the macOS 824/1024 grid. `npx tauri icon` turned it into `app/src-tauri/icons/*`; the generated Android and iOS sets were removed. Preview: `art/review/app-icon.png`.
- **Menu-bar icon:** a paw template image (`icons/tray.rgba`, 44 px raw RGBA, so no extra Tauri feature is needed), so macOS tints it for light and dark menu bars. The badge text next to it is now just the number of hidden needy pets, or ⚠ when syncing fails. Preview: `art/review/menu-bar-icon.png`.
- **Not verified:** what it looks like in the real menu bar. A screen capture from the terminal came back black, because the terminal has no Screen Recording permission.

**Writing (§10.5):** the nudge lines moved from code into one strings file per style, `app/core/strings/gentle.json` and `playful.json`. Each has Hungry lines, separate Gloomy lines (new), and the "taking a break" prompt. A core test checks every line for blaming words and unknown placeholders.
