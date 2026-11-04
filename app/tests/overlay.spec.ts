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
  await expect(page.locator("canvas")).toHaveCount(3);
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
  await page.keyboard.down("Control");
  await page.mouse.click(320, 120);
  await page.keyboard.up("Control");
  expect(await clicks()).toEqual([{ cmd: "pet_click", args: { id: "p", shift: false } }]);
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
