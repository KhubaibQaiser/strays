// A 3D pet behind the same small interface the overlay uses for Rive pets: play/pause, the rig's
// view-model numbers (mood, action, accessory), resize and cleanup. It runs its own frame loop, in step
// with the display, at a rate set by the liveliness and what the pet is doing (frames.ts).
import { Action, Pet3D, type Mood } from "./pet";
import type { SpeciesDef } from "./species";
import { cat } from "./cat";
import { dog } from "./dog";
import { hamster } from "./hamster";
import { macaw } from "./macaw";
import { cheaper, due, idleFor, rateFor, type Liveliness, type Quality } from "./frames";
import { nativeCancel, nativeRaf } from "../throttle";

/** The species drawn in 3D; the others keep their Rive rigs. */
export const SPECIES_3D: Record<string, SpeciesDef> = { cat, dog, hamster, parrot: macaw };

const MOODS: Mood[] = ["content", "joyful", "content", "peckish", "hungry", "gloomy"]; // index = the rig's mood number
/** the fewest coat layers the slow-frame guard goes down to */
const MIN_SHELLS = 6;

export class Pet3DRig {
  private pet: Pet3D;
  private raf = 0;
  private sleep = 0;
  private last = 0;
  private level: Liveliness = "gentle";
  private warnedAccessory = false;
  private software = false;
  /** smoothed milliseconds per drawn frame, and what's been given up because of it */
  private cost = 0;
  private quality: Quality;
  private size = 0;
  private disposed = false;

  /** Throws if WebGL isn't available. `onLost` runs if the GPU drops the context later. */
  constructor(
    private canvas: HTMLCanvasElement,
    species: SpeciesDef,
    colors: Record<string, string>,
    private log: (msg: string) => void,
    onLost: () => void,
  ) {
    this.pet = new Pet3D(canvas, species, species.palette(colors));
    this.software = this.pet.software !== null;
    this.quality = { shells: this.pet.shells, dpr: devicePixelRatio, slow: 1 };
    if (this.pet.software) log(`WARN no GPU for the 3D pet (${this.pet.software}); drawing it with less coat, light and frames`);
    canvas.addEventListener("webglcontextlost", (e) => {
      if (this.disposed) return; // cleanup gives the context back on purpose
      e.preventDefault();
      this.pause();
      onLost();
    });
  }

  /** How it moves: `still` (Reduce Motion) keeps only blinks and breathing. */
  setLiveliness(level: Liveliness, still: boolean) {
    this.level = level;
    this.pet.still = still;
  }

  play() {
    if (this.raf || this.sleep) return;
    this.last = performance.now();
    this.raf = nativeRaf(this.tick);
  }

  /** Holds the current pose; nothing is drawn until `play`. */
  pause() {
    nativeCancel(this.raf);
    window.clearTimeout(this.sleep);
    this.raf = this.sleep = 0;
  }

  private rate() {
    const p = this.pet;
    return rateFor(this.level, { fast: p.fast, acting: p.acting, asleep: p.asleep, software: this.software }, this.quality.slow);
  }

  private tick = (now: number) => {
    this.raf = 0;
    const rate = this.rate();
    if (!due(now, this.last, rate)) {
      this.raf = nativeRaf(this.tick);
      return;
    }
    const start = performance.now();
    this.pet.update((now - this.last) / 1000);
    this.pet.render();
    this.last = now;
    this.guard(performance.now() - start, rate);
    // far-apart frames: sleep until just before the next one instead of waking every display frame
    const wait = idleFor(this.rate());
    if (wait > 0)
      this.sleep = window.setTimeout(() => {
        this.sleep = 0;
        this.raf = nativeRaf(this.tick);
      }, wait);
    else this.raf = nativeRaf(this.tick);
  };

  /** Frames that cost more than a fifth of their slot give up coat layers, then sharpness, then frames,
   *  so a slow machine stays usable and the motion stays even for as long as possible. */
  private guard(ms: number, rate: number) {
    this.cost = this.cost * 0.9 + ms * 0.1;
    if (this.cost <= 200 / rate) return;
    const next = cheaper(this.quality, Math.min(MIN_SHELLS, this.pet.shells));
    this.cost = 0;
    if (!next) return;
    const what = next.shells !== this.quality.shells ? `${next.shells} coat layers` : next.dpr !== this.quality.dpr ? "standard sharpness" : `${next.slow}× fewer frames`;
    this.quality = next;
    if (next.shells !== this.pet.shells) this.pet.setShells(next.shells);
    if (this.size) this.pet.setSize(this.size, Math.min(devicePixelRatio, next.dpr));
    this.log(`WARN the 3D pet takes ${(200 / rate).toFixed(0)}+ ms a frame; drawing it with ${what}`);
  }

  number(name: string, value: number) {
    if (name === "mood") this.pet.mood = MOODS[value] ?? "content";
    else if (name === "action") this.pet.act(value as Action);
    else if (name === "accessory" && value && !this.warnedAccessory) {
      this.warnedAccessory = true;
      this.log("WARN 3D pets don't wear accessories yet");
    }
  }

  resize(s: number) {
    // the overlay asks on every frame message; its canvas check can't see the sharpness given up above
    if (s === this.size) return;
    this.size = s;
    this.pet.setSize(s, Math.min(devicePixelRatio, this.quality.dpr));
    // draw once at the new size even while paused, so the pet never shows a blank canvas
    this.pet.update(0);
    this.pet.render();
  }

  cleanup() {
    this.disposed = true;
    this.pause();
    this.pet.dispose();
    this.canvas.remove();
  }
}
