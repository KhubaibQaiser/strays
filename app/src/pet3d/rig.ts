// The 3D Persian behind the same small interface the overlay uses for Rive pets: play/pause, the
// rig's view-model numbers (mood, action, accessory), resize and cleanup. It runs its own frame loop.
import { Action, Cat3D, type Look, type Mood } from "./cat";

const MOODS: Mood[] = ["content", "joyful", "content", "peckish", "hungry", "gloomy"]; // index = the rig's mood number
type Liveliness = "calm" | "gentle" | "lively";
/** [frame rate while something quick moves or an action runs, frame rate for breathing and slow sway] */
const FPS: Record<Liveliness, [number, number]> = { calm: [30, 15], gentle: [30, 24], lively: [60, 30] };

/** The app's four coat colours become the Persian's look; eyes and nose are fixed. */
export function lookFor(colors: Record<string, string>): Look {
  return {
    base: colors.coatBase ?? "#ecd3ad",
    shade: colors.coatShade ?? "#c49a68",
    light: colors.coatHighlight ?? "#f8eedd",
    pink: "#d97a80",
    iris: "#c8701f",
    stripes: 0,
    tipping: 0.3,
  };
}

export class Cat3DRig {
  private cat: Cat3D;
  private raf = 0;
  private last = 0;
  private fps: [number, number] = FPS.calm;
  private warnedAccessory = false;

  /** Throws if WebGL isn't available. `onLost` runs if the GPU drops the context later. */
  constructor(
    private canvas: HTMLCanvasElement,
    colors: Record<string, string>,
    private log: (msg: string) => void,
    onLost: () => void,
  ) {
    this.cat = new Cat3D(canvas, lookFor(colors), { shells: 28, furLen: 0.07, density: 150 });
    canvas.addEventListener("webglcontextlost", (e) => {
      e.preventDefault();
      this.pause();
      onLost();
    });
  }

  /** How it moves: `still` (Reduce Motion) keeps only blinks and breathing. */
  setLiveliness(level: Liveliness, still: boolean) {
    this.fps = FPS[level];
    this.cat.still = still;
  }

  play() {
    if (this.raf) return;
    this.last = performance.now();
    this.raf = requestAnimationFrame(this.tick);
  }

  /** Holds the current pose; nothing is drawn until `play`. */
  pause() {
    cancelAnimationFrame(this.raf);
    this.raf = 0;
  }

  private tick = (now: number) => {
    this.raf = requestAnimationFrame(this.tick);
    const rate = this.cat.fast || this.cat.acting ? this.fps[0] : this.fps[1];
    if (now - this.last < 1000 / rate - 2) return;
    this.cat.update((now - this.last) / 1000);
    this.cat.render();
    this.last = now;
  };

  number(name: string, value: number) {
    if (name === "mood") this.cat.mood = MOODS[value] ?? "content";
    else if (name === "action") this.cat.act(value as Action);
    else if (name === "accessory" && value && !this.warnedAccessory) {
      this.warnedAccessory = true;
      this.log("WARN the 3D cat doesn't wear accessories yet");
    }
  }

  resize(s: number) {
    this.cat.setSize(s, devicePixelRatio);
    // draw once at the new size even while paused, so the pet never shows a blank canvas
    this.cat.update(0);
    this.cat.render();
  }

  cleanup() {
    this.pause();
    this.cat.dispose();
    this.canvas.remove();
  }
}
