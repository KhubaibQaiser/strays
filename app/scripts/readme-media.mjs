// Records the README's pet GIFs (docs/media/*.gif) from the real 3D pets.
//
// Needs the dev server (`npm run dev`, port 1420) and ffmpeg. Run from app/:
//   node scripts/readme-media.mjs            every scene
//   node scripts/readme-media.mjs gloomy     only scenes whose name contains "gloomy"
//
// Each scene loads the 3D test page with `manual=1`, steps the pets by exact time steps on the real GPU
// (Chromium's Metal backend; headless Chromium would otherwise draw in software with less fur and
// light), screenshots every frame, and lets ffmpeg build a small looping GIF with its own palette.
import { chromium } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const OUT = new URL("../../docs/media/", import.meta.url).pathname;
const FPS = 12;
const SIZE = 170; // each pet's canvas, in CSS px
const ALL = "cat,dog,hamster,parrot";

/** name, page query, seconds to settle first, seconds recorded, and an optional setup run in the page */
const SCENES = [
  { name: "pets", q: `species=${ALL}&mood=content`, settle: 1, secs: 4 },
  ...["joyful", "content", "peckish", "hungry", "gloomy"].map((mood) => ({ name: `mood-${mood}`, q: `species=${ALL}&mood=${mood}`, settle: 2, secs: 3 })),
  { name: "eating", q: `species=${ALL}&mood=peckish&act=1`, settle: 0.3, secs: 3.5 },
  { name: "celebrating", q: `species=${ALL}&mood=joyful&act=3`, settle: 0.1, secs: 3 },
  { name: "sleeping", q: `species=${ALL}&act=7`, settle: 6, secs: 3 },
  { name: "postcard", q: `species=${ALL}&mood=joyful&act=9`, settle: 0.6, secs: 2.5 },
  // every graduate wears its reward: cap, scarf, bow, star badge, flower crown
  { name: "graduates", q: `species=cat,dog,hamster,parrot,cat&pets=5&mood=joyful`, settle: 1, secs: 3, setup: () => window.cats.forEach(({ cat }, i) => cat.wear(i + 1)) },
];

const only = process.argv[2];
mkdirSync(OUT, { recursive: true });
const browser = await chromium.launch({ headless: true, args: ["--use-angle=metal", "--enable-gpu", "--ignore-gpu-blocklist"] });
for (const scene of SCENES.filter((s) => !only || s.name.includes(only))) {
  const pets = Number(new URLSearchParams(scene.q).get("pets") ?? 4);
  const width = 40 + pets * (SIZE + 30);
  const page = await browser.newPage({ viewport: { width: width + 20, height: SIZE + 80 }, deviceScaleFactor: 2 });
  await page.goto(`http://localhost:1420/proto3d.html?${scene.q}&pets=${pets}&size=${SIZE}&stats=0&bg=flat&manual=1`);
  await page.waitForFunction(() => typeof window.step === "function");
  if (scene.setup) await page.evaluate(scene.setup);
  // the pets cap one update at 1/20 s, so each frame is two half-steps
  const step = (secs) => page.evaluate(([n, dt]) => { for (let i = 0; i < n; i++) window.step(dt); }, [Math.round(secs * FPS * 2), 1 / (FPS * 2)]);
  await step(scene.settle);
  const dir = mkdtempSync(join(tmpdir(), "strays-media-"));
  const frames = Math.round(scene.secs * FPS);
  for (let f = 0; f < frames; f++) {
    await step(1 / FPS);
    // let the compositor show the frame just drawn before taking it
    await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
    await page.screenshot({ path: join(dir, `f${String(f).padStart(3, "0")}.png`), clip: { x: 20, y: 30, width, height: SIZE + 20 } });
  }
  const out = join(OUT, `${scene.name}.gif`);
  execFileSync("ffmpeg", ["-y", "-loglevel", "error", "-framerate", String(FPS), "-i", join(dir, "f%03d.png"),
    "-vf", `scale=${Math.round(width * 0.8)}:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle`,
    "-loop", "0", out]);
  rmSync(dir, { recursive: true });
  console.log(`${scene.name}.gif`);
  await page.close();
}
await browser.close();
