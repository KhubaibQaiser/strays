// Pet overlay: draws the pets the Rust core places, and plays their moments at the chosen liveliness.
// The core owns all state; this page only renders and reports ⌥-clicks. Cats are the realistic 3D
// Persian (src/pet3d); the other species, and any cat whose WebGL fails, are drawn by their Rive rigs.
import { setFps } from "./throttle";
import { Rive, RuntimeLoader } from "@rive-app/canvas";
import riveWasmUrl from "@rive-app/canvas/rive.wasm?url";
import { artFor } from "./art";
import { Cat3DRig } from "./pet3d/rig";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type Liveliness = "calm" | "gentle" | "lively";
export type Placed = { id: string; species: string; colors: Record<string, string>; label: string; mood: string; confused: boolean; x: number; y: number; s: number };
export type Frame = { pets: Placed[]; liveliness: Liveliness; reduceMotion: boolean };
export type Hover = { id: string | null; held: boolean };
export type Feedback = { id: string; ok: boolean; text: string };

const log = (msg: string) => void invoke("pet_log", { msg });
addEventListener("error", (e) => log(`ERROR ${e.message}`));
addEventListener("unhandledrejection", (e) => log(`REJECT ${String(e.reason)}`));
log(`BOOT ${location.href}`);

// Bundled locally: the runtime must never fetch its WASM from a CDN.
RuntimeLoader.setWasmUrl(riveWasmUrl);

/** What draws one pet. `continuous` rigs animate between moments too (their own frame rate keeps CPU in check). */
type Rig = {
  play(): void;
  pause(): void;
  number(name: string, value: number): void;
  resize(s: number): void;
  cleanup(): void;
  continuous?: boolean;
  setLiveliness?(level: Liveliness, still: boolean): void;
};
type Pet = { el: HTMLCanvasElement; rig: Rig; placed: Placed; timer?: number; playing: boolean; busy: boolean; leaving: boolean; run: number };
type Cue = "eat" | "fallAsleep" | "wake" | "postcard" | "hello" | { celebrate: { leave: boolean } } | { graduate: { accessory: number } };

// The rig's `mood` and `action` view-model numbers (art/STYLE.md).
const MOOD: Record<string, number> = { joyful: 1, content: 2, peckish: 3, hungry: 4, gloomy: 5 };
const ACT = { eat: 1, petted: 2, celebrate: 3, exit: 4, enter: 5, fallAsleep: 6, sleep: 7, wake: 8, postcard: 9 };
/** Seconds each clip takes (frames / 60 in art/cat/make.py, plus the blend). */
// celebrate is the longest species clip (the parrot, 330 frames) plus the blend
const CLIP_S: Record<number, number> = { 1: 4.2, 2: 2.7, 3: 5.8, 4: 3.2, 5: 3.2, 6: 3.2, 8: 3.2, 9: 4.2 };
const ASLEEP_BEFORE_WAKE_S = 60;
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
  if (on) p.rig.play();
  else p.rig.pause();
}

function setNumber(p: Pet, name: string, value: number) {
  p.rig.number(name, value);
}

const wait = (s: number) => new Promise((r) => setTimeout(r, s * 1000));

/** Plays clips in order (`[action, seconds]`), keeping the renderer running until they finish.
 *  A newer call takes over: the older one stops without touching the pet. */
async function play(p: Pet, steps: [number, number][]) {
  window.clearTimeout(p.timer);
  const run = ++p.run;
  p.busy = true;
  setPlaying(p, true);
  for (const [action, secs] of steps) {
    setNumber(p, "action", action);
    await wait(secs);
    if (run !== p.run) return;
  }
  setNumber(p, "action", 0);
  await wait(0.9); // let the pose blend (≤ 400 ms in the rig) finish before rendering stops
  if (run !== p.run || !pets.has(p.placed.id)) return;
  p.busy = false;
  if (p.leaving) fadeOut(p);
  else schedule(p);
}

function fadeOut(p: Pet) {
  p.el.style.transition = "opacity 800ms ease-out";
  p.el.style.opacity = "0";
  setTimeout(() => remove(p.placed.id), 850);
}

/** A cue from the core (§5.5, §5.6, §7.4). Reduce Motion skips the clips; leaving pets just fade. */
function cue(p: Pet, c: Cue) {
  const kind = typeof c === "string" ? c : Object.keys(c)[0];
  const leave = typeof c === "object" && "celebrate" in c && c.celebrate.leave;
  if (kind === "fallAsleep" || kind === "graduate" || leave) p.leaving = true;
  // a graduate wears its accessory for the farewell (§5.8), even with Reduce Motion
  if (typeof c === "object" && "graduate" in c) setNumber(p, "accessory", c.graduate.accessory);
  if (reduceMotion) {
    if (p.leaving) fadeOut(p);
    return;
  }
  switch (kind) {
    case "eat":
      return void play(p, [[ACT.eat, CLIP_S[ACT.eat]]]);
    case "celebrate":
      return void play(p, leave ? [[ACT.celebrate, CLIP_S[ACT.celebrate]], [ACT.exit, CLIP_S[ACT.exit]]] : [[ACT.celebrate, CLIP_S[ACT.celebrate]]]);
    case "graduate":
      return void play(p, [[ACT.celebrate, CLIP_S[ACT.celebrate]], [ACT.exit, CLIP_S[ACT.exit]]]);
    case "fallAsleep":
      return void play(p, [[ACT.fallAsleep, CLIP_S[ACT.fallAsleep]]]);
    case "wake":
      return void play(p, [[ACT.sleep, ASLEEP_BEFORE_WAKE_S], [ACT.wake, CLIP_S[ACT.wake]]]);
    case "postcard":
      return void play(p, [[ACT.postcard, CLIP_S[ACT.postcard]]]);
    case "hello":
      return void play(p, [[ACT.enter, CLIP_S[ACT.enter]]]);
  }
}

/** Schedules this pet's next moment, or keeps it moving in Lively. Rendering stops between moments. */
function schedule(p: Pet) {
  if (p.busy) return;
  window.clearTimeout(p.timer);
  p.rig.setLiveliness?.(mode, reduceMotion);
  if (frozen) return setPlaying(p, false);
  const level = reduceMotion ? "calm" : mode;
  if (level === "lively" || (p.rig.continuous && !reduceMotion)) return setPlaying(p, true);
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
  if (p.el.width !== Math.round(s * devicePixelRatio)) p.rig.resize(s);
}

function canvasFor(id: string) {
  const el = document.createElement("canvas");
  el.dataset.id = id;
  document.body.appendChild(el);
  return el;
}

function riveRig(p: Pet): Rig {
  const { placed, el } = p;
  el.width = el.height = Math.round(placed.s * devicePixelRatio);
  const rive: Rive = new Rive({
    src: artFor(placed.species),
    canvas: el,
    autoplay: true,
    stateMachine: "Idle",
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
      setNumber(p, "mood", MOOD[placed.mood] ?? 2);
      log(`LOADED pet=${placed.id} colours=${bound}/${Object.keys(placed.colors).length}`);
      // render until the mood pose has blended in, then hold still until a moment
      void play(p, []);
    },
  });
  return {
    play: () => rive.play(),
    pause: () => rive.pause(),
    number: (name, value) => {
      const prop = rive.viewModelInstance?.number(name);
      if (prop) prop.value = value;
    },
    resize: (s) => {
      el.width = el.height = Math.round(s * devicePixelRatio);
      rive.resizeDrawingSurfaceToCanvas();
    },
    cleanup: () => rive.cleanup(),
  };
}

/** The 3D Persian, or null (logged) when WebGL isn't available; if the GPU drops it later, the pet
 *  is rebuilt with its Rive rig. */
function cat3dRig(p: Pet): Rig | null {
  const { placed } = p;
  try {
    const rig = new Cat3DRig(p.el, placed.colors, log, () => {
      log(`WARN pet=${placed.id} lost its 3D drawing; switching to the drawn cat`);
      const current = pets.get(placed.id);
      if (current?.rig !== cat) return;
      remove(placed.id);
      add(current.placed, true);
    });
    const cat: Rig = Object.assign(rig, { continuous: true });
    rig.number("mood", MOOD[placed.mood] ?? 2);
    rig.setLiveliness(mode, reduceMotion);
    log(`LOADED pet=${placed.id} colours=${Object.keys(placed.colors).length}/${Object.keys(placed.colors).length} rig=3d`);
    return cat;
  } catch (e) {
    log(`ERROR pet=${placed.id} can't draw the 3D cat (${String(e)}); using the drawn cat`);
    return null;
  }
}

function add(placed: Placed, forceRive = false) {
  const p = { el: canvasFor(placed.id), placed, playing: false, busy: false, leaving: false, run: 0 } as Pet;
  let rig = placed.species === "cat" && !forceRive ? cat3dRig(p) : null;
  if (!rig) {
    // a canvas that tried WebGL can't give Rive a 2D context, so start from a fresh one
    if (placed.species === "cat" && !forceRive) {
      p.el.remove();
      p.el = canvasFor(placed.id);
    }
    rig = riveRig(p);
    p.playing = true;
  }
  p.rig = rig;
  pets.set(placed.id, p);
  position(p);
  if (rig.continuous) void play(p, []);
  // the window only takes clicks while the user's chosen modifier is held (Rust decides, and says so
  // through "hover"), so any click that arrives here is meant for the pet, whichever modifier that is
  p.el.addEventListener("mousedown", (e) => {
    if (!frozen) return;
    e.preventDefault();
    void invoke("pet_click", { id: placed.id, shift: e.shiftKey });
  });
  p.el.addEventListener("contextmenu", (e) => e.preventDefault());
}

function remove(id: string) {
  const p = pets.get(id);
  if (!p) return;
  window.clearTimeout(p.timer);
  p.rig.cleanup();
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
  // a leaving pet stays until its clip ends (play() removes it)
  [...pets.entries()].filter(([id, p]) => !keep.has(id) && !p.leaving).forEach(([id]) => remove(id));
  for (const placed of payload.pets) {
    const p = pets.get(placed.id);
    if (!p) add(placed);
    else {
      const moodChanged = p.placed.mood !== placed.mood;
      p.placed = placed;
      position(p);
      if (moodChanged) {
        // play briefly so the pose blends in, then hold it
        setNumber(p, "mood", MOOD[placed.mood] ?? 2);
        if (!p.busy) void play(p, []);
      } else if (modeChanged) schedule(p);
    }
  }
  // GitHub can't find its repo: the question stays over the pet until the user answers it (card or menu)
  document.querySelectorAll(".ask").forEach((b) => b.remove());
  for (const placed of payload.pets) if (placed.confused) bubble(placed.id, "❓ Where did my repo go?", 0, "ask");
}));

// ⌥ held: every pet freezes, and the one under the cursor gets an outline and a tooltip (§7.2, §7.3).
ready.push(listen<Hover>("hover", ({ payload }) => {
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

ready.push(listen<Feedback>("feedback", ({ payload }) => bubble(payload.id, payload.text, 2500)));
ready.push(listen<{ id: string; cue: Cue }>("cue", ({ payload }) => {
  const p = pets.get(payload.id);
  if (p) cue(p, payload.cue);
}));
// a nudge stays a little longer and needs no click (§5.10)
ready.push(listen<{ id: string; text: string }>("nudge", ({ payload }) => bubble(payload.id, payload.text, 8000)));

// Ask for the first frame only once every listener is registered, or it can arrive before them.
void Promise.all(ready).then(() => invoke("overlay_ready"));
