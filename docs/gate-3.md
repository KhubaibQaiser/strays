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
