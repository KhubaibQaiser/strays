// Signed-distance modelling for the 3D pet prototype: a pet is a list of primitives, each bound to a bone.
// `build` turns them into a smooth mesh (surface nets, snapped onto the surface) with per-vertex normals,
// skin weights derived from which primitives a vertex sits on, and material values (fur length, light coat,
// bare pink skin, dark, accents, feather bars, position along a chain) blended the same way, so colours
// and fur change smoothly.
export type V3 = [number, number, number];

export type Prim = {
  /** distance from (x, y, z) to this primitive's surface; negative inside */
  d: (x: number, y: number, z: number) => number;
  /** centre and radius of a sphere holding the primitive, to skip far ones */
  c: V3;
  r: number;
  bone: number;
  /** material: [fur length factor, light coat, bare pink skin, dark, accent, accent 2, feather bars, white] (see MAT) */
  m: number[];
  /** smooth-union radius with the shape so far (0 = hard edge) */
  k: number;
  /** carve this shape out instead of adding it */
  sub?: boolean;
  /** position along a chain, 0..1 (tail rings) */
  t?: number;
};

const len = (x: number, y: number, z: number) => Math.sqrt(x * x + y * y + z * z);

export function ellipsoid(c: V3, r: V3): (x: number, y: number, z: number) => number {
  return (x, y, z) => {
    const px = x - c[0], py = y - c[1], pz = z - c[2];
    const k0 = len(px / r[0], py / r[1], pz / r[2]);
    const k1 = len(px / (r[0] * r[0]), py / (r[1] * r[1]), pz / (r[2] * r[2]));
    return k1 === 0 ? -Math.min(...r) : (k0 * (k0 - 1)) / k1;
  };
}

/** A capsule whose radius goes from r1 at a to r2 at b (Inigo Quilez's exact round cone), optionally
 *  flattened along z (ears) or x (a beak). */
export function roundCone(a: V3, b: V3, r1: number, r2: number, squashZ = 1, squashX = 1): (x: number, y: number, z: number) => number {
  const bx = (b[0] - a[0]) * squashX, by = b[1] - a[1], bz = (b[2] - a[2]) * squashZ;
  const l2 = bx * bx + by * by + bz * bz;
  if (l2 === 0) throw new Error("roundCone needs two different end points");
  const rr = r1 - r2;
  const a2 = l2 - rr * rr;
  const il2 = 1 / l2;
  return (x, y, z) => {
    const px = (x - a[0]) * squashX, py = y - a[1], pz = (z - a[2]) * squashZ;
    const yy = px * bx + py * by + pz * bz;
    const zz = yy - l2;
    const qx = px * l2 - bx * yy, qy = py * l2 - by * yy, qz = pz * l2 - bz * yy;
    const x2 = qx * qx + qy * qy + qz * qz;
    const y2 = yy * yy * l2;
    const z2 = zz * zz * l2;
    const k = Math.sign(rr) * rr * rr * x2;
    let d: number;
    if (Math.sign(zz) * a2 * z2 > k) d = Math.sqrt(x2 + z2) * il2 - r2;
    else if (Math.sign(yy) * a2 * y2 < k) d = Math.sqrt(x2 + y2) * il2 - r1;
    else d = (Math.sqrt(x2 * a2 * il2) + yy * rr) * il2 - r1;
    // squashing stretches distances; dividing keeps the estimate conservative
    return d / Math.max(squashZ, squashX);
  };
}

function smin(a: number, b: number, k: number) {
  if (k <= 0) return Math.min(a, b);
  const h = Math.max(k - Math.abs(a - b), 0) / k;
  return Math.min(a, b) - h * h * k * 0.25;
}

function smax(a: number, b: number, k: number) {
  return -smin(-a, -b, k);
}

/** One primitive's distance, or a cheap lower bound when the point is clearly outside it. */
function primDist(p: Prim, x: number, y: number, z: number) {
  const far = len(x - p.c[0], y - p.c[1], z - p.c[2]) - p.r;
  return far > 0.06 ? far : p.d(x, y, z);
}

export function field(prims: Prim[], x: number, y: number, z: number) {
  let d = 1e9;
  for (const p of prims) {
    const pd = primDist(p, x, y, z);
    d = p.sub ? smax(d, -pd, p.k) : smin(d, pd, p.k);
  }
  return d;
}

export type Mesh = {
  position: Float32Array;
  normal: Float32Array;
  index: Uint32Array;
  skinIndex: Uint16Array;
  skinWeight: Float32Array;
  /** per vertex: fur, light, pink, chain */
  mat: Float32Array;
  /** per vertex: dark, accent, accent 2, feather bars */
  mat2: Float32Array;
  /** per vertex: white (bare white skin, a macaw's face) */
  mat3: Float32Array;
};

/** How many material values a primitive carries. */
export const MAT = 8;

/** Meshes the surface of `prims` inside the box [lo, hi] with cells of size h. */
export function build(prims: Prim[], lo: V3, hi: V3, h: number): Mesh {
  const nx = Math.ceil((hi[0] - lo[0]) / h) + 1;
  const ny = Math.ceil((hi[1] - lo[1]) / h) + 1;
  const nz = Math.ceil((hi[2] - lo[2]) / h) + 1;
  const at = (i: number, j: number, k: number) => i + nx * (j + ny * k);
  const f = new Float32Array(nx * ny * nz);
  for (let k = 0; k < nz; k++)
    for (let j = 0; j < ny; j++)
      for (let i = 0; i < nx; i++) f[at(i, j, k)] = field(prims, lo[0] + i * h, lo[1] + j * h, lo[2] + k * h);

  // one vertex per cell the surface crosses: the average of its edges' crossings
  const cellVert = new Int32Array(nx * ny * nz).fill(-1);
  const pos: number[] = [];
  const corners = [
    [0, 0, 0], [1, 0, 0], [0, 1, 0], [1, 1, 0], [0, 0, 1], [1, 0, 1], [0, 1, 1], [1, 1, 1],
  ];
  const edges = [
    [0, 1], [2, 3], [4, 5], [6, 7], [0, 2], [1, 3], [4, 6], [5, 7], [0, 4], [1, 5], [2, 6], [3, 7],
  ];
  const v = new Float32Array(8);
  for (let k = 0; k < nz - 1; k++)
    for (let j = 0; j < ny - 1; j++)
      for (let i = 0; i < nx - 1; i++) {
        let inside = 0;
        for (let c = 0; c < 8; c++) {
          v[c] = f[at(i + corners[c][0], j + corners[c][1], k + corners[c][2])];
          if (v[c] < 0) inside++;
        }
        if (inside === 0 || inside === 8) continue;
        let sx = 0, sy = 0, sz = 0, n = 0;
        for (const [a, b] of edges) {
          if (v[a] < 0 === v[b] < 0) continue;
          const t = v[a] / (v[a] - v[b]);
          sx += corners[a][0] + t * (corners[b][0] - corners[a][0]);
          sy += corners[a][1] + t * (corners[b][1] - corners[a][1]);
          sz += corners[a][2] + t * (corners[b][2] - corners[a][2]);
          n++;
        }
        cellVert[at(i, j, k)] = pos.length / 3;
        pos.push(lo[0] + (i + sx / n) * h, lo[1] + (j + sy / n) * h, lo[2] + (k + sz / n) * h);
      }

  // one quad per grid edge the surface crosses, joining the four cells around that edge
  const idx: number[] = [];
  const quad = (a: number, b: number, c: number, d: number) => {
    if (a < 0 || b < 0 || c < 0 || d < 0) return;
    idx.push(a, b, c, a, c, d);
  };
  for (let k = 1; k < nz - 1; k++)
    for (let j = 1; j < ny - 1; j++)
      for (let i = 1; i < nx - 1; i++) {
        const s = f[at(i, j, k)] < 0;
        if (s !== f[at(i + 1, j, k)] < 0)
          quad(cellVert[at(i, j - 1, k - 1)], cellVert[at(i, j, k - 1)], cellVert[at(i, j, k)], cellVert[at(i, j - 1, k)]);
        if (s !== f[at(i, j + 1, k)] < 0)
          quad(cellVert[at(i - 1, j, k - 1)], cellVert[at(i, j, k - 1)], cellVert[at(i, j, k)], cellVert[at(i - 1, j, k)]);
        if (s !== f[at(i, j, k + 1)] < 0)
          quad(cellVert[at(i - 1, j - 1, k)], cellVert[at(i, j - 1, k)], cellVert[at(i, j, k)], cellVert[at(i - 1, j, k)]);
      }

  // snap vertices onto the surface and take normals from the field's gradient
  const count = pos.length / 3;
  const position = new Float32Array(pos);
  const normal = new Float32Array(count * 3);
  const e = h * 0.25;
  const grad = (x: number, y: number, z: number): V3 => [
    field(prims, x + e, y, z) - field(prims, x - e, y, z),
    field(prims, x, y + e, z) - field(prims, x, y - e, z),
    field(prims, x, y, z + e) - field(prims, x, y, z - e),
  ];
  for (let i = 0; i < count; i++) {
    let x = position[i * 3], y = position[i * 3 + 1], z = position[i * 3 + 2];
    for (let step = 0; step < 2; step++) {
      const d = field(prims, x, y, z);
      const g = grad(x, y, z);
      const gl = len(...g) || 1;
      x -= (d * g[0]) / gl;
      y -= (d * g[1]) / gl;
      z -= (d * g[2]) / gl;
    }
    position.set([x, y, z], i * 3);
    const g = grad(x, y, z);
    const gl = len(...g) || 1;
    normal.set([g[0] / gl, g[1] / gl, g[2] / gl], i * 3);
  }

  // face winding follows the normals, so lighting and culling agree
  const index = new Uint32Array(idx);
  for (let t = 0; t < index.length; t += 3) {
    const [a, b, c] = [index[t], index[t + 1], index[t + 2]];
    const ux = position[b * 3] - position[a * 3], uy = position[b * 3 + 1] - position[a * 3 + 1], uz = position[b * 3 + 2] - position[a * 3 + 2];
    const vx = position[c * 3] - position[a * 3], vy = position[c * 3 + 1] - position[a * 3 + 1], vz = position[c * 3 + 2] - position[a * 3 + 2];
    const fx = uy * vz - uz * vy, fy = uz * vx - ux * vz, fz = ux * vy - uy * vx;
    const nsum = (o: number) => normal[a * 3 + o] + normal[b * 3 + o] + normal[c * 3 + o];
    if (fx * nsum(0) + fy * nsum(1) + fz * nsum(2) < 0) {
      index[t + 1] = c;
      index[t + 2] = b;
    }
  }

  // skin weights and materials: each primitive pulls its bone and material up near its own surface
  const skinIndex = new Uint16Array(count * 4);
  const skinWeight = new Float32Array(count * 4);
  const mat = new Float32Array(count * 4);
  const mat2 = new Float32Array(count * 4);
  const mat3 = new Float32Array(count);
  const bones = 1 + Math.max(...prims.map((p) => p.bone));
  const w = new Float32Array(bones);
  for (let i = 0; i < count; i++) {
    const x = position[i * 3], y = position[i * 3 + 1], z = position[i * 3 + 2];
    w.fill(0);
    const m = new Array(MAT).fill(0);
    let chain = 0;
    let total = 0;
    const carved: [number, Prim][] = [];
    for (const p of prims) {
      const d = primDist(p, x, y, z);
      if (p.sub) {
        carved.push([Math.exp(-Math.abs(d) / 0.004), p]);
        continue;
      }
      w[p.bone] += Math.exp(-Math.max(d, 0) / 0.018);
      const om = Math.exp(-Math.max(d, 0) / 0.01);
      for (let o = 0; o < MAT; o++) m[o] += om * (p.m[o] ?? 0);
      chain += om * (p.t ?? 0);
      total += om;
    }
    for (let o = 0; o < MAT; o++) m[o] /= total || 1;
    chain /= total || 1;
    // a carved surface (inner ear, eye socket, mouth) takes the carving shape's material
    for (const [s, p] of carved) for (let o = 0; o < MAT; o++) m[o] += ((p.m[o] ?? 0) - m[o]) * s;
    mat.set([m[0], m[1], m[2], chain], i * 4);
    mat2.set([m[3], m[4], m[5], m[6]], i * 4);
    mat3[i] = m[7];
    const top = [...w.keys()].sort((a, b) => w[b] - w[a]).slice(0, 4);
    const sum = top.reduce((s, b) => s + w[b], 0) || 1;
    top.forEach((b, n) => {
      skinIndex[i * 4 + n] = b;
      skinWeight[i * 4 + n] = w[b] / sum;
    });
  }
  return { position, normal, index, skinIndex, skinWeight, mat, mat2, mat3 };
}
