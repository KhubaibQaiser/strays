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
  await page.setViewportSize({ width: 440, height: 240 });
  await page.goto("/index.html");
  await page.waitForFunction(() => (window as unknown as { __calls: { cmd: string }[] }).__calls.some((c) => c.cmd === "overlay_ready"));
  const corgi = { ...pet("dog", "joyful", 220), species: "dog", colors: { coatBase: "#D98A4A", coatShade: "#B86C2E", coatHighlight: "#F2B57E", outline: "#6B3A17" } };
  await emit(page, "overlay", { pets: [pet("cat", "content", 0), corgi], liveliness: "calm", reduceMotion: false });
  await expect(page.locator("canvas")).toHaveCount(2);
  await page.waitForTimeout(1500);
  await page.screenshot({ path: `${shots}/overlay-species.png` });
  const logs = await page.evaluate(() => (window as unknown as { __calls: { cmd: string; args: { msg?: string } }[] }).__calls.filter((c) => c.cmd === "pet_log").map((c) => c.args.msg));
  expect(logs.filter((m) => m?.startsWith("LOADED"))).toHaveLength(2);
});
