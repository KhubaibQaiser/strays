# Strays — v1 Specification

| | |
|---|---|
| Status | Draft for review |
| Date | 2026-10-30 |
| Owner | Khubaib Qaiser |
| Platforms (v1) | macOS 14+ (Apple Silicon + Intel). The code stays cross-platform; Windows comes later. |
| Repo | Private for now. Licence to be decided. |
| Price | Free. The design must not block paid cosmetics later. |

> **Convention:** a value marked **(default)** is a starting guess the user can change in settings. A value marked **(target)** is a budget we measure against. Neither is based on data yet.

---

## 1. Summary

Strays is a calm desktop companion. Every unfinished project you choose to track gets a small animated pet that lives on the edges of your screen. Working on the project (pushing commits) keeps the pet happy. Neglect makes it gradually sadder. Shipping a release sends it on a short holiday. Finishing the project "graduates" it and frees a slot for a new one.

**Who it's for:** developers who start many projects and finish few.

**Story:** *The Unfinished Shelter.* Every half-built project leaves a stray behind. Starting a project gives a stray a temporary home; finishing it gives the stray a forever home.

### 1.1 Goals
1. Keep a few unfinished projects visible, without distraction.
2. Motivate finishing, not just activity.
3. Earn trust: local-first, minimal permissions, every permission and network call visible.
4. Be buildable and verifiable almost entirely by AI agents (§14).

### 1.2 Non-goals (v1)
- Game world / full-screen app (deferred; §17)
- Syncing pets across devices
- Windows release, mobile apps and widgets
- Automatic release detection
- Hosts other than GitHub for remote activity (local git works with any host)
- Accounts, telemetry, payments

---

## 2. Principles

1. **Never punish the user for things outside their control.** Offline time, an expired token, GitHub being down, or the Mac being off never make a pet sadder.
2. **Pets never die and never age.** Only their mood changes.
3. **Calm by default.** Strict limits on nudges, sound off, respect for Focus mode and quiet hours, a cap on visible pets.
4. **Least privilege, visible privilege.** Everything is opt-in, explained in plain language, revocable in one click, and logged.
5. **Only work feeds pets.** Petting or playing gives a momentary reaction and never changes mood.
6. **Deterministic and testable.** All logic is a pure function of stored data plus an injectable clock.

---

## 3. Platform and stack

| Concern | Choice |
|---|---|
| App shell | **Tauri 2** (Rust core + system webview) |
| Front end | TypeScript, Vite. No UI framework unless one proves necessary. |
| Animation | **Rive**: authored as RML text by agents through the Rive CLI, compiled to `.riv` in CI, played by the official web runtime (`@rive-app/canvas` or `webgl2`). Its WASM is bundled locally, never loaded from a CDN. |
| Native macOS APIs | Rust through `objc2` crates: window behaviour, cursor/modifier state, screens, capture exclusion, bookmarks |
| Audio | Native playback in Rust (`rodio`), never in the webview |
| Git (local) | `gix` (pure-Rust git library). Reads commit dates and tags only. No `git` binary needed. |
| HTTP | `reqwest` in Rust, behind a host allowlist |
| Secrets | macOS Keychain (`keyring` crate) for release builds. Dev builds use the file at `~/.config/strays/dev-token`. |
| Storage | JSON files in the app container, written atomically |
| Sandbox | macOS App Sandbox enabled (Developer ID distribution) |

**Minimum OS:** macOS 14 Sonoma (user decision, Gate 0). It keeps frame-rate control available and removes the need for an older-OS test matrix.

**Placeholder bundle ID:** `com.khubaibqaiser.strays`.

---

## 4. Glossary

| Term | Meaning |
|---|---|
| **Pet** | A tracked project's companion. Has a species, colours, name and state. |
| **Activity** | Evidence of work on the project: a push seen on GitHub, or a local commit. |
| **Last activity** | The latest activity timestamp from all sources. It only moves forward. |
| **Mood** | Calculated from idle days. One of 5 levels (§5.2). |
| **Slot** | A pet's place on the active roster. Default 3 (configurable). |
| **Shelter** | The list of tracked projects waiting for a free slot. They're tracked, but not shown on screen. |
| **Snooze** | Hide and silence a pet for a while. Hunger keeps counting. |
| **Pause** | Freeze a project's mood (a deliberate break). |
| **Holiday** | Time away after a release. Mood is frozen. |
| **Graduate** | The project is finished. The pet retires and its slot is freed. |
| **Let go** | The project is abandoned on purpose. The pet leaves, its slot is freed, and there's no reward. |

---

## 5. Game mechanics

### 5.1 Pet states

```
             adopt (free slot)                 mark release (§5.5)
 [Shelter] ───────────────────► [Active] ─────────────────────► [Holiday] ──(N days)──► [Active]
     ▲                             │  ▲
     │                             │  └── wake / snooze ends ── [Snoozed]
  new repo                         │  └── resume ────────────── [Paused]
  (or manual add)                  ├──► [Graduated]  (mark finished, or GitHub repo archived)
                                   └──► [Let go]     (user choice, or repo deleted)
```

- **Active** pets may appear on screen (subject to §5.7). Snoozed, Paused and Holiday pets never do.
- **Graduated** and **Let go** pets are permanent records, listed in the menu-bar menu.
- If a repo is deleted on GitHub, its pet moves to **Let go** with the reason "Repo no longer exists". The user can undo this within 30 days **(default)**.

### 5.2 Mood

`idleDays` is calculated as in §5.3. Thresholds are upper bounds in days:

| Level | Default up to | Pose / behaviour | Speech bubbles |
|---|---|---|---|
| 1 Joyful | 2 | Playful idle, occasional happy hop | None |
| 2 Content | 5 | Calm idle | None |
| 3 Peckish | 10 | Glances at its empty bowl | None |
| 4 Hungry | 20 | Lies down, slow breathing, sighs | ≤ 1 per day |
| 5 Gloomy | (beyond) | Sits facing away, small rain cloud | ≤ 1 per day; this is the floor |

**Configurable** (Settings → Moods): global thresholds; per-pet override; presets **Sprint** (×0.5), **Normal** (×1, default), **Slow-burn** (×2), or **Custom**. Thresholds must strictly increase; the UI enforces this. A live preview shows each pose. Option: **"Don't count weekends"** (default off). Saturday and Sunday in local time are excluded from `idleDays`.

### 5.3 Calculating idle days (the fairness rule)

```
evalTime     = now                             for local-only pets
             = lastSuccessfulCheck(source)     for GitHub-backed pets
idleSpan     = evalTime − lastActivity
frozen       = total of holiday + pause intervals overlapping [lastActivity, evalTime]
excluded     = weekend time in the span, if "don't count weekends" is on
idleDays     = max(0, idleSpan − frozen − excluded) / 24h
```

- When a GitHub sync fails (offline, token expired, rate limited), `lastSuccessfulCheck` stays put, so the mood **cannot get worse**.
- When the Mac wakes after a long sleep, mood is recalculated once and **one** summary bubble is allowed, not a burst.
- If the system clock moves backwards (`now < lastActivity`), `idleDays` = 0 and the anomaly is logged.

### 5.4 Activity sources

**Rule: `lastActivity` = the maximum over every source linked to the pet. It never decreases.**

| Source | What's read | Notes |
|---|---|---|
| GitHub `pushed_at` | Repo metadata through a fine-grained token with **Metadata: Read-only** | The source of truth for repos on GitHub. Counts pushes to *any* branch, including bots and teammates (accepted in v1, §17). |
| Local commits | Committer date of the latest commit on any local branch | Catches work before it's pushed |
| Local remote-tracking refs | Committer date of the commits under `refs/remotes/*` | Sees other devices' work only after the user runs `fetch`/`pull` |

- **The app never runs `git fetch`** and never touches git credentials or SSH keys.
- **A local folder with no GitHub match** shows the badge *"This Mac only. Work from other devices won't count."* and defaults to the **Slow-burn** pace.
- **Matching a local folder to a GitHub repo:** parse `remote.*.url` (https, ssh, scp-style), resolve host aliases from `~/.ssh/config` (only if the user grants access to that file: the App Sandbox blocks it otherwise, and an unresolved alias falls back to asking), apply git `url.*.insteadOf` rewrites, and match `owner/name` case-insensitively. Ambiguous or unmatched → **ask the user to confirm**.

### 5.5 Releases → holiday

- **v1 trigger:** **"Mark release…"** only, from the info card or context menu. Optionally enter a version label.
- **Eligible only if** there's been new activity since the previous release **and** the last holiday was ≥ 14 days ago **(default)**. If not eligible: a small celebration plays, with no holiday, and a tooltip explains why.
- **Sequence:** celebration (~5 s, species-specific: §10.3) → bubble "Shipped {version}! Taking a little holiday 🏖" → the pet exits at the nearest screen edge → **Holiday** for 3 days **(default; configurable 1–14)** → it returns walking in with a postcard. The info card shows "Back from holiday after {version}".
- Mood is frozen during the holiday (§5.3).
- The release is recorded in the pet's history (`releases[]`).

### 5.6 Snooze and pause

| | Snooze | Pause |
|---|---|---|
| Purpose | "Not now" | "I'm deliberately taking a break from this project" |
| On screen | Hidden | Hidden |
| Bubbles and sound | Silenced | Silenced |
| Hunger | **Keeps counting** | **Frozen** |
| Durations | 1 h · 4 h · until tomorrow (09:00 local, **default**) · 3 days · custom · until I wake it | Until resumed. Optional resume date. |
| Where it's shown | Menu bar → 💤 Snoozed (until …) | Menu bar → ⏸ Paused (since …) |

**Going to sleep (snooze):** yawn → curl up → small bed fades in → exits at the nearest edge (~3 s).

**Waking up (snooze ends):**
1. The pet appears at its home spot **already asleep**, with a soft "Zzz", for about 60 s.
2. It wakes with a stretch and a yawn; one quiet wake sound if sound is on.
3. Bubble: "I'm back. Shall we work on *{repo}*?" It disappears after ~8 s and needs no click.

**Deferred** if the wake time falls in quiet hours, Focus, Presentation mode or a full-screen app. If several pets wake together, they come back 3 minutes apart **(default)**. "Wake now" is in the menu-bar menu.

### 5.7 Display rules (keeping the screen calm)

- **Max visible pets:** 3 **(default; 1–8)**. **Min visible pets:** 1 **(default; 0–max)**.
- **Priority among Active pets:** neediest first (highest mood level). Ties go to the longest since last shown. **At most 1 Gloomy pet is visible at once.** The menu-bar icon shows a count badge for the hidden needy pets.
- **Rotation:** each visible set lasts 3 h **(default)**, then rotates. A pet never disappears while the modifier is held over it or while its info card is open.
- Pets not on screen are "Resting" in the menu-bar menu.

### 5.8 Slots and the shelter

- Slots: 3 **(default; 1–20)**, changeable in settings.
- When a slot is full, a newly detected or manually added project joins the **Shelter**: it's tracked, but no pet is adopted yet. Shelter projects are listed with their last activity.
- Freeing a slot (graduate, let go, or raising the limit) shows one prompt: "A slot opened. Adopt {top shelter project}?"
- Graduating earns a cosmetic reward. v1 ships a small accessory set (§10.2), and a "Graduates" list appears in the menu-bar menu.

### 5.9 Detecting new projects

- **GitHub:** a repo seen for the first time, created after Strays was installed, and **not** a fork, archived, or matching an ignore rule.
- **Local:** a new `.git` folder appears in a watched parent folder (FSEvents, with a rescan every 6 h **(default)** as a backup).
- **Prompts are batched:** at most one "New projects found (n)" prompt a day **(default)**, shown as a bubble from any visible pet or as a menu-bar badge. Options: Adopt / Add to shelter / Ignore / "Never ask for names matching…".
- **First run:** an "Adopt existing projects" list covers all repos (GitHub) and git folders (local), sorted by last activity, with multi-select.

### 5.10 Nudges

- **Maximum:** 1 bubble per pet per day, and 3 a day across all pets **(defaults)**.
- **Nudge style:** Gentle (default) / Playful / Off. The wording is never guilt-tripping (§10.5).
- **Never shown:** during quiet hours, while Focus is on (if Focus is enabled, §9), in Presentation mode, or over a full-screen app.
- **"Everyone's idle" detection:** if every Active pet reaches Hungry within the same 72-hour window, show one prompt: "Taking a break? Pause everyone?"
- **Global vacation mode:** pauses all pets (§5.6).

---

## 6. GitHub integration

### 6.1 Token

- **Type:** fine-grained personal access token. **Classic tokens (`ghp_`) are rejected**; only `github_pat_` is accepted.
- **Required permission:** Repository → **Metadata: Read-only**. Nothing else.
- **Repository access:** the user chooses. "All repositories" enables new-repo detection. "Only select repositories" means no detection, and the UI says so.
- **Multiple tokens:** one per resource owner (the personal account and each organisation). The "pending organisation approval" state is shown.
- **Storage:** Keychain item `com.khubaibqaiser.strays.github.<owner>`. **The token never reaches the webview.**
- **Verified behaviour (2026-10-30 test):**
  - `GET /user` returns the `github-authentication-token-expiration` header.
  - `/user/repos` returns `pushed_at`.
  - Contents and commits of **private** repos return **403**.
  - Public repos are readable by any token.
- **Collaborator repos (user decision, Gate 1):** not supported in v1. A repo owned by another account can be tracked as a local folder only, and the adoption list says so.

### 6.2 Setup flow

1. A guided steps screen with each GitHub setting spelled out, and a button that opens the token page in the browser. The app **never reads the clipboard automatically**; the user pastes into a field.
2. **Validate:** check the prefix → `GET /user` → `GET /user/repos?per_page=1`.
3. **Show the result before saving:** "Connected as **{login}**. Sees **{n}** repos. Expires **{date}**. Can't read code in private repos; public repos are readable by anyone." → [Save] [Cancel].
4. Save to the Keychain, then run the first sync.

### 6.3 Sync

**Schedule:** every 30 min **(default)**. Options: 5 min · 15 min · 30 min · 1 h · 3 h · 12 h · Manual only.

| Also sync | Default |
|---|---|
| On wake from sleep | On |
| On app launch | On |
| Only on Wi-Fi | Off |
| Pause in Low Power Mode | On |

**Request:** `GET /user/repos?sort=pushed&per_page=100&affiliation=owner,collaborator,organization_member`.
- Follow pagination only until a page's oldest `pushed_at` is earlier than the last sync.
- Use `If-None-Match` (ETag). A **304** doesn't count against the rate limit.

**Errors:**

| Response | Handling |
|---|---|
| 401 | State "Disconnected"; pets freeze (§5.3); prompt to replace the token |
| 403 / 429 (rate limits) | Respect `Retry-After` or `x-ratelimit-reset`; exponential backoff, max 1 h |
| Network error | Back off; retry on the next scheduled run or when the network comes back |

**Manual sync:**
- Menu bar → "Sync now".
- ⌥⇧-click a pet → syncs that pet's repo (`GET /repos/{owner}/{repo}`).
- Optional global shortcut (off by default; suggested ⌃⌥S).

**Status line (Settings → Syncing):** "Last synced 12:04 · next 12:34 · 41 requests today · 4,959/5,000 left this hour · [Sync now]". A failed sync shows a ⚠ on the menu-bar icon.

### 6.4 Token expiry

- Read the expiry header on every response and store it.
- **Reminders** (in-app bubble or menu-bar badge, not system alerts): 7 days and 1 day before expiry **(defaults)**.
- **After expiry:** "Disconnected", pets frozen, and a "Paste new token" flow that replaces the token in place.
- **Disconnect:** deletes the Keychain item immediately.

---

## 7. Desktop overlay and interaction

### 7.1 Windows and placement

- **Window architecture (decided at Gate 0):** **(B)** one transparent window per display, with pets drawn inside it. It measured lower CPU and memory than one window per pet. See `docs/gate-0.md`.
- **Window properties:**
  - Borderless, transparent, always on top (floating level).
  - Joins all Spaces, stationary in Mission Control.
  - Hidden from ⌘-Tab and the Dock (the app is `LSUIElement`).
  - Excluded from screen capture (`sharingType = .none`); whether this holds on current macOS is a **Gate 2 check**.
- **Placement:** pets live on screen **edges** within each screen's `visibleFrame`, avoiding the notch, menu bar and Dock.
  - Ground species (cat, dog, hamster) use the bottom edge.
  - The parrot perches top-right under the menu bar, or on the bottom edge.
  - Pets keep out of the central 60% **(default)** of the frontmost window's bounds, using window bounds from `CGWindowListCopyWindowInfo` (bounds don't need Screen Recording permission).
- **Displays:** pets on a disconnected display move to the main display, and positions are remembered per display layout.
- **Full-screen apps:** pets are hidden while the frontmost window covers the whole display **(default; setting: show / hide)**.
- **Pet size:** Small 72 pt / Medium 104 pt **(default)** / Large 136 pt tall.

### 7.2 Click-through and the modifier

- **By default, every pet window passes all mouse events through.**
- **Polling:**
  - Reads cursor position (`NSEvent.mouseLocation`) and modifier state (`NSEvent.modifierFlags`). No Accessibility or Input Monitoring permission is needed (verified at Gate 0).
  - Rate: 4 Hz normally, 30 Hz while the cursor is within 150 pt of a pet **(targets)**.
- **Interactive state:** when the **modifier is held and the cursor is over a pet's hit shape** (the pet's opaque area + 6 pt padding), that pet's window stops passing clicks through. Releasing the key or leaving the shape restores pass-through within one polling tick.
- **Modifier down:** every pet **freezes** its movement and shows a soft outline. This prevents clicks landing where a pet has just walked away.
- **Modifier key:** ⌥ Option **(default)**; options ⌃, ⇧, ⌥⇧. Changeable in Settings → Interaction.

### 7.3 Actions

| Input | Action |
|---|---|
| ⌥ + hover | Outline, plus a tooltip: name · repo · mood (repo hidden in Presentation mode) |
| ⌥ + click | Open the **info card** (§8.3) |
| ⌥⇧ + click | Quick-sync this pet. The pet perks its ears; result shown as a tick or ⚠. |
| ⌥ + drag | Move the pet. It snaps to the nearest valid edge spot, which becomes its home spot. |
| ⌥ + right-click | Context menu: Snooze ▸ · Sync now · Mark release… · Pause · Graduate… · Pet settings · Hide for today |
| Petting (⌥ + press and hold 1 s, moving < 4 pt) | Petted reaction animation. **No effect on mood.** Moving ≥ 4 pt while held counts as a drag; releasing within 1 s counts as a click. |

**Every action is also available in the menu-bar menu**, so keyboard and VoiceOver users can use them.

### 7.4 Animation behaviour

- **Liveliness (Settings → Display):** how often pets move between big moves. Rendering stops completely while a pet holds a pose. A held pose costs about 0.07% CPU; each animated frame is what costs (Gate 0).
  - **Calm (default):** the pet holds a resting pose. A short moment (a blink, an ear/tail/feather twitch) plays per pet every 20–60 s, randomised.
  - **Gentle:** moments every 5–15 s.
  - **Lively:** continuous breathing and blinks every 3–7 s at 15 fps.
  - The setting shows the expected cost next to each option (for example "Lively: noticeably higher CPU and battery use"). The figures shown come from the Gate 2 measurements, not guesses.
- **Big moves** (stretch, walk to a new spot, groom) happen every 2–5 h, randomised **(default)**. They're slow, under 8 s, and are skipped in Reduce Motion mode.
- **Feeding:** new activity detected → the bowl fills and an eat animation plays (~4 s) the next time the pet is visible.
- **Reduce Motion (system setting):** no walking; Calm moments only (blinks); holiday and snooze exits become fades.

### 7.5 Menu-bar menu

- **Header:** sync status + "Sync now".
- **Sections:**
  - 🐾 On screen
  - 🛋 Resting
  - 🏠 Shelter (n)
  - 💤 Snoozed
  - ⏸ Paused
  - 🏖 On holiday
  - 🎓 Graduates
  - 👋 Let go

  Each pet row has a submenu with every action from §7.3.
- **Toggles:** Presentation mode · Vacation mode · Hide all pets.
- **Footer:** Settings… · Network activity… · Quit.

---

## 8. User interface

All windows are standard macOS windows, and they are the only places with clickable UI apart from the overlay.

### 8.1 First run (3 skippable screens)

1. **Welcome:** the shelter story; a pet walks in. "Strays works offline. Nothing is on until you choose."
2. **Where are your projects?** Options: **GitHub** (recommended if you use multiple devices) / **Folders on this Mac** / **Both**. Each has a one-line trade-off. This leads to §6.2 and/or a folder picker.
3. **Comfort:**

   | Setting | Default |
   |---|---|
   | Sound | Off |
   | Launch at login | Off |
   | Nudges per day | 3 |
   | Quiet hours | 22:00–08:00 |
   | Max visible pets | 3 |

   Then → "Adopt existing projects" list (§5.9) → adoption flow.

### 8.2 Adoption flow

1. Project picked (or prefilled).
2. **Species:** cat (Persian) / dog (Pembroke Welsh Corgi) / hamster (Syrian) / parrot (budgerigar).
3. **Colour:** curated colour chips + a free colour wheel. A live preview shows the pet. Accent colours and outline contrast are calculated automatically (§10.4).
4. **Name** (prefilled with a random suggestion).
5. **Goal:** "What does done look like?" One line, optional, plus an optional checklist.
6. **Pace:** Sprint / Normal / Slow-burn.
7. **Arrival animation:** the pet walks in from the edge.

### 8.3 Info card

A small floating panel next to the pet. It closes on Esc or a click outside.

- Name, species icon, mood (word + icon), "{n} days since last activity", "next mood in {n} days".
- **Tracking:** repo `owner/name` (or folder path), source badges (GitHub / Local / This Mac only), last activity with its source and time.
- Goal and checklist (tickable).
- **Buttons:** Sync now · Snooze ▸ · Mark release… · Pause · Graduate… · Open on GitHub · Reveal in Finder · Pet settings.

### 8.4 Settings window (sidebar sections)

| Section | Contents |
|---|---|
| General | Launch at login · menu-bar icon style · language (English only in v1) |
| Pets | Slots · default pace · holiday length · release cooldown · pet size · ignore rules |
| Moods | Thresholds, presets, preview, don't-count-weekends |
| Display | Liveliness (§7.4) · max/min visible · rotation period · full-screen behaviour · which displays · Presentation mode · hide repo names in bubbles |
| Nudges | Style · per-pet and global daily caps · quiet hours · respect Focus (§9) · wake stagger |
| Sound | Master on/off (off by default) · volume · per-species toggle |
| Interaction | Modifier · global shortcut (off by default) · shortcut recorder |
| Syncing | Interval · conditions · status line · Sync now |
| **Privacy & Permissions** | §8.5 |
| About | Version · licences · diagnostics |

### 8.5 Privacy & Permissions screen

One card per capability. Each card shows **status · what it can do · what it can't do · actions**.

```
┌─ GitHub access ─────────────────────────────── ● Connected ─┐
│ Can: see repo names and last push times                     │
│ Can't: read code in private repos, write anything, see      │
│        issues/PRs. Public repos are readable by any token.  │
│ Accounts: khubaibqaiser (expires 29 Nov)                    │
│ [Test]  [Replace token]  [Add account/org]  [Disconnect]    │
├─ Project folders ───────────────────────────── ● 2 folders ─┤
│ Reads commit dates and tags only. Never file contents.      │
│ ~/Code  ~/Work                    [Add folder]  [Remove]    │
├─ Network ───────────────────────────── ● api.github.com only┤
│ 41 requests today.  [View activity log]  [ ] Go fully offline│
├─ Launch at login ──────────────────────────── ○ Off [Turn on]┤
├─ Focus status ───────────────────────────── ○ Off [Turn on]  ┤
├─ Screen capture ────────────── Hide pets when sharing ● On   ┤
└─ Your data ──── [Export JSON]  [Open data folder]  [Delete everything] ┘
```

**Network activity log:** time · method · host · path (repo names replaced with ids) · status · reason (scheduled / manual / validation). The last 1,000 entries are kept.

**Delete everything:** removes the Keychain items, folder bookmarks, data files and logs, then quits.

---

## 9. Permissions and security model

| Capability | Mechanism | Default | Requested when |
|---|---|---|---|
| Network (`api.github.com`) | Sandbox entitlement `network.client` + a Rust host allowlist + webview CSP `connect-src 'none'` | Allowlisted host only | GitHub connect |
| User-chosen folders | Sandbox user-selected read-only + security-scoped bookmarks | None | Adding a folder |
| Keychain | App's own items only | — | Saving a token |
| Launch at login | `SMAppService` (shown in System Settings → Login Items) | Off | User turns it on |
| Focus status | `INFocusStatusCenter`. Shows a macOS permission prompt; the app learns only "Focus on/off". | Off | User turns on "Respect Focus" |
| Notifications | — | **Not used in v1** (in-app bubbles only) | — |

**Never requested:** Accessibility, Input Monitoring, Screen Recording, Full Disk Access, camera, microphone, location, contacts, admin rights. Each of these is checked by a test (§14).

**Hardening:**
- Tauri capabilities expose a minimal command list to the webview; no shell, file system or HTTP plugins are exposed to it.
- No remote content; every asset is bundled; CSP `default-src 'self'`.
- The token lives only in Rust memory and the Keychain. Logs redact anything that looks like a token.
- Dependencies: lockfiles committed; `cargo audit`, `cargo deny` and `npm audit` in CI; GitHub Actions pinned to commit SHAs; Dependabot.
- Data files are written atomically (temp file + rename), with 3 rotating backups and a `schemaVersion` with migrations.

**Known limits (documented honestly):**
- The network entitlement is fixed when the app is built. "Offline until you opt in" is enforced by our code and visible in the log; macOS itself doesn't enforce it.
- Any token can read public repos.

---

## 10. Art, animation and audio

### 10.1 Style guide (to be produced first, as `art/STYLE.md`)
- Flat vector, soft rounded shapes, a gentle outline, a limited shading model (base + one shade + one highlight).
- A fixed palette for non-coat parts (eyes, nose, accessories, props).
- Silhouettes recognisable at 72 pt; consistent line weight, measured at the Medium size.

### 10.2 Assets (v1)

| Asset | Notes |
|---|---|
| 4 species rigs | Cat (Persian), Dog (Pembroke Welsh Corgi), Hamster (Syrian), Parrot (budgerigar). Bones where Gate 0 proves them. |
| Clips per species | idle-breathe · blink · twitch · walk · sit · sleep · eat · petted · mood poses ×5 · celebrate · exit · enter · fall-asleep · wake-stretch · carry-postcard. Parrot adds fly + perch. Each mood pose blends with the idle animation. |
| Props | Food bowl (empty → full), small bed, rain cloud, postcard, confetti |
| Accessories (graduation rewards) | 5: graduation cap, scarf, bow, star badge, flower crown |
| UI | Menu-bar icon (template image, mood-badge variants), app icon, speech bubble, info-card art, onboarding illustrations |
| Audio | CC0 or licensed: ~3 sounds per species (greeting, happy, sad) + wake sound + celebration jingle + UI tick. The source and licence of every file is recorded in `assets/audio/CREDITS.md`. |

### 10.3 Celebrations
- Cat: zoomies, then sits proudly.
- Dog: spins, play bow.
- Hamster: cheek stuff, hops.
- Parrot: loop flight, feather fluff.

### 10.4 Recolouring
- Each rig exposes a view model with colour properties: `coatBase`, `coatShade`, `coatHighlight`, `outline`, plus `mood` (number) and triggers for one-off clips.
- The app picks `coatBase`; the others are calculated in OKLCH colour space:
  - shade: L −0.12
  - highlight: L +0.10
  - outline: L −0.35, clamped
  - on dark coats the outline goes lighter: whichever of L −0.35 / L +0.35 reaches 3:1 is used, pushed further if neither does (Gate 3; a fixed L < 0.3 threshold failed the contrast test for coats just above 0.3)
- **Contrast test:** outline vs coat ≥ 3:1 for every curated colour and for 500 random colours.

### 10.5 Writing
Bubble text lives in one strings file per nudge style. The tone is warm and never shaming. Examples (Gentle):
- Hungry: "I miss *{repo}*. Even a tiny commit would make my day."
- Gloomy: "Still here, still waiting by the window."

### 10.6 Pipeline
1. RML source in `art/<species>/` is the source of truth.
2. `rive <dir> --verify` + `rive inspect --summary` run on every change.
3. CI builds unsigned `.riv` files with `rive <dir> --once`. **No Luau scripts** in shipped files.
4. **Review artefacts:** `--screenshot --advance` at 12 time steps per clip → a contact sheet + GIF in `art/review/`. The agent reviews them, and the user signs off **once per species**.

---

## 11. Architecture

```
┌──────────────────────── Rust core (src-tauri) ────────────────────────┐
│ clock ── mood engine (pure) ── state store (JSON, atomic)             │
│ sources: github client (allowlisted HTTP) · local git (gix, bookmarks)│
│ scheduler (sync, snooze/holiday/rotation timers from timestamps)      │
│ platform: overlay windows · cursor/modifier poller · screens · focus  │
│           capture exclusion · login item · keychain · audio (rodio)   │
│ net log · diagnostics                                                 │
└─────────────── typed IPC (minimal command set) ───────────────────────┘
┌──────── Webview: overlay pet renderer (Rive) · settings/card/onboarding UI ────────┐
```

- **The Rust core owns all state and decisions.** The webview renders what it's told and sends user intents back.
- **Timers are recalculated from timestamps**, never long sleeps: on start, on wake, and after any state change.
- **Clock:** a single trait `Clock`. Debug builds support `--time-warp <duration>` and a test-only IPC to set the time.

### 11.1 Data model (JSON, `schemaVersion: 1`)

```jsonc
// pets/<uuid>.json — one file per pet (ready for future sync)
{
  "id": "uuid", "createdAt": "ISO", "updatedAt": { "<field>": "ISO" },
  "name": "Miso", "species": "cat",
  "colors": { "coatBase": "#E8C9A0" }, "accessory": null,
  "project": {
    "github": { "id": 123, "fullName": "owner/name", "account": "owner" } | null,
    "local":  { "bookmarkId": "uuid", "displayPath": "~/Code/name" } | null
  },
  "goal": { "text": "", "checklist": [{ "text": "", "done": false }] },
  "pace": "normal", "thresholdsOverride": null,
  "state": { "kind": "active|shelter|snoozed|paused|holiday|graduated|letGo",
             "until": "ISO|null", "since": "ISO", "reason": "" },
  "activity": { "last": "ISO", "source": "github|localCommit|localRemoteRef" },
  "frozen": [{ "from": "ISO", "to": "ISO|null", "why": "holiday|pause" }],
  "releases": [{ "at": "ISO", "label": "v1.2" }],
  "home": { "displayKey": "…", "edge": "bottom", "offset": 0.82 }
}
// settings.json, accounts.json (no secrets), sync-state.json (ETags, lastSuccessfulCheck), netlog.jsonl
```

### 11.2 Repo layout

```
strays/
  app/            Tauri app (src-tauri/ Rust, src/ TS)
  art/            RML per species, STYLE.md, review/ (generated)
  assets/audio/   sounds + CREDITS.md
  tools/          fake GitHub server, overlay test harness (Swift input/screenshot tool)
  docs/spec.md    this file
```

---

## 12. Performance budgets (targets, measured at Gate 2 on an Apple Silicon Mac)

| Metric | Target |
|---|---|
| Average CPU, 3 visible pets, Calm (default), 10 min | ≤ 1.5% (user decision, Gate 2a: set from measurements of 0.9–1.2%; the original < 1% was not reached reliably) |
| Average CPU, 3 visible pets, Gentle / Lively | Measured and shown in Settings; no hard target |
| CPU while every pet holds a pose | < 0.1% |
| CPU during a walk | < 5%, then back to idle within 2 s |
| Memory (all processes) | < 200 MB |
| Frame rate during big moves | 60 fps |
| Frame rate during micro-idle | ≤ 30 fps (Lively: 15 fps) |
| Frame rate when hidden or covered | Paused rendering |
| Energy | No "Significant Energy" entry in Activity Monitor |
| Cold start to pets visible | < 2 s |

---

## 13. Accessibility

- Every function is reachable from the menu-bar menu and the windows with the keyboard alone; VoiceOver labels on all controls.
- Respects **Reduce Motion** (§7.4) and **Increase Contrast** (thicker outlines).
- Mood is never shown by colour alone: pose + icon + word.
- Sound is off by default; nothing depends on sound.

---

## 14. Verification (AI-driven)

### 14.1 Gates

| Gate | Scope | Pass criteria |
|---|---|---|
| **0: Test** | Rive CLI rig of one cat: breathing, blinking, tail sway with bones, colour data binding in the web runtime inside Tauri. Overlay window prototype A vs B. Cursor/modifier polling without permission prompts. Unsigned `.riv` plays without a watermark. | All pass + contact sheet reviewed + **user approves the look**. Any failure → **pause and decide with the user**. |
| **1: Core** | Mood engine, state machine, settings, storage, GitHub client, local git, matching | Unit + property tests green; fake-server scenarios green; live test token scenarios green |
| **2a: Overlay** | App shell on the core, sync loop, Keychain, windows, placement, modifier interaction, menu bar, Spaces, full screen, capture exclusion, Reduce Motion | Automated OS tests green; performance budgets met |
| **2b: Windows** | Info card, settings, token setup, first run and adoption (§8), local folders, drag and home spots, nudges, per-display windows | Front-end tests and OS tests green |
| **3: Content** | All 4 species, all clips, props, accessories, audio, writing | Contact sheets reviewed, **user sign-off per species**, contrast tests green |
| **4: Release** | Signing, notarisation, packaging, opt-in updater, docs | Notarised `.dmg` installs and runs on a clean macOS 14 VM and on the latest macOS |

### 14.2 Test layers

| Layer | How it runs |
|---|---|
| Logic | Rust unit tests with a fake `Clock`. Property tests: `idleDays` never negative; never higher after a failed sync; frozen intervals respected. |
| GitHub | `tools/fake-github` replays 200, 304, 401, 403 secondary rate limit, 429, pagination, expiry header, archived/deleted/renamed/transferred repos. |
| Front end | The webview UI runs in a browser with Tauri's IPC mocks; Playwright tests. |
| Real OS | `tools/overlay-harness`: launches a debug build, places pets through test IPC, uses `screencapture` and synthetic `CGEvent` input (⌥-hover, ⌥-click, plain clicks through a pet onto a test window underneath), and asserts on screenshots and app state. Uses the Screen Recording + Accessibility permissions the user granted to the terminal. |
| Permissions audit | Check the built app's entitlements and `Info.plist` against an allowlist; run a session and confirm no TCC prompts appear other than the opted-in ones. |
| Network audit | A test run with a local proxy confirms every request goes to an allowlisted host and matches the log. |
| Animation | `rive --verify`, contact sheets, loop-seam checks (first and last frames match). |
| Performance | Scripted 10-minute idle run sampling `ps` CPU and memory; fails the build if over budget. |

### 14.3 Debug-only features
`--time-warp`, `--fake-github <url>`, `--spawn <species>:<mood>:<state>`, a test IPC channel and a state dump. **All are compiled out of release builds** (Cargo feature `devtools`).

---

## 15. Release and distribution (macOS)

1. **Testing yourself:** local builds (`npm run tauri build`).
2. **Beta and public:** signed with a Developer ID and **notarised**. Distributed as a `.dmg` on GitHub Releases, with SHA-256 checksums. A universal binary (Apple Silicon + Intel) if size allows; otherwise two builds.
3. **CI:** GitHub Actions on a macOS runner: test → build `.riv` → build app → sign → notarise → draft release. Note: macOS runner minutes on private repos count at a higher rate (check the current GitHub pricing).
4. **Updates:** Tauri updater with signed update manifests. **Opt-in**, off by default; checks weekly when on, logged in the network log. Adds the GitHub release hosts to the allowlist only while it's on.
5. **Later:** Homebrew cask. No Mac App Store in v1.

---

## 16. Manual steps (user)

| When | Step |
|---|---|
| ✅ Done | Rust + Rive CLI installed; test token created (expires 2026-11-29) |
| Before Gate 0 | Grant **Screen Recording + Accessibility** to the terminal app that runs the agent (permanent, by user decision) |
| Gate 0 | Approve the look of the test cat |
| Gate 1 | Confirm creation of `strays-test-*` repos using the `gh` login |
| Before 2026-11-29 | Replace the test token if testing continues |
| Gate 3 | Approve sound download batches; sign off each species |
| Gate 0 / Gate 4 | Approve installing UTM and downloading a macOS 14 restore image for the minimum-OS VM |
| Gate 4 | Enrol in the Apple Developer Program; create a Developer ID certificate and a notarisation key; store CI secrets; keep an offline backup of the updater signing key |
| Before release | Choose the licence and whether the repo goes public; confirm the final name after a conflict check |

---

## 17. Deferred / backlog

- **Game world:** the full-screen "Shelter House" where holidays and graduates live.
- **Accurate mode** (`Contents: Read-only`): own commits only, default branch, bot filtering, automatic release detection from GitHub releases or tags.
- **Cross-device sync**, user-controlled: export/import file → a user-chosen folder → a private GitHub save repo with a separate write token.
- Windows release; Homebrew; localisation; more breeds and species; monorepo subfolders; linking several repos to one pet; GitLab/Bitbucket remote support.
- Paid cosmetic packs (the design keeps accessories data-driven).
- Mobile companion (widget + app).

## 18. Open questions

| # | Question | Resolved by |
|---|---|---|
| 1 | Do fine-grained tokens see collaborator-only repos? | **Skipped for v1** (user decision); local folders only |
| 2 | Is `sharingType = .none` respected by current screen-capture APIs? | Gate 2 |
| 3 | Overlay architecture A vs B | **Resolved (Gate 0):** B |
| 4 | Does Rive colour data binding work in the web runtime inside Tauri's webview? | **Resolved (Gate 0):** yes |
| 5 | Can the RML/CLI route produce bones-based motion of acceptable quality? | **Resolved (Gate 0):** yes; user approved the test cat as a starting direction |
| 6 | Is Focus status available to a sandboxed Tauri app (Intents entitlement)? | Gate 2; fallback: quiet hours only |
| 7 | Does launch at login through `SMAppService` work from Tauri in the sandbox? | Gate 2 |
