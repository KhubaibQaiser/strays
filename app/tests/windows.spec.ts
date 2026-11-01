import { expect, test } from "@playwright/test";
import { PET_ID, calls, mockApp } from "./mock";
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
  expect((await calls(page, "pet_action")).at(-1)?.args).toEqual({ id: PET_ID, action: "graduate" });

  await page.keyboard.press("Escape");
  expect((await calls(page, "close_window")).at(-1)?.args).toEqual({ label: "card" });
});

test("settings save each change and reject thresholds that don't increase", async ({ page }) => {
  await mockApp(page);
  await page.goto("/ui.html#settings/pets");
  await page.getByLabel("slots").fill("5");
  await page.getByLabel("slots").press("Tab");
  expect((await calls(page, "save_settings")).at(-1)?.args.settings).toMatchObject({ slots: 5 });
  await page.screenshot({ path: `${shots}/settings-pets.png` });

  await page.getByRole("button", { name: "Moods" }).click();
  const before = (await calls(page, "save_settings")).length;
  await page.getByLabel("Content up to (days)").fill("1");
  await page.getByLabel("Content up to (days)").press("Tab");
  await expect(page.getByRole("alert")).toHaveText("Each level must last longer than the one before it.");
  expect(await calls(page, "save_settings")).toHaveLength(before);

  await page.getByRole("button", { name: "Display" }).click();
  await page.getByLabel("liveliness").selectOption({ label: "Lively: always moving (about 11% CPU, noticeably more battery)" });
  expect((await calls(page, "save_settings")).at(-1)?.args.settings).toMatchObject({ liveliness: "lively" });
  await page.screenshot({ path: `${shots}/settings-display.png` });
});

test("sound settings: off by default, a master switch, volume, per-species and a test button", async ({ page }) => {
  await mockApp(page);
  await page.goto("/ui.html#settings/sound");
  await expect(page.getByLabel("Play sounds")).not.toBeChecked();
  await page.getByLabel("Play sounds").check();
  expect((await calls(page, "save_settings")).at(-1)?.args.settings).toMatchObject({ sound: true });
  await page.getByLabel("Cats").uncheck();
  expect((await calls(page, "save_settings")).at(-1)?.args.settings).toMatchObject({ mutedSpecies: ["cat"] });
  await page.getByLabel("Volume").fill("80");
  await page.getByLabel("Volume").dispatchEvent("change");
  expect((await calls(page, "save_settings")).at(-1)?.args.settings).toMatchObject({ volume: 80 });
  await page.getByRole("button", { name: "Test" }).click();
  expect(await calls(page, "test_sound")).toHaveLength(1);
  await page.screenshot({ path: `${shots}/settings-sound.png` });
});

test("privacy page shows what GitHub access can and can't do, and guards delete", async ({ page }) => {
  await mockApp(page);
  await page.goto("/ui.html#settings/privacy");
  await expect(page.getByText("Account: me · sees 96 repos")).toBeVisible();
  await page.getByRole("button", { name: "View activity log" }).click();
  await expect(page.getByText("GET api.github.com/user/repos?sort=pushed")).toBeVisible();
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
  await page.screenshot({ path: `${shots}/adopt.png`, fullPage: true });
  await page.getByRole("button", { name: "Adopt", exact: true }).click();
  expect((await calls(page, "adopt")).at(-1)?.args.request).toEqual({
    repoId: 2,
    folder: null,
    species: "parrot",
    colors: looksFor("parrot")[2], // budgies get their own swatches
    name: "Biscuit",
    goal: "Launch it",
    pace: "slowBurn",
  });
});

test("welcome walks three steps and saves the comfort settings", async ({ page }) => {
  await mockApp(page);
  await page.setViewportSize({ width: 560, height: 560 });
  await page.goto("/ui.html#welcome");
  await expect(page.getByText("Strays works offline. Nothing is on until you choose.")).toBeVisible();
  await page.screenshot({ path: `${shots}/welcome.png` });
  await page.getByRole("button", { name: "Continue" }).click();
  await page.getByRole("button", { name: "Add a folder…" }).click();
  await page.getByRole("button", { name: "Continue" }).click();
  await page.getByRole("button", { name: "Adopt your first pet…" }).click();
  expect((await calls(page, "save_settings")).at(-1)?.args.settings).toMatchObject({ watchedFolders: ["/Users/me/Code"] });
  expect((await calls(page, "open_window")).at(-1)?.args).toEqual({ kind: "adopt" });
});
