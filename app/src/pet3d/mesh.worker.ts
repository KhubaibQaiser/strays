// Builds the species meshes off the page's main thread (review-spec A6): meshing a species takes
// 0.3–0.6 s, which used to freeze the overlay the first time a pet of it appeared.
import { SPECIES_3D } from "./species3d";

self.onmessage = ({ data: name }: MessageEvent<string>) => {
  try {
    const mesh = SPECIES_3D[name].shape();
    // copies, so the worker's own cache stays usable
    const copy = Object.fromEntries(Object.entries(mesh).map(([k, a]) => [k, (a as Float32Array).slice()]));
    self.postMessage({ name, mesh: copy }, { transfer: Object.values(copy).map((a) => a.buffer) });
  } catch (e) {
    self.postMessage({ name, error: String(e) });
  }
};
