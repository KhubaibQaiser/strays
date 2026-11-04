import { expect, test } from "@playwright/test";
import { PET_ID, STATE, calls, mockApp } from "./mock";
import { looksFor } from "../src/ui/look";

const shots = "test-results/screens";

test("info card shows mood, tracking and goal, and saves checklist changes", async ({ page }) => {
  await mockApp(page);
  await page.setViewportSize({ width: 320, height: 430 });
  await page.goto(`/ui.html#card/${PET_ID}`);
  await expect(page.getByRole("heading", { name: /Miso/ })).toBeVisible();
  await expect(page.getByText(/Peckish · 6 days since last activity/)).toBeVisible();
  await expect(page.getByText("next mood in 4 days")).toBeVisible();
  await expect(page.getByText("me/strays")).toBeVisible();
  await page.screenshot({ path: `${shots}/card.png` });

  await page.getByPlaceholder("Add a step and press Return").fill("Write the report");
  await page.keyboard.press("Enter");
  const saved = await calls(page, "set_goal");
  expect(saved.at(-1)?.args.goal).toEqual({ text: "Ship v1", checklist: [{ text: "Gate 2", done: true }, { text: "Write the report", done: false }] });

  // Graduate needs a second click, so a slip can't end a project
  await page.getByRole("button", { name: "Graduate…" }).click();
  expect(await calls(page, "pet_action")).toHaveLength(0);
  await page.getByRole("button", { name: "Finished? Click again" }).click();
  expect((await calls(page, "pet_action")).at(-1)?.args).toMatchObject({ id: PET_ID, action: "graduate" });

  await page.keyboard.press("Escape");
  expect((await calls(page, "close_window")).at(-1)?.args).toEqual({ label: "card" });
});

test("settings save each change and reject thresholds that don't increase", async ({ page }) => {
  await mockApp(page);
  await page.goto("/ui.html#settings/pets");
  await page.getByLabel("slots").fill("5");
  await page.getByLabel("slots").press("Tab");
  await expect.poll(async () => (await calls(page, "save_settings")).at(-1)?.args.settings).toMatchObject({ slots: 5 });
  await page.screenshot({ path: `${shots}/settings-pets.png` });

  await page.getByRole("button", { name: "Moods" }).click();
  const before = (await calls(page, "save_settings")).length;
  await page.getByLabel("Content up to (days)").fill("1");
  await page.getByLabel("Content up to (days)").press("Tab");
  await expect(page.getByRole("alert")).toHaveText("Each level must last longer than the one before it.");
  expect(await calls(page, "save_settings")).toHaveLength(before);

  await page.getByRole("button", { name: "Display" }).click();
  await page.getByLabel("liveliness").selectOption({ label: "Lively: the smoothest, always moving (most battery)" });
  await expect.poll(async () => (await calls(page, "save_settings")).at(-1)?.args.settings).toMatchObject({ liveliness: "lively" });
  await page.screenshot({ path: `${shots}/settings-display.png` });
});

test("sound settings: off by default, a master switch, volume, per-species and a test button", async ({ page }) => {
  await mockApp(page);
  await page.goto("/ui.html#settings/sound");
  await expect(page.getByLabel("Play sounds")).not.toBeChecked();
  await page.getByLabel("Play sounds").check();
  await expect.poll(async () => (await calls(page, "save_settings")).at(-1)?.args.settings).toMatchObject({ sound: true });
  await page.getByLabel("Cats").uncheck();
  await expect.poll(async () => (await calls(page, "save_settings")).at(-1)?.args.settings).toMatchObject({ mutedSpecies: ["cat"] });
  await page.getByLabel("Volume").fill("80");
  await page.getByLabel("Volume").dispatchEvent("change");
  await expect.poll(async () => (await calls(page, "save_settings")).at(-1)?.args.settings).toMatchObject({ volume: 80 });
  await page.getByRole("button", { name: "Test" }).click();
  expect(await calls(page, "test_sound")).toHaveLength(1);
  await page.screenshot({ path: `${shots}/settings-sound.png` });
});

test("launch at login: a refusal from macOS unticks the box and says why", async ({ page }) => {
  await mockApp(page, { set_login_item: "Operation not permitted" });
  await page.goto("/ui.html#settings/privacy");
  await page.getByLabel("Open Strays when you log in").click();
  await expect(page.getByLabel("Open Strays when you log in")).not.toBeChecked();
  await expect(page.getByText("Operation not permitted")).toBeVisible();
});

test("update check: off by default, opt-in saves, a newer release offers a download", async ({ page }) => {
  await mockApp(page);
  await page.goto("/ui.html#settings/about");
  await expect(page.getByLabel("Tell me when a new version is out")).not.toBeChecked();
  await page.getByLabel("Tell me when a new version is out").check();
  const saved = (await calls(page, "save_settings")).at(-1)?.args as { settings: { checkUpdates: boolean } };
  expect(saved.settings.checkUpdates).toBe(true);

  await mockApp(page, {}, { settings: { ...STATE.settings, checkUpdates: true }, update: { tag_name: "v0.2.0", html_url: "https://github.com/KhubaibQaiser/strays/releases/tag/v0.2.0" } });
  await page.reload();
  await expect(page.getByText("Strays v0.2.0 is out.")).toBeVisible();
  await page.getByRole("button", { name: "Download…" }).click();
  expect((await calls(page, "open_link")).at(-1)?.args).toEqual({ which: "release" });
});

test("privacy page shows what GitHub access can and can't do, and guards delete", async ({ page }) => {
  await mockApp(page);
  await page.goto("/ui.html#settings/privacy");
  await expect(page.getByText("Account: me · sees 96 repos")).toBeVisible();
  await page.getByRole("button", { name: "View activity log" }).click();
  await expect(page.getByText("GET api.github.com/user/repos?sort=pushed")).toBeVisible();
  await page.getByLabel("Open Strays when you log in").check();
  expect((await calls(page, "set_login_item")).at(-1)?.args).toEqual({ on: true });
  await page.screenshot({ path: `${shots}/settings-privacy.png`, fullPage: true });
  await page.getByRole("button", { name: "Delete everything" }).click();
  expect((await calls(page, "delete_everything")).at(-1)?.args).toEqual({ confirm: "" });
});

test("GitHub setup checks the token, shows the result, then saves", async ({ page }) => {
  await mockApp(page);
  await page.setViewportSize({ width: 520, height: 600 });
  await page.goto("/ui.html#setup");
  await page.getByLabel("Token").fill("github_pat_test");
  await page.getByRole("button", { name: "Check token" }).click();
  await expect(page.getByText("Sees 96 repos")).toBeVisible();
  expect(await calls(page, "token_save")).toHaveLength(0);
  await page.screenshot({ path: `${shots}/setup.png` });
  await page.getByRole("button", { name: "Save" }).click();
  expect((await calls(page, "token_save")).at(-1)?.args).toEqual({ token: "github_pat_test" });
  await expect(page.getByText(/Saved to the Keychain/)).toBeVisible();
});

test("GitHub setup shows why a token was refused", async ({ page }) => {
  await mockApp(page, { token_check: "That isn't a fine-grained token. It should start with github_pat_" });
  await page.goto("/ui.html#setup");
  await page.getByLabel("Token").fill("ghp_classic");
  await page.getByRole("button", { name: "Check token" }).click();
  await expect(page.getByText(/isn't a fine-grained token/)).toBeVisible();
});

test("adoption needs a project, then sends every choice", async ({ page }) => {
  await mockApp(page);
  await page.setViewportSize({ width: 560, height: 900 });
  await page.goto("/ui.html#adopt");
  await page.getByRole("button", { name: "Adopt", exact: true }).click();
  await expect(page.getByText("Pick a GitHub repo or a folder first.")).toBeVisible();
  expect(await calls(page, "adopt")).toHaveLength(0);

  await page.getByLabel("Repository").selectOption("2");
  await page.getByRole("button", { name: /Parrot/ }).click();
  await page.getByRole("button", { name: "Colour 3" }).click();
  await page.getByLabel("Name").fill("Biscuit");
  await page.getByLabel("Goal").fill("Launch it");
  await page.getByLabel("Pace").selectOption("slowBurn");
  // the preview is the 3D pet itself, once its shape is built
  await expect(page.getByRole("img", { name: "Preview of your parrot" }).locator("canvas")).toHaveCount(1, { timeout: 30_000 });
  await page.waitForTimeout(1000);
  await page.screenshot({ path: `${shots}/adopt.png`, fullPage: true });
  await page.getByRole("button", { name: "Adopt", exact: true }).click();
  expect((await calls(page, "adopt")).at(-1)?.args.request).toEqual({
    repoId: 2,
    folder: null,
    species: "parrot",
    colors: looksFor("parrot")[2], // parrots (macaws) get their own swatches
    name: "Biscuit",
    goal: "Launch it",
    pace: "slowBurn",
  });
});

test("welcome walks its steps, saves the comfort settings, and adopts several projects at once", async ({ page }) => {
  await mockApp(page);
  await page.setViewportSize({ width: 560, height: 560 });
  await page.goto("/ui.html#welcome");
  await expect(page.getByText("Strays works offline. Nothing is on until you choose.")).toBeVisible();
  await page.screenshot({ path: `${shots}/welcome.png` });
  await page.getByRole("button", { name: "Continue" }).click();
  await page.getByRole("button", { name: "Add a folder…" }).click();
  await page.getByRole("button", { name: "Continue" }).click();
  // error reports are asked for, never assumed
  const reports = page.getByLabel("Help fix bugs: send anonymous error and crash reports");
  await expect(reports).not.toBeChecked();
  await page.screenshot({ path: `${shots}/welcome-comfort.png` });
  await page.getByRole("button", { name: "Continue" }).click();
  await expect.poll(async () => (await calls(page, "save_settings")).at(-1)?.args.settings).toMatchObject({ watchedFolders: ["/Users/me/Code"], errorReports: false });
  // existing projects, untracked ones only, ticked as many as wanted
  await expect(page.getByRole("heading", { name: "Adopt your projects" })).toBeVisible();
  await expect(page.getByLabel(/me\/strays/)).toHaveCount(0);
  await page.getByLabel(/me\/side-project/).check();
  await page.screenshot({ path: `${shots}/welcome-adopt.png` });
  await page.getByRole("button", { name: "Adopt 1" }).click();
  await expect.poll(async () => (await calls(page, "adopt")).length).toBe(1);
  expect((await calls(page, "adopt"))[0].args.request).toMatchObject({ repoId: 2, folder: null, species: "cat", goal: "" });
  expect((await calls(page, "close_window")).at(-1)?.args).toEqual({ label: "welcome" });
});

test("a refresh keeps what's being typed and the scroll position", async ({ page }) => {
  await mockApp(page);
  await page.setViewportSize({ width: 600, height: 300 });
  await page.goto("/ui.html#settings/privacy");
  const box = page.getByLabel("Type DELETE to confirm");
  await box.fill("DEL");
  await page.locator("main").evaluate((m) => (m.scrollTop = 120));
  await page.evaluate(() => (window as unknown as { __emit: (e: string, p: unknown) => void }).__emit("state-changed", null));
  await expect.poll(() => calls(page, "ui_state").then((c) => c.length)).toBe(2);
  await expect(box).toHaveValue("DEL");
  await expect(box).toBeFocused();
  // out of the field, a refresh redraws but stays where the user scrolled
  await page.locator("main").click({ position: { x: 5, y: 5 } });
  await page.evaluate(() => (window as unknown as { __emit: (e: string, p: unknown) => void }).__emit("state-changed", null));
  await expect.poll(() => calls(page, "ui_state").then((c) => c.length)).toBe(3);
  expect(await page.locator("main").evaluate((m) => m.scrollTop)).toBeGreaterThan(100);
});

test("problems show in Settings until dismissed, and a failed call is never silent", async ({ page }) => {
  await mockApp(page, { open_data_folder: "Finder isn't responding" }, { issues: ["Sync history couldn't be read, so Strays started fresh."] });
  await page.goto("/ui.html#settings/privacy");
  await expect(page.getByRole("alert").getByText(/Sync history couldn't be read/)).toBeVisible();
  await page.getByRole("button", { name: "Dismiss" }).click();
  expect(await calls(page, "dismiss_issues")).toHaveLength(1);
  // this button has no handler of its own; the page-wide one still reports it
  await page.getByRole("button", { name: "Open data folder" }).click();
  await expect(page.getByText("Finder isn't responding")).toBeVisible();
});

test("the card asks what happened to a repo GitHub can't find", async ({ page }) => {
  await mockApp(page, {}, { pets: [{ ...STATE.pets[0], missing: true }] });
  await page.setViewportSize({ width: 320, height: 430 });
  await page.goto(`/ui.html#card/${PET_ID}`);
  await expect(page.getByText("Where did my repo go?")).toBeVisible();
  await expect(page.getByText(/GitHub can't find me\/strays any more/)).toBeVisible();
  await page.screenshot({ path: `${shots}/card-missing.png` });
  await page.getByRole("button", { name: "Look again" }).click();
  expect((await calls(page, "pet_action")).at(-1)?.args).toMatchObject({ id: PET_ID, action: "sync" });
  await page.getByRole("button", { name: "Check the token…" }).click();
  expect((await calls(page, "open_window")).at(-1)?.args).toMatchObject({ kind: "setup" });
  // letting go needs a second click, like every other way out
  await page.getByRole("button", { name: "It was deleted: let go" }).click();
  expect(await calls(page, "pet_action")).toHaveLength(1);
  await page.getByRole("button", { name: "Let go? Click again" }).click();
  expect((await calls(page, "pet_action")).at(-1)?.args).toMatchObject({ id: PET_ID, action: "letgo" });
});

test("error reports are opt-in, explained, and unavailable without a key", async ({ page }) => {
  await mockApp(page);
  await page.goto("/ui.html#settings/privacy");
  const box = page.getByLabel("Send anonymous error and crash reports");
  await expect(box).not.toBeChecked();
  await expect(page.getByText(/a short code \(never its message, so no names or paths\)/).first()).toBeVisible();
  await box.check();
  await expect.poll(async () => (await calls(page, "save_settings")).at(-1)?.args.settings).toMatchObject({ errorReports: true });
  await page.getByRole("button", { name: "Show log" }).click();
  expect(await calls(page, "reveal_log")).toHaveLength(1);
});

test("a build without a PostHog key can't turn error reports on", async ({ page }) => {
  await mockApp(page, {}, { reportsAvailable: false });
  await page.goto("/ui.html#settings/privacy");
  await expect(page.getByLabel("Send anonymous error and crash reports")).toBeDisabled();
  await expect(page.getByText("○ Not in this build")).toBeVisible();
});

test("a settings change lands on top of what another window saved meanwhile, and a bad number isn't sent", async ({ page }) => {
  await mockApp(page);
  await page.goto("/ui.html#settings/pets");
  // the welcome window (say) turned sound on after this page loaded
  await page.evaluate(() => ((window as unknown as { __answers: { ui_state: { settings: { sound: boolean } } } }).__answers.ui_state.settings.sound = true));
  await page.getByLabel("slots").fill("5");
  await page.getByLabel("slots").press("Tab");
  await expect.poll(async () => (await calls(page, "save_settings")).at(-1)?.args.settings).toMatchObject({ slots: 5, sound: true });
  await page.getByLabel("slots").fill("");
  await page.getByLabel("slots").press("Tab");
  await expect(page.getByText("slots must be a number from 1 to 20")).toBeVisible();
  await expect(page.getByLabel("slots")).toHaveValue("5");
  expect(await calls(page, "save_settings")).toHaveLength(1);
});

test("a pet can have its own liveliness, or follow Settings again", async ({ page }) => {
  await mockApp(page);
  await page.goto(`/ui.html#card/${PET_ID}`);
  await page.getByText("Pet settings").click();
  await page.getByLabel("Liveliness").selectOption("lively");
  await expect.poll(async () => (await calls(page, "set_pet")).at(-1)?.args).toMatchObject({ id: PET_ID, liveliness: "lively" });
  await page.getByLabel("Liveliness").selectOption("");
  await expect.poll(async () => (await calls(page, "set_pet")).at(-1)?.args).toMatchObject({ liveliness: null });
});

test("the log can be saved for a bug report, or sent after reading exactly what goes", async ({ page }) => {
  await mockApp(page);
  await page.goto("/ui.html#settings/privacy");
  await page.getByRole("button", { name: "Save log for a bug report…" }).click();
  await expect.poll(async () => (await calls(page, "open_link")).at(-1)?.args).toEqual({ which: "new-issue" });
  await expect(page.getByText(/Saved \/tmp\/strays-log.txt/)).toBeVisible();

  await page.getByRole("button", { name: "Send log to the developer…" }).click();
  await expect(page.getByLabel("The log that would be sent")).toContainText("WARN save: Couldn't save <name>");
  expect(await calls(page, "send_log")).toHaveLength(0);
  await page.getByRole("button", { name: "Send this" }).click();
  await expect.poll(async () => (await calls(page, "send_log")).length).toBe(1);
  await expect(page.getByLabel("The log that would be sent")).toBeHidden();
  await page.screenshot({ path: `${shots}/settings-privacy-log.png` });
});

test("adopting right after connecting GitHub: the repo list fills in when the first sync lands", async ({ page }) => {
  await mockApp(page, {}, { repos: [], lastSync: null });
  await page.goto("/ui.html#adopt");
  await expect(page.getByLabel("Repository").locator("option").first()).toHaveText("Fetching your repos…");
  await page.getByLabel("Name").fill("Pixel");
  await page.evaluate(() => {
    const w = window as unknown as { __answers: { ui_state: { repos: unknown[]; lastSync: string } }; __emit: (e: string, p: unknown) => void };
    w.__answers.ui_state = { ...w.__answers.ui_state, lastSync: "2026-11-04T10:00:00Z", repos: [{ id: 2, fullName: "me/side-project", pushedAt: null, private: false, tracked: false }] };
    w.__emit("state-changed", null);
  });
  await expect(page.getByLabel("Repository").locator("option")).toHaveText(["Choose a GitHub repo…", "me/side-project"]);
  await expect(page.getByLabel("Name")).toHaveValue("Pixel");
});

test("marking a release takes a second click, so a slip can't send a pet away", async ({ page }) => {
  await mockApp(page);
  await page.goto(`/ui.html#card/${PET_ID}`);
  await page.getByRole("button", { name: "Mark release…" }).click();
  expect(await calls(page, "pet_action")).toHaveLength(0);
  await page.getByRole("button", { name: "Shipped it? Click again" }).click();
  expect((await calls(page, "pet_action")).at(-1)?.args).toMatchObject({ id: PET_ID, action: "release" });
});

test("snooze until a picked time, pause until a date, and a release with its version", async ({ page }) => {
  await mockApp(page);
  await page.goto(`/ui.html#card/${PET_ID}`);
  await page.getByLabel("Snooze", { exact: true }).selectOption("custom");
  await page.getByLabel("Snooze until").fill("2030-01-02T10:30");
  await page.getByRole("button", { name: "Snooze", exact: true }).click();
  const last = async () => (await calls(page, "pet_action")).at(-1)?.args as { action: string; until: string | null; label: string | null };
  await expect.poll(last).toMatchObject({ action: "snooze-until", until: new Date("2030-01-02T10:30").toISOString() });

  await page.getByLabel("Pause until (optional)").fill("2030-02-01");
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  await expect.poll(last).toMatchObject({ action: "pause", until: new Date("2030-02-01T09:00").toISOString() });

  await page.getByLabel("Release version (optional)").fill("v1.2");
  await page.getByRole("button", { name: "Mark release…" }).click();
  await page.getByRole("button", { name: "Shipped it? Click again" }).click();
  await expect.poll(last).toMatchObject({ action: "release", label: "v1.2" });
});

test("a pet can have its own mood days", async ({ page }) => {
  await mockApp(page);
  await page.goto(`/ui.html#card/${PET_ID}`);
  await page.getByText("Pet settings").click();
  await page.getByLabel("Pace").selectOption("custom");
  await expect.poll(async () => (await calls(page, "set_pet")).at(-1)?.args).toMatchObject({ thresholds: [2, 5, 10, 20], pace: "normal" });
  await page.getByLabel("Hungry up to (days), this pet").fill("30");
  await page.getByLabel("Hungry up to (days), this pet").press("Tab");
  await expect.poll(async () => (await calls(page, "set_pet")).at(-1)?.args).toMatchObject({ thresholds: [2, 5, 10, 30] });
  // out of order: refused here, nothing saved
  const before = (await calls(page, "set_pet")).length;
  await page.getByLabel("Content up to (days), this pet").fill("40");
  await page.getByLabel("Content up to (days), this pet").press("Tab");
  await expect(page.getByText("Each level must last longer than the one before it.")).toBeVisible();
  expect(await calls(page, "set_pet")).toHaveLength(before);
  // back to a preset clears them
  await page.getByLabel("Pace").selectOption("sprint");
  await expect.poll(async () => (await calls(page, "set_pet")).at(-1)?.args).toMatchObject({ thresholds: null, pace: "sprint" });
});

test("a folder-only pet starts on the Slow-burn pace", async ({ page }) => {
  await mockApp(page);
  await page.goto("/ui.html#adopt");
  await expect(page.getByLabel("Pace")).toHaveValue("normal");
  await page.getByLabel("Folder").selectOption({ index: 1 });
  await expect(page.getByLabel("Pace")).toHaveValue("slowBurn");
});
