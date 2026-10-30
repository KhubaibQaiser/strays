// Caps the frame rate of everything that animates through requestAnimationFrame (the Rive runtime).
// Must be imported before the runtime. Rive advances by elapsed time, so motion speed is unchanged.
let interval = 0;
let next = performance.now();
const nativeRaf = window.requestAnimationFrame.bind(window);
const nativeCancel = window.cancelAnimationFrame.bind(window);

/** 0 or ≥ 60 = uncapped (display rate). */
export function setFps(fps: number) {
  interval = fps > 0 && fps < 60 ? 1000 / fps : 0;
}

window.requestAnimationFrame = (cb: FrameRequestCallback): number => {
  if (!interval) return nativeRaf(cb);
  const now = performance.now();
  next = Math.max(next + interval, now);
  // negative ids keep timer ids apart from native frame ids
  return -window.setTimeout(() => cb(performance.now()), next - now);
};
window.cancelAnimationFrame = (id: number) => (id < 0 ? window.clearTimeout(-id) : nativeCancel(id));
