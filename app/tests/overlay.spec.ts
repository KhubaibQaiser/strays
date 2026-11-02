// The pet overlay in Chromium with the real Rive runtime: frames place pets, cues play clips.
import { expect, test } from "@playwright/test";
import { mockApp } from "./mock";

const shots = "test-results/screens";
const pet = (id: string, mood: string, x: number) => ({
  id,
  species: "cat",
  colors: { coatBase: "#E8A15C", coatShade: "#CB8240", coatHighlight: "#FBE3C6", outline: "#7A4520" },
  label: `${id} · ${mood}`,
  mood,
  x,
  y: 20,
  s: 200,
});
const emit = (page: import("@playwright/test").Page, event: string, payload: unknown) =>
  page.evaluate(([e, p]) => (window as unknown as { __emit: (e: string, p: unknown) => void }).__emit(e as string, p), [event, payload]);

test("moods pick poses, and a release cue celebrates then leaves", async ({ page }) => {
  await mockApp(page);
  await page.setViewportSize({ width: 660, height: 240 });
  await page.goto("/index.html");
  await page.waitForFunction(() => (window as unknown as { __calls: { cmd: string }[] }).__calls.some((c) => c.cmd === "overlay_ready"));
  await emit(page, "overlay", { pets: [pet("joy", "joyful", 0), pet("hungry", "hungry", 220), pet("gloomy", "gloomy", 440)], liveliness: "calm", reduceMotion: false });
  await expect(page.locator("canvas")).toHaveCount(3);
  await page.waitForTimeout(1500);
  await page.screenshot({ path: `${shots}/overlay-moods.png` });

  await emit(page, "cue", { id: "joy", cue: { celebrate: { leave: true } } });
  // the next frame no longer lists it, but it stays until its clip ends
  await emit(page, "overlay", { pets: [pet("hungry", "hungry", 220), pet("gloomy", "gloomy", 440)], liveliness: "calm", reduceMotion: false });
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
  await emit(page, "overlay", { pets: [pet("grad", "content", 0)], liveliness: "calm", reduceMotion: false });
  await expect(page.locator("canvas")).toHaveCount(1);
  // cue while the load-time pose blend is still running: the farewell must not be cut short by it
  await page.waitForFunction(() => (window as unknown as { __calls: { cmd: string; args: { msg?: string } }[] }).__calls.some((c) => c.args?.msg?.startsWith("LOADED")));
  await emit(page, "cue", { id: "grad", cue: { graduate: { accessory: 1 } } });
  await emit(page, "overlay", { pets: [], liveliness: "calm", reduceMotion: false });
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
  await emit(page, "overlay", { pets: [pet("a", "content", 0)], liveliness: "calm", reduceMotion: true });
  await expect(page.locator("canvas")).toHaveCount(1);
  await emit(page, "cue", { id: "a", cue: "fallAsleep" });
  await expect(page.locator('canvas[data-id="a"]')).toHaveCount(0, { timeout: 2000 });
});

test("each species draws with its own rig", async ({ page }) => {
  await mockApp(page);
  await page.setViewportSize({ width: 880, height: 240 });
  await page.goto("/index.html");
  await page.waitForFunction(() => (window as unknown as { __calls: { cmd: string }[] }).__calls.some((c) => c.cmd === "overlay_ready"));
  const corgi = { ...pet("dog", "joyful", 440), species: "dog", colors: { coatBase: "#D98A4A", coatShade: "#B86C2E", coatHighlight: "#F2B57E", outline: "#6B3A17" } };
  const hamster = { ...pet("hamster", "content", 220), species: "hamster", colors: { coatBase: "#E3A564", coatShade: "#C4843F", coatHighlight: "#F3C98F", outline: "#6E4320" } };
  const parrot = { ...pet("parrot", "joyful", 660), species: "parrot", colors: { coatBase: "#7DC44E", coatShade: "#5EA233", coatHighlight: "#A6DE7B", outline: "#2F5A1C" } };
  await emit(page, "overlay", { pets: [pet("cat", "content", 0), hamster, corgi, parrot], liveliness: "calm", reduceMotion: false });
  await expect(page.locator("canvas")).toHaveCount(4);
  await page.waitForTimeout(1500);
  await page.screenshot({ path: `${shots}/overlay-species.png` });
  const logs = await page.evaluate(() => (window as unknown as { __calls: { cmd: string; args: { msg?: string } }[] }).__calls.filter((c) => c.cmd === "pet_log").map((c) => c.args.msg));
  expect(logs.filter((m) => m?.startsWith("LOADED"))).toHaveLength(4);
});
