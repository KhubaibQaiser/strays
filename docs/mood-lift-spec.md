# Mood lift: readable emotions for the 3D pets

Status: draft for review (2026-11-05). Nothing is implemented yet.

## Problem
Each mood is meant to be readable at a glance, at the pet's real on-screen size (about 100–170 pt). Today it isn't.
- **What each mood changes now** (`pet.ts`, the `LOW` table):
  - A small head and ear drop.
  - Half-shut lids.
  - Pupil size: bigger when joyful, smaller when gloomy.
  - Tail height.
  - Glance timing.
  - Props: the empty bowl for peckish and hungry, a small cloud for gloomy.
- **What you see at full-body size:** joyful and content look the same, and peckish, hungry and gloomy differ mostly by their props. A close-up shows the eyes do change, but the change vanishes at normal size.
- **No mouth on the cat** (or the hamster or macaw). Only the corgi has one: a mouth line, plus a tongue for panting.
- **The video is not the cause.** I checked recorded frames against a face close-up, and the differences are simply too small.

## Goal
Each mood must be recognisable from its silhouette and face alone, without the props, at 120 pt. The props still help, but they shouldn't be needed.

Two constraints:
- **CPU:** stays within the current per-pet budget. All the new motion is bone and uniform changes plus a few small meshes, with no extra draw passes.
- **Reduce Motion:** pets keep the pose for their mood, but skip the extra motion (bounces, sighs, tail flicks).

## The moods

| Mood | Body | Face | Motion | Props |
|---|---|---|---|---|
| **Joyful** | Sitting tall, chest up, ears forward | Happy crescent eyes (lower lid curves up), an open smile | A little bounce every few seconds, quick tail flicks with the tip curled high, a playful head tilt, purring | — |
| **Content** | Relaxed sit | Soft eyes, slow blinks, a small closed smile | Slow sway, gentle glances (today's idle, slightly calmer) | — |
| **Peckish** | Sitting, leaning toward the bowl | Neutral mouth, a lick of the lips (tongue flick) | Looks at the bowl often, taps it with a front paw | Empty bowl |
| **Hungry** | Crouched low, half lying, beside the bowl | Sad, pleading eyes (inner lid corners raised), a slightly open mouth | Stares at the bowl, an occasional silent "meow" (mouth opens), ears out sideways | Empty bowl, nudged |
| **Gloomy** | **Lying down**, chin on paws (per species, below) | Droopy lids, a downturned mouth, small pupils | A slow heavy sigh now and then, otherwise very still; ears flat; tail limp | A bigger, darker cloud with a visible steady shower, and a small puddle |

### Lying down, per species
- **Cat:** a "sphinx" loaf. The body sinks to the floor, the front legs stretch forward, and the chin rests on the paws. The tail wraps round the side.
- **Corgi:** belly down, front legs forward, head flat on the paws, ears back.
- **Hamster:** a flattened "pancake": body squashed low and wide, paws tucked, head down.
- **Macaw (stays on its perch):** the head turned down and tucked toward the chest, feathers fluffed out (body scaled wider), tail drooping.

## The mouth (new)
- A small mouth for the cat, hamster and macaw (the macaw's is a beak open and close), plus a new expression set for the corgi's existing mouth.
- Four expressions: smile, neutral, frown and open (meow or yawn), blended smoothly.
- The cat's mouth is the classic inverted "Y" line under the nose. It curves up into a smile, droops into a frown and opens into a small dark mouth with a pink tongue.
- It's built like the whiskers: thin curves plus one small mesh on the head bone, so it costs almost nothing.

## How it's built (engine)
- **Poses:** a per-mood target pose for the hips, spine, legs, neck, head and tail, blended with the existing springs, so changes are smooth (about 1.2 s down, 0.8 s up).
- **Species-specific poses:** each species file gets a `poses` entry for `low` (hungry crouch) and `down` (gloomy lie).
- **Clipping at the floor:** the coat, eyes and mouth stop drawing below y = 0, so a body that sinks to the floor doesn't poke through. This is a small shader change.
- **Eye shapes:** two new uniforms. "Smile" raises and curves the lower lid; "sad" raises the inner corners. Moods set them; blinks still work.
- **Actions:** actions (eat, celebrate, sleep, petting) still override mood poses. A gloomy pet that's fed sits up to eat, then lies back down.
- **Fallback:** the 2D Rive art (used only without WebGL) is unchanged in this work.

## Tests and review
- **Unit tests:** pose targets per mood are inside joint limits, and blends finish in time.
- **Recorded preview:** each species × each mood, plus the transitions content → gloomy and gloomy → eat → gloomy. Recorded with the existing script and sent to you as GIFs or MP4 **for approval before the video work resumes**.
- **CPU:** measured as before (one cat, Gentle) and reported against today's numbers.
- **README:** the mood GIFs are re-recorded.

## Delivery
- One PR off `main` (`feature/mood-lift`), split into commits per part: engine, cat, corgi, hamster, macaw, then props.
- If it grows past a comfortable review size, it becomes 2–3 PRs: engine and cat first, then the other species.

## Decisions (2026-11-05)
1. **Sad music:** in the video, and also in the app. While a pet is gloomy, the app can play a soft, quiet sad loop.
   - Off by default, with a Settings switch ("Sad music while a pet is gloomy").
   - Plays at most once at a time, however many pets are gloomy.
   - Fades in over 3 s and out when no pet is gloomy.
   - Never plays while sounds are muted, during Focus, or while the overlay is hidden.
   - The loop is a new CC0 audio asset, credited in `assets/audio/CREDITS.md`. You approve the download (see open question below).
2. **Hungry crouches; gloomy lies down** (as proposed).
3. **Joyful sparkle:** a small heart or sparkle floats up above a joyful pet every 10–20 s (fades out after about 1.2 s). With Reduce Motion it's off.
4. **Mouth:** the drawn line mouth (as proposed).

## Open question
- **Where the sad loop comes from:**
  - **(a)** I search Freesound and OpenGameArt for CC0 sad or melancholy loops and send you 2–3 candidates to approve, the same way the current pet sounds were chosen.
  - **(b)** You provide a track.
