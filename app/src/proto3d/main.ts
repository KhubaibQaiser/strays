// Prototype page for realistic 3D pets: renders N pets and reports their frame cost. Everything is a
// query parameter, so CPU can be measured per setting: species (comma list, cycled), pets, size (pt),
// fps, idle (fps when only slow motion is happening), shells, dpr, mood, look, face (close-up), bg.
import { Pet3D, type Mood } from "../pet3d/pet";
import { SPECIES_3D } from "../pet3d/rig";
import type { Palette } from "../pet3d/species";
import { invoke } from "@tauri-apps/api/core";

// inside the app's overlay, stats go to the app log (the page itself is click-through and unseen)
const inApp = "__TAURI_INTERNALS__" in window;

const q = new URLSearchParams(location.search);
const num = (k: string, d: number) => (q.has(k) ? Number(q.get(k)) : d);
const pets = num("pets", 1);
const size = num("size", 104);
const fps = num("fps", 60);
/** frame rate while only slow motion (breathing, sway) is happening */
const idleFps = num("idle", fps);
const dpr = num("dpr", devicePixelRatio);
const species = (q.get("species") ?? "cat").split(",");
if (q.get("bg") === "1") document.body.classList.add("bg");

type Look = { colors: Record<string, string>; extra?: Partial<Palette> };
const coat = (coatBase: string, coatShade: string, coatHighlight: string, outline = "#333333") => ({ coatBase, coatShade, coatHighlight, outline });
/** A few coats per species, as the app would send them (plus palette tweaks for demos). */
const LOOKS: Record<string, Look[]> = {
  cat: [
    { colors: coat("#ecd3ad", "#c49a68", "#f8eedd"), extra: { tipping: 0 } }, // cream, copper eyes
    { colors: coat("#eceef0", "#5b5f66", "#fafafa"), extra: { iris: "#3f9a5a", irisOuter: "#2c4a22", tipping: 0.8 } }, // silver chinchilla
    { colors: coat("#d2893f", "#8d4a1c", "#f0d2ad"), extra: { stripes: 0.8, tipping: 0 } }, // red tabby
  ],
  dog: [{ colors: coat("#D98A4A", "#B86C2E", "#F2B57E") }, { colors: coat("#2c2522", "#1a1513", "#c98a4b") }, { colors: coat("#e8d2b0", "#c9aa80", "#fbf3e6") }],
  hamster: [{ colors: coat("#E3A564", "#C4843F", "#F3C98F") }, { colors: coat("#d9d4cc", "#a59d92", "#f6f3ee") }, { colors: coat("#8a7464", "#5f4e42", "#e8ddd0") }],
  parrot: [{ colors: coat("#7DC44E", "#5EA233", "#A6DE7B") }, { colors: coat("#5aa8e0", "#3a7fb8", "#a6d4f2") }, { colors: coat("#e8e070", "#c8bd40", "#f6f2b8") }],
};

const t0 = performance.now();
for (const s of species) SPECIES_3D[s]?.shape();
const buildMs = performance.now() - t0;

const cats = Array.from({ length: pets }, (_, i) => {
  const name = species[i % species.length];
  const def = SPECIES_3D[name];
  const canvas = document.createElement("canvas");
  Object.assign(canvas.style, { left: `${40 + i * (size + 30)}px`, top: "40px", width: `${size}px`, height: `${size}px` });
  document.body.appendChild(canvas);
  const looks = LOOKS[name] ?? LOOKS.cat;
  const look = looks[(num("look", 0) + Math.floor(i / species.length)) % looks.length];
  const cat = new Pet3D(canvas, def, { ...def.palette(look.colors), ...look.extra });
  if (q.has("shells")) cat.setShells(num("shells", def.fur.shells));
  cat.mood = (q.get("mood") as Mood) ?? "content";
  cat.setSize(size, dpr);
  // ?face=1 frames the head, for checking detail
  if (q.get("face") === "1") {
    const h = def.eyes.at;
    cat.camera.position.set(-0.25, h[1] - 0.06, h[2] + 0.8);
    cat.camera.lookAt(0.02, h[1] - 0.1, 0);
  }
  return { cat, canvas };
});

(window as unknown as { cats: typeof cats }).cats = cats; // for poking at from the console
// the cats watch the cursor while it's near them
addEventListener("mousemove", (e) => {
  for (const { cat, canvas } of cats) {
    const r = canvas.getBoundingClientRect();
    const x = (e.clientX - (r.left + r.width / 2)) / (r.width * 2);
    const y = -(e.clientY - (r.top + r.height * 0.3)) / (r.height * 2);
    cat.cursor = Math.hypot(x, y) < 2 ? { x, y } : null;
  }
});
addEventListener("mouseleave", () => cats.forEach(({ cat }) => (cat.cursor = null)));

const stats = q.get("stats") === "0" ? null : Object.assign(document.createElement("div"), { id: "stats" });
if (stats) document.body.appendChild(stats);
// each cat keeps its own frame clock: it draws at `fps` while something quick moves, `idleFps` otherwise
const lastDrawn = cats.map(() => performance.now());
let frames = 0, work = 0, windowStart = performance.now();
function frame(now: number) {
  requestAnimationFrame(frame);
  const start = performance.now();
  let drew = 0;
  cats.forEach(({ cat }, i) => {
    if (now - lastDrawn[i] < 1000 / (cat.fast ? fps : idleFps) - 2) return;
    cat.update((now - lastDrawn[i]) / 1000);
    cat.render();
    lastDrawn[i] = now;
    drew++;
  });
  if (!drew) return;
  work += performance.now() - start;
  frames += drew / Math.max(cats.length, 1);
  if (inApp && now - windowStart > 5000) {
    void invoke("pet_log", { msg: `PROTO3D fps=${((frames * 1000) / (now - windowStart)).toFixed(1)} work=${(work / frames).toFixed(2)}ms pets=${pets}` });
    frames = 0;
    work = 0;
    windowStart = now;
  }
  if (stats && now - windowStart > 1000) {
    const s = SPECIES_3D[species[0]].shape();
    stats.textContent =
      `${pets} pet(s) · ${size}pt · dpr ${dpr} · ${species.join("+")} · ${s.index.length / 3} tris/shell · build ${buildMs.toFixed(0)} ms\n` +
      `${((frames * 1000) / (now - windowStart)).toFixed(0)} fps · JS+GL submit ${(work / frames).toFixed(2)} ms/frame`;
    frames = 0;
    work = 0;
    windowStart = now;
  }
}
requestAnimationFrame(frame);
