// Prototype page for realistic 3D pets: renders N cats and reports their frame cost. Everything is a
// query parameter, so CPU can be measured per setting: pets, size (pt), fps, idle (fps when only slow
// motion is happening), shells, dpr, fur, density, mood, look.
import { Cat3D, catShape, type Look, type Mood } from "./cat";
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
const quality = { shells: num("shells", 28), furLen: num("fur", 0.07), density: num("density", 150) };
if (q.get("bg") === "1") document.body.classList.add("bg");

const LOOKS: Look[] = [
  { base: "#ecd3ad", shade: "#c49a68", light: "#f8eedd", pink: "#d97a80", iris: "#c8701f", stripes: 0, tipping: 0 }, // cream, copper eyes
  { base: "#eceef0", shade: "#5b5f66", light: "#fafafa", pink: "#e2a0a2", iris: "#3f9a5a", stripes: 0, tipping: 0.8 }, // silver chinchilla
  { base: "#d2893f", shade: "#8d4a1c", light: "#f0d2ad", pink: "#d98c85", iris: "#c46a1c", stripes: 0.8, tipping: 0 }, // red tabby
];

const t0 = performance.now();
catShape();
const buildMs = performance.now() - t0;

const cats = Array.from({ length: pets }, (_, i) => {
  const canvas = document.createElement("canvas");
  Object.assign(canvas.style, { left: `${40 + i * (size + 30)}px`, top: "40px", width: `${size}px`, height: `${size}px` });
  document.body.appendChild(canvas);
  const cat = new Cat3D(canvas, LOOKS[(num("look", 0) + i) % LOOKS.length], quality);
  cat.mood = (q.get("mood") as Mood) ?? "content";
  cat.setSize(size, dpr);
  // ?face=1 frames the head, for checking detail
  if (q.get("face") === "1") {
    cat.camera.position.set(-0.25, 0.72, 0.95);
    cat.camera.lookAt(0.02, 0.68, 0);
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
    const s = catShape();
    stats.textContent =
      `${pets} pet(s) · ${size}pt · dpr ${dpr} · ${quality.shells} shells · ${s.index.length / 3} tris/shell · build ${buildMs.toFixed(0)} ms\n` +
      `${((frames * 1000) / (now - windowStart)).toFixed(0)} fps · JS+GL submit ${(work / frames).toFixed(2)} ms/frame`;
    frames = 0;
    work = 0;
    windowStart = now;
  }
}
requestAnimationFrame(frame);
