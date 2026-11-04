// Gets the 3D species' meshes built in a worker before their pets are drawn, so the page never
// freezes while one is built. If the worker can't run, a mesh is built on the page as before.
import type { Mesh } from "./sdf";
import { SPECIES_3D } from "./species3d";

const pending = new Map<string, Promise<void>>();
const ready = new Set<string>();

/** Whether `name` can be drawn now without building its mesh on the page. */
export const meshBuilt = (name: string) => !SPECIES_3D[name] || ready.has(name);
let worker: Worker | null | undefined;

/** Resolves once `name`'s mesh is ready (at once for one already built or not drawn in 3D). */
export function meshReady(name: string, problem: (key: string, text: string) => void): Promise<void> {
  const def = SPECIES_3D[name];
  if (!def) return Promise.resolve();
  let p = pending.get(name);
  if (p) return p;
  p = new Promise<void>((done) => {
    try {
      worker ??= new Worker(new URL("./mesh.worker.ts", import.meta.url), { type: "module" });
    } catch (e) {
      problem("3d-worker", `The 3D shapes are built in the pets' window, which can pause it briefly: ${e}`);
      worker = null;
    }
    const finish = () => {
      ready.add(name);
      done();
    };
    if (!worker) return finish();
    const w = worker;
    const onMessage = ({ data }: MessageEvent<{ name: string; mesh?: Mesh; error?: string }>) => {
      if (data.name !== name) return;
      w.removeEventListener("message", onMessage);
      w.removeEventListener("error", onError);
      if (data.mesh) {
        const mesh = data.mesh;
        def.shape = () => mesh;
      } else problem("3d-worker", `The ${name} shape couldn't be built in the background (${data.error}), so it's built in the pets' window`);
      finish();
    };
    // a worker that can't start (blocked, failed to load) answers nothing: build on the page instead
    const onError = (e: ErrorEvent) => {
      w.removeEventListener("message", onMessage);
      problem("3d-worker", `The ${name} shape couldn't be built in the background (${e.message || "the worker didn't start"}), so it's built in the pets' window`);
      finish();
    };
    w.addEventListener("message", onMessage);
    w.addEventListener("error", onError);
    w.postMessage(name);
  });
  pending.set(name, p);
  return p;
}
