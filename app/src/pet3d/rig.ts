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
  private software = false;
  /** smoothed milliseconds per drawn frame, and how far the frame rate has been cut because of it */
  private cost = 0;
  private slow = 1;

  /** Throws if WebGL isn't available. `onLost` runs if the GPU drops the context later. */
  constructor(
    private canvas: HTMLCanvasElement,
    colors: Record<string, string>,
    private log: (msg: string) => void,
    onLost: () => void,
  ) {
    this.cat = new Cat3D(canvas, lookFor(colors), { shells: 28, furLen: 0.07, density: 150 });
    // without a GPU (software WebGL, e.g. in a VM) full fur would starve the whole page
    const gl = this.cat.renderer.getContext();
    const info = gl.getExtension("WEBGL_debug_renderer_info");
    const gpu = String(info ? gl.getParameter(info.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER));
    if (/swiftshader|llvmpipe|softpipe|software/i.test(gpu)) {
      this.cat.setShells(6);
      this.software = true;
      log(`WARN no GPU for the 3D cat (${gpu}); drawing it with less fur and fewer frames`);
    }
    canvas.addEventListener("webglcontextlost", (e) => {
      e.preventDefault();
      this.pause();
      onLost();
    });
  }

  /** How it moves: `still` (Reduce Motion) keeps only blinks and breathing. */
  setLiveliness(level: Liveliness, still: boolean) {
    this.fps = this.software ? [8, 4] : FPS[level];
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
    const rate = (this.cat.fast || this.cat.acting ? this.fps[0] : this.fps[1]) / this.slow;
    if (now - this.last < 1000 / rate - 2) return;
    const start = performance.now();
    this.cat.update((now - this.last) / 1000);
    this.cat.render();
    this.last = now;
    // frames that cost more than a fifth of their slot get fewer of them, so a slow machine stays usable
    this.cost = this.cost * 0.9 + (performance.now() - start) * 0.1;
    if (this.cost > 200 / rate && this.slow < 8) {
      this.slow *= 2;
      this.cost = 0;
      this.log(`WARN the 3D cat takes ${(200 / rate).toFixed(0)}+ ms a frame; drawing it ${this.slow}× less often`);
    }
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
