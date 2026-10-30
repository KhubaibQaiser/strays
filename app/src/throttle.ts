// Cap the frame rate for everything that animates through requestAnimationFrame (the Rive runtime).
// Must be imported before the runtime. Rive advances by elapsed time, so motion speed is unchanged.
// ponytail: prototype-only; production should drive the low-level runtime's loop directly.
const fps = Number(new URLSearchParams(location.search).get("fps") ?? 30);

if (fps > 0 && fps < 60) {
  const interval = 1000 / fps;
  let next = performance.now();
  window.requestAnimationFrame = (cb: FrameRequestCallback): number => {
    const now = performance.now();
    next = Math.max(next + interval, now);
    return window.setTimeout(() => cb(performance.now()), next - now);
  };
  window.cancelAnimationFrame = (id: number) => window.clearTimeout(id);
}
