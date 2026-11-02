# Strays user guide

Strays gives each unfinished project a small animated pet on the edge of your screen. This guide covers installing it, connecting your projects, day-to-day use, privacy and removing it. The [README](../README.md) explains how moods work.

Needs a Mac with **Apple Silicon** (M1 or later) and macOS 14 Sonoma or later. **Intel Macs are not supported**: Apple is ending macOS for Intel, and there is no Intel Mac to test on. See [Intel Macs](gate-4.md#intel-macs) for details and what adding support would take.

## Install

1. Open `Strays.dmg` and drag **Strays** into **Applications**.
2. Open Strays from Applications. It lives in the menu bar (🐾), not the Dock.

> Until releases are signed with a Developer ID and notarised, macOS blocks the app on first open. Builds before then are for testing only.

## First run

The welcome screens ask where your projects are:

- **GitHub:** tracks pushes to your repos. Good if you work on more than one Mac.
- **Folders on this Mac:** tracks commits in local git repos. No account needed.
- **Both.**

Nothing is switched on until you choose. You can skip every screen and set things up later in **Settings**.

## Connect GitHub

Strays needs a **fine-grained** personal access token with one permission. Classic tokens (starting `ghp_`) are refused.

1. In Strays, open **Settings → Privacy & Permissions → Connect GitHub…**. The button there opens GitHub's token page.
2. On GitHub, create a fine-grained token:
   - **Expiration:** your choice. Strays reminds you 7 days and 1 day before it expires.
   - **Repository access:** **All repositories** lets Strays notice new repos. **Only select repositories** works too, but new repos aren't detected.
   - **Permissions:** **Metadata: Read-only**, and nothing else.
3. Paste the token into Strays. Before saving, it shows the account, how many repos it sees and when it expires.

The token is stored in the macOS Keychain. It can see when repos were pushed to, not their code. **Disconnect** deletes it from the Keychain at once.

When the token expires, pets freeze (moods don't drop) until you paste a new one.

## Add project folders

**Settings → Privacy & Permissions → Add folder…** and choose a folder that holds your git repos, such as `~/Code`. Every repo inside it is offered as a project, including ones you create later.

Strays reads commit dates and remote URLs only, never file contents, and never runs `git`. macOS only lets Strays read folders you pick here; it remembers them across restarts.

## Adopt a pet

Choose **Adopt a pet…** from the menu bar or Settings, then pick a project, a species (cat, corgi, hamster or budgie), a colour, a name, an optional goal and a pace.

You can keep 3 pets by default (change it in **Settings → Pets**). Other projects wait in the **Shelter**. When a slot opens, Strays offers the next one.

## Everyday use

Clicks go straight through pets, so they never get in your way. Hold **⌥ Option** to interact:

| Input | Action |
|---|---|
| ⌥ + hover | Pets freeze; this one shows its name, repo and mood |
| ⌥ + click | Opens the pet's card: mood, goal checklist, sync, snooze, release, pause, graduate |
| ⌥⇧ + click | Syncs this pet now |

Every action is also in the 🐾 menu. The modifier key can be changed in **Settings → Interaction**.

- **Snooze** hides a pet for a while. Its hunger keeps counting.
- **Pause** is for a deliberate break. Its hunger stops.
- **Vacation mode** (menu bar) pauses every pet.
- **Mark release** sends the pet on a short holiday.
- **Graduate** is for a finished project. The pet puts on an accessory, says goodbye and joins the **Graduates** list.

## Can't see the menu bar icon?

On a MacBook with a full menu bar, macOS hides icons behind the notch. Open **Strays** again from Applications or Spotlight and Settings opens.

## Launch at login

**Settings → Privacy & Permissions → Launch at login.** If macOS asks for approval, allow Strays in **System Settings → General → Login Items**.

## Updates

**Settings → About → Tell me when a new version is out.** It's off by default. When it's on, Strays asks GitHub once a week for the latest release, without your token, and tells you in About and the menu bar when a new version is out. **Download…** opens the release page; install the new `.dmg` the same way as the first. Strays never installs anything by itself.

## Privacy

- No network until you connect GitHub, and then only `api.github.com`.
- **Settings → Privacy & Permissions → View activity log** lists every request Strays made.
- **Export JSON** saves all your data. **Open data folder** shows where it lives.
- Pets are hidden from screen recordings and screen sharing.

## Remove Strays

1. **Settings → Privacy & Permissions → Delete everything** and type `DELETE`. This removes the GitHub token from the Keychain and all Strays data, then quits.
2. Drag **Strays** from Applications to the Bin.
