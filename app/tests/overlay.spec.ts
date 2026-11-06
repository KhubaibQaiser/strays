// The pet overlay in Chromium with the real Rive runtime: frames place pets, cues play clips.
import { expect, test } from "@playwright/test";
import { mockApp, placed as pet } from "./mock";

const shots = "test-results/screens";
const emit = (page: import("@playwright/test").Page, event: string, payload: unknown) =>
  page.evaluate(([e, p]) => (window as unknown as { __emit: (e: string, p: unknown) => void }).__emit(e as string, p), [event, payload]);

test("moods pick poses, and a release cue celebrates then leaves", async ({ page }) => {
  // the celebrate, exit and fade sequence alone runs ~17 s, more than half the default limit
  test.setTimeout(60_000);
  await mockApp(page);
  await page.setViewportSize({ width: 660, height: 240 });
  await page.goto("/index.html");
  await page.waitForFunction(() => (window as unknown as { __calls: { cmd: string }[] }).__calls.some((c) => c.cmd === "overlay_ready"));
  await emit(page, "overlay", { pets: [pet("joy", "joyful", 0), pet("hungry", "hungry", 220), pet("gloomy", "gloomy", 440)], liveliness: "calm", reduceMotion: false, hidden: false });
  // 3D shapes are built in a worker first, which takes a while when the machine is busy
  await expect(page.locator("canvas")).toHaveCount(3, { timeout: 30_000 });
  await page.waitForTimeout(1500);
  await page.screenshot({ path: `${shots}/overlay-moods.png` });

  await emit(page, "cue", { id: "joy", cue: { celebrate: { leave: true } } });
  // the next frame no longer lists it, but it stays until its clip ends
  await emit(page, "overlay", { pets: [pet("hungry", "hungry", 220), pet("gloomy", "gloomy", 440)], liveliness: "calm", reduceMotion: false, hidden: false });
  await page.waitForTimeout(2500);
  await page.screenshot({ path: `${shots}/overlay-celebrate.png` });
  await expect(page.locator('canvas[data-id="joy"]')).toHaveCount(1);
  await expect(page.locator('canvas[data-id="joy"]')).toHaveCount(0, { timeout: 12000 });
});

test("a graduate puts on its accessory, celebrates and leaves", async ({ page }) => {
  await mockApp(page);
  await page.setViewportSize({ width: 440, height: 240 });
  await page.goto("/index.html");
  await page.waitForFunction(() => (window as unknown as { __calls: { cmd: string }[] }).__calls.some((c) => c.cmd === "overlay_ready"));
  await emit(page, "overlay", { pets: [pet("grad", "content", 0)], liveliness: "calm", reduceMotion: false, hidden: false });
  await expect(page.locator("canvas")).toHaveCount(1);
  // cue while the load-time pose blend is still running: the farewell must not be cut short by it
  await page.waitForFunction(() => (window as unknown as { __calls: { cmd: string; args: { msg?: string } }[] }).__calls.some((c) => c.args?.msg?.startsWith("LOADED")));
  await emit(page, "cue", { id: "grad", cue: { graduate: { accessory: 1 } } });
  await emit(page, "overlay", { pets: [], liveliness: "calm", reduceMotion: false, hidden: false });
  await page.waitForTimeout(800);
  await page.screenshot({ path: `${shots}/overlay-graduate.png` });
  // celebrate + exit last ~8 s; the old bug faded it out ~1.7 s after the cue
  await page.waitForTimeout(2500);
  await expect(page.locator('canvas[data-id="grad"]')).toHaveCount(1, { timeout: 100 });
  await expect(page.locator('canvas[data-id="grad"]')).toHaveCount(0, { timeout: 14000 });
});

test("with Reduce Motion a leaving pet only fades", async ({ page }) => {
  await mockApp(page);
  await page.goto("/index.html");
  await page.waitForFunction(() => (window as unknown as { __calls: { cmd: string }[] }).__calls.some((c) => c.cmd === "overlay_ready"));
  await emit(page, "overlay", { pets: [pet("a", "content", 0)], liveliness: "calm", reduceMotion: true, hidden: false });
  await expect(page.locator("canvas")).toHaveCount(1);
  await emit(page, "cue", { id: "a", cue: "fallAsleep" });
  await expect(page.locator('canvas[data-id="a"]')).toHaveCount(0, { timeout: 2000 });
});

test("each species draws with its own model", async ({ page }) => {
  // four 3D pets, each with its own WebGL context; CI draws them in software
  test.setTimeout(60_000);
  await mockApp(page);
  await page.setViewportSize({ width: 880, height: 240 });
  await page.goto("/index.html");
  await page.waitForFunction(() => (window as unknown as { __calls: { cmd: string }[] }).__calls.some((c) => c.cmd === "overlay_ready"));
  const corgi = { ...pet("dog", "joyful", 440), species: "dog", colors: { coatBase: "#D98A4A", coatShade: "#B86C2E", coatHighlight: "#F2B57E", outline: "#6B3A17" } };
  const hamster = { ...pet("hamster", "content", 220), species: "hamster", colors: { coatBase: "#E3A564", coatShade: "#C4843F", coatHighlight: "#F3C98F", outline: "#6E4320" } };
  const parrot = { ...pet("parrot", "joyful", 660), species: "parrot", colors: { coatBase: "#7DC44E", coatShade: "#5EA233", coatHighlight: "#A6DE7B", outline: "#2F5A1C" } };
  await emit(page, "overlay", { pets: [pet("cat", "content", 0), hamster, corgi, parrot], liveliness: "calm", reduceMotion: false, hidden: false });
  // each shape is built in a worker first (several seconds in all when the machine is busy)
  await expect(page.locator("canvas")).toHaveCount(4, { timeout: 30_000 });
  await page.waitForTimeout(1500);
  await page.screenshot({ path: `${shots}/overlay-species.png` });
  const logs = await page.evaluate(() => (window as unknown as { __calls: { cmd: string; args: { msg?: string } }[] }).__calls.filter((c) => c.cmd === "pet_log").map((c) => c.args.msg));
  expect(logs.filter((m) => m?.startsWith("LOADED"))).toHaveLength(4);
  // every species has a 3D model now
  expect(logs.filter((m) => m?.includes("rig=3d"))).toHaveLength(4);
});

test("a pet whose repo GitHub can't find keeps asking until it's answered", async ({ page }) => {
  await mockApp(page);
  await page.setViewportSize({ width: 440, height: 240 });
  await page.goto("/index.html");
  await page.waitForFunction(() => (window as unknown as { __calls: { cmd: string }[] }).__calls.some((c) => c.cmd === "overlay_ready"));
  const frame = (confused: boolean) => ({ pets: [{ ...pet("lost", "content", 220), confused }], liveliness: "calm", reduceMotion: false, hidden: false });
  await emit(page, "overlay", frame(true));
  await expect(page.getByText("❓ Where did my repo go?")).toBeVisible();
  // a passing message doesn't knock the question away
  await emit(page, "feedback", { id: "lost", ok: true, text: "✓" });
  await expect(page.getByText("❓ Where did my repo go?")).toBeVisible();
  await page.screenshot({ path: `${shots}/overlay-confused.png` });
  await emit(page, "overlay", frame(false));
  await expect(page.getByText("❓ Where did my repo go?")).toHaveCount(0);
});

test("a click counts whenever the chosen modifier is held, whichever key that is", async ({ page }) => {
  await mockApp(page);
  await page.setViewportSize({ width: 440, height: 240 });
  await page.goto("/index.html");
  await page.waitForFunction(() => (window as unknown as { __calls: { cmd: string }[] }).__calls.some((c) => c.cmd === "overlay_ready"));
  await emit(page, "overlay", { pets: [pet("p", "content", 220)], liveliness: "calm", reduceMotion: false, hidden: false });
  await expect(page.locator("canvas")).toHaveCount(1);
  const clicks = () => page.evaluate(() => (window as unknown as { __calls: { cmd: string; args: unknown }[] }).__calls.filter((c) => c.cmd === "pet_click"));
  // not held: the window is click-through anyway, and a stray event does nothing
  await page.mouse.click(320, 120);
  expect(await clicks()).toHaveLength(0);
  // Control chosen and held (no ⌥ involved): Rust reports it through "hover"
  await emit(page, "hover", { id: "p", held: true });
  // held: every pet glows faintly, so it's clear they can be clicked now
  await expect(page.locator("body")).toHaveClass(/\bheld\b/);
  expect(await page.locator("canvas").evaluate((c) => getComputedStyle(c).filter)).toContain("drop-shadow");
  await page.keyboard.down("Control");
  await page.mouse.click(320, 120);
  await page.keyboard.up("Control");
  expect(await clicks()).toEqual([{ cmd: "pet_click", args: { id: "p", shift: false } }]);
  await emit(page, "hover", { id: null, held: false });
  await expect(page.locator("body")).not.toHaveClass(/\bheld\b/);
});

test("a full-screen app hides pets without rebuilding them", async ({ page }) => {
  await mockApp(page);
  await page.goto("/index.html");
  await page.waitForFunction(() => (window as unknown as { __calls: { cmd: string }[] }).__calls.some((c) => c.cmd === "overlay_ready"));
  const frame = (hidden: boolean) => ({ pets: [pet("a", "content", 0)], liveliness: "gentle", reduceMotion: false, hidden });
  await emit(page, "overlay", frame(false));
  await expect(page.locator("canvas")).toHaveCount(1);
  // mark this canvas, so a rebuilt one would show up as unmarked
  await page.locator("canvas").evaluate((c) => (c.dataset.same = "1"));
  await emit(page, "overlay", frame(true));
  await expect(page.locator("canvas")).toBeHidden();
  await emit(page, "overlay", frame(false));
  await expect(page.locator('canvas[data-same="1"]')).toBeVisible();
});

test("a script error in the overlay is reported, not just logged", async ({ page }) => {
  await mockApp(page);
  await page.goto("/index.html");
  await page.waitForFunction(() => (window as unknown as { __calls: { cmd: string }[] }).__calls.some((c) => c.cmd === "overlay_ready"));
  await page.evaluate(() => setTimeout(() => { throw new Error("boom"); }));
  const reported = () => page.evaluate(() => (window as unknown as { __calls: { cmd: string; args: { key?: string; text?: string } }[] }).__calls.filter((c) => c.cmd === "report_problem").map((c) => c.args));
  await expect.poll(reported).toContainEqual({ key: "script", text: expect.stringContaining("boom") });
});

test("with the modifier held, a pet can be dragged onto the grid; a press that doesn't move is a click", async ({ page }) => {
  await mockApp(page);
  await page.setViewportSize({ width: 800, height: 500 });
  await page.goto("/index.html");
  await page.waitForFunction(() => (window as unknown as { __calls: { cmd: string }[] }).__calls.some((c) => c.cmd === "overlay_ready"));
  const grid = { x0: 580, y0: 280, cw: 140, ch: 140, cols: 5, rows: 3, s: 200 };
  await emit(page, "overlay", { pets: [pet("a", "content", 580)], grid, liveliness: "gentle", reduceMotion: false, hidden: false });
  await expect(page.locator("canvas")).toHaveCount(1, { timeout: 30_000 });
  await emit(page, "hover", { id: "a", held: true });
  const calls = (cmd: string) => page.evaluate((c) => (window as unknown as { __calls: { cmd: string; args: unknown }[] }).__calls.filter((x) => x.cmd === c).map((x) => x.args), cmd);

  // a press and let go in place: a click
  await page.mouse.move(680, 140);
  await page.mouse.down();
  await page.mouse.up();
  await expect.poll(() => calls("pet_click")).toHaveLength(1);
  expect(await calls("pet_drop")).toHaveLength(0);

  // press, move: the grid shows; let go: Rust is asked to drop it there
  await page.mouse.move(680, 140);
  await page.mouse.down();
  await page.mouse.move(500, 100, { steps: 5 });
  await expect(page.locator("#grid .cell")).toHaveCount(15);
  await expect(page.locator("#grid .cell.target")).toHaveCount(1);
  await page.screenshot({ path: `${shots}/overlay-drag.png` });
  await page.mouse.move(400, 60, { steps: 5 });
  await page.mouse.up();
  await expect.poll(() => calls("pet_drop")).toHaveLength(1);
  const [drop] = (await calls("pet_drop")) as { id: string; x: number; y: number }[];
  expect(drop).toEqual({ id: "a", x: 580 - 280, y: 20 - 80 });
  await expect(page.locator("#grid")).toHaveCount(0);
  expect(await calls("pet_click")).toHaveLength(1);

  // a trackpad press that eases off mid-drag and presses again at once carries the drag on: one drop, at the end
  await page.mouse.move(400, 300);
  await emit(page, "overlay", { pets: [pet("a", "content", 580)], grid, liveliness: "gentle", reduceMotion: false, hidden: false });
  await page.mouse.move(680, 140);
  await page.mouse.down();
  await page.mouse.move(600, 140, { steps: 4 });
  await page.mouse.up();
  await page.mouse.down();
  await page.mouse.move(500, 140, { steps: 4 });
  await page.waitForTimeout(400);
  expect(await calls("pet_drop")).toHaveLength(1);
  await page.mouse.up();
  await expect.poll(() => calls("pet_drop")).toHaveLength(2);
  expect(((await calls("pet_drop")) as { x: number }[])[1].x).toBe(580 - 180);
});

test("holding still on a pet pets it (no click), and a right-click opens its menu", async ({ page }) => {
  await mockApp(page);
  await page.setViewportSize({ width: 800, height: 500 });
  await page.goto("/index.html");
  await page.waitForFunction(() => (window as unknown as { __calls: { cmd: string }[] }).__calls.some((c) => c.cmd === "overlay_ready"));
  const grid = { x0: 580, y0: 280, cw: 140, ch: 140, cols: 5, rows: 3, s: 200 };
  await emit(page, "overlay", { pets: [pet("a", "content", 580)], grid, liveliness: "gentle", reduceMotion: true, hidden: false });
  await expect(page.locator("canvas")).toHaveCount(1, { timeout: 30_000 });
  await emit(page, "hover", { id: "a", held: true });
  const calls = (cmd: string) => page.evaluate((c) => (window as unknown as { __calls: { cmd: string; args: unknown }[] }).__calls.filter((x) => x.cmd === c).map((x) => x.args), cmd);

  await page.mouse.move(680, 140);
  await page.mouse.down();
  await page.waitForTimeout(1200);
  await expect(page.locator(".bubble")).toHaveText("♥"); // with Reduce Motion the reaction is a heart
  await page.mouse.up();
  await expect.poll(() => calls("pet_press")).toHaveLength(2);
  expect(await calls("pet_click")).toHaveLength(0);

  await page.mouse.click(680, 140, { button: "right" });
  await expect.poll(() => calls("pet_menu")).toEqual([{ id: "a" }]);
});

test("every few hours a pet strolls a little way and back, but never with Reduce Motion", async ({ page }) => {
  await page.clock.install();
  await page.addInitScript(() => (Math.random = () => 0.9)); // the stroll, not the stretch
  await mockApp(page);
  await page.setViewportSize({ width: 800, height: 400 });
  await page.goto("/index.html");
  await page.waitForFunction(() => (window as unknown as { __calls: { cmd: string }[] }).__calls.some((c) => c.cmd === "overlay_ready"));
  await emit(page, "overlay", { pets: [pet("a", "content", 500)], grid: { x0: 500, y0: 20, cw: 140, ch: 140, cols: 3, rows: 1, s: 200 }, liveliness: "gentle", reduceMotion: false, hidden: false });
  await expect(page.locator("canvas")).toHaveCount(1, { timeout: 30_000 });
  const left = () => page.locator("canvas").evaluate((c) => c.style.left);
  // the first try may find it still settling into its pose; then the retry comes ten minutes later
  await page.clock.fastForward(6 * 3_600_000);
  if ((await left()) !== "380px") await page.clock.fastForward(11 * 60_000);
  await expect.poll(left).toBe("380px"); // towards the middle: 0.6 of its 200 px canvas
  await page.clock.fastForward(4000);
  await expect.poll(left).toBe("500px");

  await emit(page, "overlay", { pets: [pet("a", "content", 500)], grid: { x0: 500, y0: 20, cw: 140, ch: 140, cols: 3, rows: 1, s: 200 }, liveliness: "gentle", reduceMotion: true, hidden: false });
  await page.clock.fastForward(6 * 3_600_000);
  expect(await left()).toBe("500px");
});

test("holding the modifier keeps pets animating, with a glow", async ({ page }) => {
  await mockApp(page);
  await page.setViewportSize({ width: 440, height: 240 });
  await page.goto("/index.html");
  await page.waitForFunction(() => (window as unknown as { __calls: { cmd: string }[] }).__calls.some((c) => c.cmd === "overlay_ready"));
  await emit(page, "overlay", { pets: [pet("p", "content", 220)], liveliness: "lively", reduceMotion: false, hidden: false });
  await expect(page.locator("canvas")).toHaveCount(1, { timeout: 30_000 });
  await page.waitForTimeout(1500);
  await emit(page, "hover", { id: null, held: true });
  await page.waitForTimeout(3000);
  await expect(page.locator("body")).toHaveClass(/\bheld\b/);
  // frames taken apart differ only if the pet is still being drawn
  const canvas = page.locator("canvas");
  const a = await canvas.screenshot();
  await page.waitForTimeout(700);
  const b = await canvas.screenshot();
  expect(a.equals(b), "the pet stopped animating while the modifier was held").toBe(false);
});
