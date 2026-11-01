// Pet overlay: draws the pets the Rust core places, and plays their moments at the chosen liveliness.
// The core owns all state; this page only renders and reports ⌥-clicks.
import { setFps } from "./throttle";
import { Rive, RuntimeLoader } from "@rive-app/canvas";
import riveWasmUrl from "@rive-app/canvas/rive.wasm?url";
import catUrl from "../../art/cat/build/cat.riv?url";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type Liveliness = "calm" | "gentle" | "lively";
type Placed = { id: string; species: string; colors: Record<string, string>; label: string; mood: string; x: number; y: number; s: number };
type Frame = { pets: Placed[]; liveliness: Liveliness; reduceMotion: boolean };

const log = (msg: string) => void invoke("pet_log", { msg });
addEventListener("error", (e) => log(`ERROR ${e.message}`));
addEventListener("unhandledrejection", (e) => log(`REJECT ${String(e.reason)}`));
log(`BOOT ${location.href}`);

// Bundled locally: the runtime must never fetch its WASM from a CDN.
RuntimeLoader.setWasmUrl(riveWasmUrl);

// ponytail: every species renders as the cat until the other rigs land in Gate 3.
const ART: Record<string, string> = { cat: catUrl };

type Pet = { el: HTMLCanvasElement; rive: Rive; placed: Placed; timer?: number; playing: boolean };
const pets = new Map<string, Pet>();
let mode: Liveliness = "calm";
let reduceMotion = false;
let frozen = false;

/** Seconds between short moments per pet (§7.4); Lively plays continuously instead. */
const GAP: Record<Exclude<Liveliness, "lively">, [number, number]> = { calm: [20, 60], gentle: [5, 15] };
const MOMENT_MS = 1000;

function hexToArgb(hex: string): number {
  return (0xff000000 | parseInt(hex.slice(1), 16)) >>> 0;
}

function setPlaying(p: Pet, on: boolean) {
  if (on === p.playing) return;
  p.playing = on;
  if (on) p.rive.play();
  else p.rive.pause();
}

/** Schedules this pet's next moment, or keeps it moving in Lively. Rendering stops between moments. */
function schedule(p: Pet) {
  window.clearTimeout(p.timer);
  if (frozen) return setPlaying(p, false);
  const level = reduceMotion ? "calm" : mode;
  if (level === "lively") return setPlaying(p, true);
  setPlaying(p, false);
  const [lo, hi] = GAP[level];
  p.timer = window.setTimeout(() => {
    setPlaying(p, true);
    p.timer = window.setTimeout(() => schedule(p), MOMENT_MS);
  }, (lo + Math.random() * (hi - lo)) * 1000);
}

function position(p: Pet) {
  const { x, y, s } = p.placed;
  Object.assign(p.el.style, { left: `${x}px`, top: `${y}px`, width: `${s}px`, height: `${s}px` });
  const size = Math.round(s * devicePixelRatio);
  if (p.el.width !== size) {
    p.el.width = p.el.height = size;
    p.rive.resizeDrawingSurfaceToCanvas();
  }
}

function add(placed: Placed) {
  const el = document.createElement("canvas");
  el.dataset.id = placed.id;
  el.width = el.height = Math.round(placed.s * devicePixelRatio);
  document.body.appendChild(el);
  const rive: Rive = new Rive({
    src: ART[placed.species] ?? catUrl,
    canvas: el,
    autoplay: true,
    stateMachines: "Idle",
    autoBind: true,
    onLoadError: (e) => log(`LOADERROR pet=${placed.id} ${JSON.stringify(e.data)}`),
    onLoad: () => {
      rive.resizeDrawingSurfaceToCanvas();
      const vmi = rive.viewModelInstance;
      let bound = 0;
      for (const [name, hex] of Object.entries(placed.colors)) {
        const prop = vmi?.color(name);
        if (prop) {
          prop.value = hexToArgb(hex);
          bound++;
        }
      }
      log(`LOADED pet=${placed.id} colours=${bound}/${Object.keys(placed.colors).length}`);
      // let the first frame draw with the new colours, then hold still until a moment
      window.setTimeout(() => schedule(p), 300);
    },
  });
  const p: Pet = { el, rive, placed, playing: true };
  pets.set(placed.id, p);
  position(p);
  el.addEventListener("mousedown", (e) => {
    if (!e.altKey) return;
    e.preventDefault();
    void invoke("pet_click", { id: placed.id, shift: e.shiftKey });
  });
  el.addEventListener("contextmenu", (e) => e.preventDefault());
}

function remove(id: string) {
  const p = pets.get(id);
  if (!p) return;
  window.clearTimeout(p.timer);
  p.rive.cleanup();
  p.el.remove();
  pets.delete(id);
}

function bubble(id: string, text: string, ms: number, className = "bubble") {
  const p = pets.get(id);
  if (!p) return;
  document.querySelectorAll(`.${className}[data-for="${id}"]`).forEach((b) => b.remove());
  const b = document.createElement("div");
  b.className = className;
  b.dataset.for = id;
  b.textContent = text;
  b.style.left = `${p.placed.x + p.placed.s / 2}px`;
  // above the pet's head; pets at the top edge get it below instead
  const above = p.placed.y + p.placed.s * 0.25 - 34;
  b.style.top = `${above > 0 ? above : p.placed.y + p.placed.s}px`;
  document.body.appendChild(b);
  if (ms > 0) window.setTimeout(() => b.remove(), ms);
}

const ready: Promise<unknown>[] = [];
ready.push(listen<Frame>("overlay", ({ payload }) => {
  const modeChanged = payload.liveliness !== mode || payload.reduceMotion !== reduceMotion;
  mode = payload.liveliness;
  reduceMotion = payload.reduceMotion;
  setFps(mode === "lively" && !reduceMotion ? 15 : 24);
  const keep = new Set(payload.pets.map((p) => p.id));
  [...pets.keys()].filter((id) => !keep.has(id)).forEach(remove);
  for (const placed of payload.pets) {
    const p = pets.get(placed.id);
    if (!p) add(placed);
    else {
      p.placed = placed;
      position(p);
      if (modeChanged) schedule(p);
    }
  }
}));

// ⌥ held: every pet freezes, and the one under the cursor gets an outline and a tooltip (§7.2, §7.3).
ready.push(listen<{ id: string | null; held: boolean }>("hover", ({ payload }) => {
  if (payload.held !== frozen) {
    frozen = payload.held;
    pets.forEach(schedule);
  }
  document.querySelectorAll(".tooltip").forEach((b) => b.remove());
  for (const [id, p] of pets) {
    p.el.classList.toggle("hover", id === payload.id);
    if (id === payload.id) bubble(id, p.placed.label, 0, "tooltip");
  }
}));

ready.push(listen<{ id: string; ok: boolean; text: string }>("feedback", ({ payload }) => bubble(payload.id, payload.text, 2500)));
// a nudge stays a little longer and needs no click (§5.10)
ready.push(listen<{ id: string; text: string }>("nudge", ({ payload }) => bubble(payload.id, payload.text, 8000)));

// Ask for the first frame only once every listener is registered, or it can arrive before them.
void Promise.all(ready).then(() => invoke("overlay_ready"));
