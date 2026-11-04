// How often a 3D pet is drawn (review-spec A2): in step with the display, at a rate set by the
// liveliness and by what the pet is doing. Kept apart from the drawing so it can be tested.

export type Liveliness = "calm" | "gentle" | "lively";

/** [frames per second while something quick moves or an action runs, for breathing and slow sway] */
export const FPS: Record<Liveliness, [moving: number, idle: number]> = { calm: [30, 20], gentle: [60, 30], lively: [60, 60] };
/** a sleeping pet only breathes */
export const ASLEEP_FPS = 10;
/** without a GPU everything is drawn in software, so it gets very few frames */
export const SOFTWARE_FPS: [number, number] = [8, 4];

export function rateFor(level: Liveliness, s: { fast: boolean; acting: boolean; asleep: boolean; software: boolean }, slow = 1): number {
  const [moving, idle] = s.software ? SOFTWARE_FPS : FPS[level];
  const rate = s.fast || s.acting ? moving : s.asleep ? Math.min(ASLEEP_FPS, idle) : idle;
  return rate / slow;
}

/** A display frame early by up to this many ms still counts, so a 30 fps pet takes every 2nd frame
 *  of a 60 Hz display (every 4th at 120 Hz) instead of drifting between them. */
const EARLY_MS = 4;

/** Whether this display frame is due to be drawn. */
export const due = (now: number, last: number, rate: number) => now - last >= 1000 / rate - EARLY_MS;

/** How long to sleep before asking for display frames again: the loop doesn't wake for every
 *  display frame at low rates, only for the last couple before the next one is due. */
export const idleFor = (rate: number) => Math.max(0, 1000 / rate - 24);

/** What to give up, in order, when frames cost too much: coat layers, then sharpness, then frames. */
export type Quality = { shells: number; dpr: number; slow: number };
export function cheaper(q: Quality, minShells: number): Quality | null {
  if (q.shells > minShells) return { ...q, shells: Math.max(minShells, Math.floor(q.shells / 2)) };
  if (q.dpr > 1) return { ...q, dpr: 1 };
  if (q.slow < 4) return { ...q, slow: q.slow * 2 };
  return null;
}
