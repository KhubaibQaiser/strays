// Pet overlay: draws the pets the Rust core places, and plays their moments at the chosen liveliness.
// The core owns all state; this page only renders and reports ⌥-clicks. Species with a 3D model
// (src/pet3d) draw in 3D; the rest, and any pet whose WebGL fails, are drawn by their Rive rigs.
import { setFps } from "./throttle";
import { Rive, RuntimeLoader } from "@rive-app/canvas";
import riveWasmUrl from "@rive-app/canvas/rive.wasm?url";
import { artFor } from "./art";
import { Pet3DRig, SPECIES_3D } from "./pet3d/rig";
import { meshBuilt, meshReady } from "./pet3d/meshes";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type Liveliness = "calm" | "gentle" | "lively";
export type Placed = { id: string; species: string; colors: Record<string, string>; label: string; mood: string; confused: boolean; liveliness: Liveliness | null; x: number; y: number; s: number };
/** The screen grid pets stand on (Rust's `overlay::Grid`): cell (0, 0) is bottom right, columns go left, rows up. */
export type Grid = { x0: number; y0: number; cw: number; ch: number; cols: number; rows: number; s: number };
/** `hidden`: a full-screen app is in front, so the pets are kept but not shown or drawn. */
export type Frame = { pets: Placed[]; grid: Grid; liveliness: Liveliness; reduceMotion: boolean; hidden: boolean };
export type Hover = { id: string | null; held: boolean };
export type Feedback = { id: string; ok: boolean; text: string };

/** Debug-run traces (the OS test harness reads them); release builds drop them. */
const log = (msg: string) => void invoke("pet_log", { msg });
/** A problem the user should know about: logged and listed in the menu and Settings (a repeat of the
 *  same `key` replaces the earlier one). */
const problem = (key: string, text: string) => {
  log(`WARN ${key}: ${text}`);
  invoke("report_problem", { key, text }).catch((e) => log(`ERROR couldn't report ${key}: ${e}`));
};
/** For a call to Rust's `.catch`: a failure is reported, never dropped. */
const unreached = (cmd: string) => (e: unknown) => problem(cmd, `The pets' window couldn't reach Strays (${cmd}): ${e}`);
const nameOf = (p: Placed) => p.label.split(" · ")[0];
addEventListener("error", (e) => problem("script", `Something went wrong in the pets' window: ${e.message}`));
addEventListener("unhandledrejection", (e) => problem("script", `Something went wrong in the pets' window: ${String(e.reason)}`));
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
type Pet = { el: HTMLCanvasElement; rig: Rig; placed: Placed; timer?: number; playing: boolean; busy: boolean; leaving: boolean; run: number; big?: number };
type Cue = "eat" | "fallAsleep" | "wake" | "postcard" | "hello" | { celebrate: { leave: boolean } } | { graduate: { accessory: number } };

// The rig's `mood` and `action` view-model numbers (art/STYLE.md).
const MOOD: Record<string, number> = { joyful: 1, content: 2, peckish: 3, hungry: 4, gloomy: 5 };
const ACT = { eat: 1, petted: 2, celebrate: 3, exit: 4, enter: 5, fallAsleep: 6, sleep: 7, wake: 8, postcard: 9 };
/** Seconds each clip takes (frames / 60 in art/cat/make.py, plus the blend). */
// celebrate is the longest species clip (the parrot, 330 frames) plus the blend
const CLIP_S: Record<number, number> = { 1: 4.2, 2: 2.7, 3: 5.8, 4: 3.2, 5: 3.2, 6: 3.2, 8: 3.2, 9: 4.2 };
const ASLEEP_BEFORE_WAKE_S = 60;
const pets = new Map<string, Pet>();
let mode: Liveliness = "gentle";
/** A pet's own liveliness, or the app's. */
const levelOf = (p: { placed: Placed }) => p.placed.liveliness ?? mode;
let reduceMotion = false;
let frozen = false;
let hidden = false;
/** WebKit keeps only so many WebGL contexts and drops the oldest past that; pets beyond this draw in 2D. */
const MAX_3D = 12;
let grid: Grid = { x0: 0, y0: 0, cw: 0, ch: 0, cols: 0, rows: 0, s: 0 };
/** A press on a pet: it becomes a drag once the cursor moves this many points (§7.2). */
const DRAG_PT = 4;
let press: { p: Pet; x: number; y: number; shift: boolean; dragging: boolean; petted: boolean; timer: number } | null = null;
/** Holding still on a pet this long pets it (§7.2): a happy reaction that never changes mood. */
const PET_MS = 1000;

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
  p.rig.setLiveliness?.(levelOf(p), reduceMotion);
  if (frozen || hidden) return setPlaying(p, false);
  const level = reduceMotion ? "calm" : levelOf(p);
  if (level === "lively" || (p.rig.continuous && !reduceMotion)) return setPlaying(p, true);
  setPlaying(p, false);
  const [lo, hi] = GAP[level];
  p.timer = window.setTimeout(() => {
    setPlaying(p, true);
    p.timer = window.setTimeout(() => schedule(p), MOMENT_MS);
  }, (lo + Math.random() * (hi - lo)) * 1000);
}

/** Big moves (§7.4): every 2–5 hours a pet stretches or strolls a little way and back, under 8 s.
 *  Never with Reduce Motion, while the modifier is held, or while it's hidden or busy. */
const BIG_EVERY_H: [number, number] = [2, 5];
function scheduleBig(p: Pet) {
  window.clearTimeout(p.big);
  const [lo, hi] = BIG_EVERY_H;
  p.big = window.setTimeout(() => bigMove(p), (lo + Math.random() * (hi - lo)) * 3_600_000);
}
function bigMove(p: Pet) {
  if (!pets.has(p.placed.id)) return;
  // not now: try again in ten minutes
  if (reduceMotion || frozen || hidden || p.busy || p.leaving || press) {
    p.big = window.setTimeout(() => bigMove(p), 600_000);
    return;
  }
  scheduleBig(p);
  if (Math.random() < 0.5) return void play(p, [[ACT.wake, CLIP_S[ACT.wake]]]); // a stretch and a yawn
  // a stroll: a little way towards the middle of the screen and back, about 6 s
  const dir = p.placed.x + p.placed.s / 2 > innerWidth / 2 ? -1 : 1;
  const away = `${p.placed.x + dir * p.placed.s * 0.6}px`;
  p.el.style.transition = "left 2.6s ease-in-out";
  p.el.style.left = away;
  window.setTimeout(() => {
    if (p.el.style.left === away) p.el.style.left = `${p.placed.x}px`;
    window.setTimeout(() => (p.el.style.transition = ""), 2700);
  }, 3000);
}

function position(p: Pet) {
  if (press?.dragging && press.p === p) return;
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
    onLoadError: (e) => {
      log(`LOADERROR pet=${placed.id} ${JSON.stringify(e.data)}`);
      problem(`draw-${placed.id}`, `${nameOf(placed)} couldn't be drawn: ${JSON.stringify(e.data)}`);
    },
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

/** The 3D pet, or null (logged) when WebGL isn't available; if the GPU drops it later, the pet is
 *  rebuilt with its Rive rig. */
function rig3d(p: Pet): Rig | null {
  const { placed } = p;
  try {
    const rig = new Pet3DRig(p.el, SPECIES_3D[placed.species], placed.colors, problem, () => {
      problem(`3d-${placed.id}`, `The graphics card dropped ${nameOf(placed)}'s 3D drawing, so it's drawn in 2D now`);
      const current = pets.get(placed.id);
      if (current?.rig !== pet3d) return;
      remove(placed.id);
      add(current.placed, true);
    });
    const pet3d: Rig = Object.assign(rig, { continuous: true });
    rig.number("mood", MOOD[placed.mood] ?? 2);
    rig.setLiveliness(levelOf(p), reduceMotion);
    log(`LOADED pet=${placed.id} colours=${Object.keys(placed.colors).length}/${Object.keys(placed.colors).length} rig=3d`);
    return pet3d;
  } catch (e) {
    problem(`3d-${placed.id}`, `${nameOf(placed)} can't be drawn in 3D (${String(e)}), so it's drawn in 2D`);
    return null;
  }
}

function add(placed: Placed, forceRive = false) {
  const p = { el: canvasFor(placed.id), placed, playing: false, busy: false, leaving: false, run: 0 } as Pet;
  const in3d = [...pets.values()].filter((o) => o.rig.continuous).length;
  if (placed.species in SPECIES_3D && !forceRive && in3d >= MAX_3D) problem("3d-limit", `Only ${MAX_3D} pets can be drawn in 3D at once; the rest are drawn in 2D`);
  const want3d = placed.species in SPECIES_3D && !forceRive && in3d < MAX_3D;
  let rig = want3d ? rig3d(p) : null;
  if (!rig) {
    // a canvas that tried WebGL can't give Rive a 2D context, so start from a fresh one
    if (want3d) {
      p.el.remove();
      p.el = canvasFor(placed.id);
    }
    rig = riveRig(p);
    p.playing = true;
  }
  p.rig = rig;
  pets.set(placed.id, p);
  scheduleBig(p);
  position(p);
  if (rig.continuous) void play(p, []);
  // the window only takes clicks while the user's chosen modifier is held (Rust decides, and says so
  // through "hover"), so any press that arrives here is meant for the pet, whichever modifier that is.
  // Let go where it was pressed and it's a click; hold still a second and it's petting; move it and
  // it's a drag onto the grid.
  p.el.addEventListener("mousedown", (e) => {
    if (!frozen || e.button !== 0 || press) return;
    e.preventDefault();
    const timer = window.setTimeout(() => {
      if (!press || press.p !== p || press.dragging) return;
      press.petted = true;
      if (!reduceMotion && !p.busy) void play(p, [[ACT.petted, CLIP_S[ACT.petted]]]);
      else bubble(p.placed.id, "♥", 1200);
    }, PET_MS);
    press = { p, x: e.clientX, y: e.clientY, shift: e.shiftKey, dragging: false, petted: false, timer };
    invoke("pet_press", { id: p.placed.id, down: true }).catch(unreached("pet_press"));
  });
  // the modifier and a right-click: the pet's own menu, where the cursor is (§7.2)
  p.el.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    if (frozen) invoke("pet_menu", { id: p.placed.id }).catch(unreached("pet_menu"));
  });
}

/** The pets the latest frame lists, for pets that arrive once their shape is built. */
let latest = new Map<string, Placed>();

/** Adds a pet now if its 3D shape is built, else once a worker has built it (if it's still wanted). */
function addWhenReady(placed: Placed) {
  if (meshBuilt(placed.species)) return add(placed);
  void meshReady(placed.species, problem).then(() => {
    const now = latest.get(placed.id);
    if (!now || pets.has(now.id)) return;
    add(now);
    if (now.confused) bubble(now.id, "❓ Where did my repo go?", 0, "ask");
  });
}

/** Grid cell nearest a canvas top-left (the same rule as Rust's `Grid::cell`). */
function cellAt(x: number, y: number): [number, number] {
  const pick = (d: number, step: number, n: number) => (step > 0 ? Math.min(n - 1, Math.max(0, Math.round(d / step))) : 0);
  return [pick(grid.x0 - x, grid.cw, grid.cols), pick(grid.y0 - y, grid.ch, grid.rows)];
}

/** While a pet is dragged, a faint grid shows where it can stand: taken cells dimmer, the target lit. */
function drawGrid(dragged: Pet) {
  let layer = document.getElementById("grid");
  if (!layer) {
    layer = document.createElement("div");
    layer.id = "grid";
    if (reduceMotion) layer.classList.add("still");
    document.body.prepend(layer);
    for (let r = 0; r < grid.rows; r++)
      for (let c = 0; c < grid.cols; c++) {
        const cell = document.createElement("div");
        cell.className = "cell";
        cell.dataset.cell = `${c},${r}`;
        // the cell marks where the pet's body stands: the lower middle of its canvas
        Object.assign(cell.style, {
          left: `${grid.x0 - c * grid.cw + grid.s * 0.15}px`,
          top: `${grid.y0 - r * grid.ch + grid.s * 0.3}px`,
          width: `${grid.s * 0.7}px`,
          height: `${grid.s * 0.7}px`,
        });
        layer.appendChild(cell);
      }
    requestAnimationFrame(() => layer?.classList.add("shown"));
  }
  const taken = new Set([...pets.values()].filter((o) => o !== dragged).map((o) => cellAt(o.placed.x, o.placed.y).join(",")));
  const target = cellAt(parseFloat(dragged.el.style.left), parseFloat(dragged.el.style.top)).join(",");
  for (const cell of layer.children as HTMLCollectionOf<HTMLElement>) {
    cell.classList.toggle("taken", taken.has(cell.dataset.cell!));
    cell.classList.toggle("target", cell.dataset.cell === target);
  }
}

function hideGrid() {
  document.getElementById("grid")?.remove();
}

/** Ends a press: a click if it never moved, else a drop that Rust snaps to the nearest free cell. */
function endPress() {
  if (!press) return;
  const { p, shift, dragging, petted, timer } = press;
  window.clearTimeout(timer);
  press = null;
  hideGrid();
  p.el.classList.remove("dragging");
  if (!dragging) {
    invoke("pet_press", { id: p.placed.id, down: false }).catch(unreached("pet_press"));
    // a pat isn't a click
    if (!petted) invoke("pet_click", { id: p.placed.id, shift }).catch(unreached("pet_click"));
  } else {
    // the pet stays where it was let go until the next frame puts it in its cell
    invoke("pet_drop", { id: p.placed.id, x: parseFloat(p.el.style.left), y: parseFloat(p.el.style.top) }).catch(unreached("pet_drop"));
  }
}

addEventListener("mousemove", (e) => {
  if (!press) return;
  const dx = e.clientX - press.x, dy = e.clientY - press.y;
  if (!press.dragging && Math.hypot(dx, dy) < DRAG_PT) return;
  const p = press.p;
  if (!press.dragging) {
    press.dragging = true;
    p.el.classList.add("dragging");
    document.querySelectorAll(".tooltip, .bubble, .ask").forEach((b) => b.remove());
  }
  p.el.style.left = `${p.placed.x + dx}px`;
  p.el.style.top = `${p.placed.y + dy}px`;
  drawGrid(p);
});
addEventListener("mouseup", (e) => e.button === 0 && endPress());

function remove(id: string) {
  const p = pets.get(id);
  if (!p) return;
  window.clearTimeout(p.timer);
  window.clearTimeout(p.big);
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
// Rust saw the button let go without the page hearing it: drop the pet where it is
ready.push(listen("drag-end", endPress));
ready.push(listen<Frame>("overlay", ({ payload }) => {
  const modeChanged = payload.liveliness !== mode || payload.reduceMotion !== reduceMotion || payload.hidden !== hidden;
  hidden = payload.hidden;
  document.body.classList.toggle("hidden", hidden);
  mode = payload.liveliness;
  reduceMotion = payload.reduceMotion;
  // ponytail: the Rive cap is one for the page, so a Rive (fallback) pet follows the app's liveliness
  grid = payload.grid;
  setFps(mode === "lively" && !reduceMotion ? 15 : 24);
  const keep = new Set(payload.pets.map((p) => p.id));
  // a leaving pet stays until its clip ends (play() removes it)
  [...pets.entries()].filter(([id, p]) => !keep.has(id) && !p.leaving).forEach(([id]) => remove(id));
  latest = new Map(payload.pets.map((p) => [p.id, p]));
  for (const placed of payload.pets) {
    const p = pets.get(placed.id);
    if (!p) addWhenReady(placed);
    else {
      const moodChanged = p.placed.mood !== placed.mood;
      const ownChanged = p.placed.liveliness !== placed.liveliness;
      p.placed = placed;
      position(p);
      if (moodChanged) {
        // play briefly so the pose blends in, then hold it
        setNumber(p, "mood", MOOD[placed.mood] ?? 2);
        if (!p.busy) void play(p, []);
      } else if (modeChanged || ownChanged) schedule(p);
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
    // a pet mid-stroll goes straight back to where the clicks expect it
    if (frozen)
      pets.forEach((p) => {
        if (!p.el.style.transition.startsWith("left")) return;
        p.el.style.transition = "";
        position(p);
      });
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

// Ask for the first frame only once every listener is registered, or it can arrive before them. If one
// can't be registered, say so and ask anyway: pets that miss some events beat no pets at all.
void Promise.all(ready)
  .catch((e) => problem("listen", `The pets' window can't hear some of what Strays sends it: ${e}`))
  .finally(() => invoke("overlay_ready").catch(unreached("overlay_ready")));
