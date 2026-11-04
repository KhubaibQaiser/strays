// The 3D cat's shape and rig: an original sitting cat built from distance-field primitives, 16 bones.
import { build, ellipsoid, roundCone, type Mesh, type Prim, type V3 } from "./sdf";

export const HIPS = 0, SPINE = 1, NECK = 2, HEAD = 3, EAR_L = 4, EAR_R = 5, TAIL = 6, LEG_L = 14, LEG_R = 15;
export const TAIL_PTS: V3[] = [
  [0.0, 0.12, -0.34], [0.07, 0.06, -0.4], [0.17, 0.035, -0.385], [0.25, 0.03, -0.3], [0.3, 0.03, -0.19],
  [0.31, 0.03, -0.07], [0.29, 0.03, 0.04], [0.25, 0.032, 0.13], [0.2, 0.035, 0.19],
];
/** The head is modelled around (0, 0.72, 0.08); this scales it up and seats it lower on a short neck. */
export const HEAD_SCALE = 1.22;
export const head = (p: V3): V3 => [p[0] * HEAD_SCALE, (p[1] - 0.72) * HEAD_SCALE + 0.64, (p[2] - 0.08) * HEAD_SCALE + 0.1];
/** Where the eyes sit (centre for the pet's left eye; mirror x for the right) and how big they are. */
export const EYE = head([0.058, 0.8, 0.162]);
export const EYE_R = 0.038 * HEAD_SCALE;
export const NOSE_AT = head([0, 0.772, 0.209]);
/** Root of whisker `j` (0..4) on the pet's left side; mirror x for the right. */
export const whiskerRoot = (j: number): V3 => head([0.03 + j * 0.005, 0.752 - j * 0.005, 0.2 - j * 0.004]);
export const PIVOT: V3[] = [
  [0, 0.18, -0.12], [0, 0.36, -0.02], [0, 0.5, 0.05], head([0, 0.72, 0.08]), head([0.1, 0.86, 0.07]), head([-0.1, 0.86, 0.07]),
  ...TAIL_PTS.slice(0, 8), [0.075, 0.42, 0.1], [-0.075, 0.42, 0.1],
];
export const PARENT = [-1, HIPS, SPINE, NECK, HEAD, HEAD, HIPS, TAIL, TAIL + 1, TAIL + 2, TAIL + 3, TAIL + 4, TAIL + 5, TAIL + 6, SPINE, SPINE];

// material values: [fur length factor, light coat, bare pink skin]. A Persian: long body fur, a mane
// round the neck and chest, fluffy trousers, a plume tail, and short fur only on the face and paws.
const FUR: V3 = [1, 0, 0], PANTS: V3 = [1.15, 0, 0], RUFF: V3 = [1.35, 0.55, 0], LEG: V3 = [0.7, 0, 0];
const HEAD_FUR: V3 = [0.5, 0, 0], CHEEK: V3 = [0.75, 0.2, 0], MUZZLE: V3 = [0.28, 0.5, 0], NOSE: V3 = [0, 0, 1];
const INNER_EAR: V3 = [0.35, 0.4, 0.7], EAR: V3 = [0.35, 0, 0], PAW: V3 = [0.45, 0.3, 0], SOCKET: V3 = [0.03, 0, 0.25];
const PLUME: V3 = [1.5, 0, 0];

/** A head part: its centre goes through `head` and its size is scaled to match. */
const onHead = <T extends unknown[]>(make: (h: (p: V3) => V3, k: number) => T) => make(head, HEAD_SCALE);

function ell(c: V3, r: V3, bone: number, m: V3, k: number, sub = false): Prim {
  return { d: ellipsoid(c, r), c, r: Math.max(...r), bone, m, k, sub };
}

function cone(a: V3, b: V3, r1: number, r2: number, bone: number, m: V3, k: number, squashZ = 1, sub = false, t?: number): Prim {
  const c: V3 = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2];
  const half = Math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2]) / 2;
  return { d: roundCone(a, b, r1, r2, squashZ), c, r: half + Math.max(r1, r2), bone, m, k, sub, t };
}

/** The pet's left (+x) and right (-x) copies of a part. */
const both = (make: (s: number) => Prim[]) => [...make(1), ...make(-1)];

function catPrims(): Prim[] {
  const tail = TAIL_PTS.slice(0, 8).map((p, i) =>
    cone(p, TAIL_PTS[i + 1], 0.05 - i * 0.002, 0.05 - (i + 1) * 0.002, TAIL + i, PLUME, i === 0 ? 0.05 : 0.025, 1, false, (i + 0.5) / 8),
  );
  return [
    // a cobby body: broad haunches, deep chest, short thick legs
    ell([0, 0.2, -0.13], [0.22, 0.2, 0.23], HIPS, FUR, 0),
    ...both((s) => [ell([0.14 * s, 0.16, -0.05], [0.1, 0.13, 0.17], HIPS, PANTS, 0.06)]),
    ...both((s) => [ell([0.13 * s, 0.03, 0.07], [0.05, 0.03, 0.07], HIPS, PAW, 0.04)]),
    cone([0, 0.24, -0.1], [0, 0.48, 0.05], 0.21, 0.16, SPINE, FUR, 0.08),
    ell([0, 0.43, 0.08], [0.16, 0.16, 0.13], SPINE, RUFF, 0.06),
    cone([0, 0.5, 0.04], [0, 0.6, 0.08], 0.15, 0.13, NECK, RUFF, 0.06),
    ...both((s) => [
      cone([0.075 * s, 0.4, 0.1], [0.075 * s, 0.06, 0.15], 0.06, 0.045, s > 0 ? LEG_L : LEG_R, LEG, 0.05),
      ell([0.075 * s, 0.03, 0.17], [0.048, 0.03, 0.055], s > 0 ? LEG_L : LEG_R, PAW, 0.025),
    ]),
    // a round head with a flat face: full cheeks, a short muzzle, a tiny nose set high, big round eyes
    ...onHead((h, k) => {
      const e = (c: V3, r: V3, bone: number, m: V3, kk: number, sub = false) => ell(h(c), [r[0] * k, r[1] * k, r[2] * k], bone, m, kk * k, sub);
      const c = (a: V3, b: V3, r1: number, r2: number, bone: number, m: V3, kk: number, sq: number, sub = false) =>
        cone(h(a), h(b), r1 * k, r2 * k, bone, m, kk * k, sq, sub);
      return [
        e([0, 0.785, 0.085], [0.15, 0.13, 0.125], HEAD, HEAD_FUR, 0.05),
        ...both((s) => [e([0.07 * s, 0.75, 0.135], [0.075, 0.065, 0.065], HEAD, CHEEK, 0.05)]),
        e([0, 0.748, 0.18], [0.048, 0.032, 0.03], HEAD, MUZZLE, 0.035),
        e([0, 0.722, 0.165], [0.03, 0.02, 0.025], HEAD, MUZZLE, 0.02),
        e([0, 0.772, 0.209], [0.024, 0.016, 0.013], HEAD, NOSE, 0.008),
        ...both((s) => [e([0.058 * s, 0.8, 0.176], [0.042, 0.04, 0.03], HEAD, SOCKET, 0.012, true)]),
        // small rounded ears, set low and wide, tipped forward, with a cup carved into the front
        ...both((s) => [
          c([0.095 * s, 0.86, 0.065], [0.135 * s, 0.935, 0.075], 0.042, 0.014, s > 0 ? EAR_L : EAR_R, EAR, 0.03, 2.4),
          c([0.096 * s, 0.868, 0.08], [0.131 * s, 0.928, 0.086], 0.03, 0.01, s > 0 ? EAR_L : EAR_R, INNER_EAR, 0.006, 2.4, true),
        ]),
      ];
    }),
    ...tail,
  ];
}

let shape: Mesh | undefined;
/** The cat's mesh, built once and shared by every pet (about half a second on first use). */
export function catShape(): Mesh {
  return (shape ??= build(catPrims(), [-0.32, -0.03, -0.5], [0.42, 1.0, 0.32], 0.0105));
}

