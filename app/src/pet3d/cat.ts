// The 3D Persian: an original sitting cat built from distance-field primitives, 16 bones. A flat round
// face, big round eyes, small low ears, a mane, fluffy trousers, a plume tail and a long triple coat.
import { build, type Mesh, type Prim, type V3 } from "./sdf";
import { both, cone, ell, mat, Pattern, type SpeciesDef } from "./species";

const HIPS = 0, SPINE = 1, NECK = 2, HEAD = 3, EAR_L = 4, EAR_R = 5, TAIL = 6, LEG_L = 14, LEG_R = 15;
const TAIL_PTS: V3[] = [
  [0.0, 0.12, -0.34], [0.07, 0.06, -0.4], [0.17, 0.035, -0.385], [0.25, 0.03, -0.3], [0.3, 0.03, -0.19],
  [0.31, 0.03, -0.07], [0.29, 0.03, 0.04], [0.25, 0.032, 0.13], [0.2, 0.035, 0.19],
];
/** The head is modelled around (0, 0.72, 0.08); this scales it up and seats it lower on a short neck. */
const HEAD_SCALE = 1.22;
const head = (p: V3): V3 => [p[0] * HEAD_SCALE, (p[1] - 0.72) * HEAD_SCALE + 0.64, (p[2] - 0.08) * HEAD_SCALE + 0.1];
const PIVOT: V3[] = [
  [0, 0.18, -0.12], [0, 0.36, -0.02], [0, 0.5, 0.05], head([0, 0.72, 0.08]), head([0.1, 0.86, 0.07]), head([-0.1, 0.86, 0.07]),
  ...TAIL_PTS.slice(0, 8), [0.075, 0.42, 0.1], [-0.075, 0.42, 0.1],
];
const PARENT = [-1, HIPS, SPINE, NECK, HEAD, HEAD, HIPS, TAIL, TAIL + 1, TAIL + 2, TAIL + 3, TAIL + 4, TAIL + 5, TAIL + 6, SPINE, SPINE];

// long body fur, a mane round the neck and chest, fluffy trousers, a plume tail, short fur on the face and paws
const FUR = mat({ fur: 1 }), PANTS = mat({ fur: 1.15 }), RUFF = mat({ fur: 1.35, light: 0.55 }), LEG = mat({ fur: 0.7 });
const HEAD_FUR = mat({ fur: 0.5 }), CHEEK = mat({ fur: 0.75, light: 0.2 }), MUZZLE = mat({ fur: 0.28, light: 0.5 }), NOSE = mat({ pink: 1 });
const INNER_EAR = mat({ fur: 0.35, light: 0.4, pink: 0.7 }), EAR = mat({ fur: 0.35 }), PAW = mat({ fur: 0.45, light: 0.3 });
const SOCKET = mat({ fur: 0.03, pink: 0.25 }), PLUME = mat({ fur: 1.5 });

/** A head part: its centre goes through `head` and its size is scaled to match. */
const onHead = <T extends unknown[]>(make: (h: (p: V3) => V3, k: number) => T) => make(head, HEAD_SCALE);

function prims(): Prim[] {
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
      const e = (c: V3, r: V3, bone: number, m: number[], kk: number, sub = false) => ell(h(c), [r[0] * k, r[1] * k, r[2] * k], bone, m, kk * k, sub);
      const c = (a: V3, b: V3, r1: number, r2: number, bone: number, m: number[], kk: number, sq: number, sub = false) =>
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

export const cat: SpeciesDef = {
  shape: () => (shape ??= build(prims(), [-0.32, -0.03, -0.5], [0.42, 1.0, 0.32], 0.0105)),
  pivot: PIVOT,
  wear: { top: [0, 0.905, 0.1], r: 0.18, neck: [0, 0.52, 0.07], neckR: 0.2, chest: [0, 0.45, 0.27], badge: 0.05 },
  parent: PARENT,
  hips: HIPS,
  spine: SPINE,
  neck: NECK,
  head: HEAD,
  ears: [EAR_L, EAR_R],
  tail: { bones: [0, 1, 2, 3, 4, 5, 6, 7].map((i) => TAIL + i), pts: TAIL_PTS, style: "curl" },
  // hungry: hunched low over its front paws; gloomy: lying down
  poses: {
    crouch: { drop: 0.1, rot: { [SPINE]: [0.62, 0, 0], [NECK]: [-0.3, 0, 0], [HEAD]: [-0.2, 0, 0], [LEG_L]: [-0.62, 0, 0], [LEG_R]: [-0.62, 0, 0] } },
    // lying down like a real cat: body flat and long on the floor, front paws resting just out in front of
    // the chest (the forelegs shortened, as if folded at the elbow), head low and forward, ears back
    down: {
      drop: 0.15,
      fwd: 0.03,
      rot: { [SPINE]: [1.1, 0, 0], [NECK]: [-0.5, 0, 0], [HEAD]: [-0.3, 0, 0], [LEG_L]: [-1.9, 0, 0], [LEG_R]: [-1.9, 0, 0], [EAR_L]: [-0.5, 0, 0.35], [EAR_R]: [-0.5, 0, -0.35] },
      scale: { [LEG_L]: [1, 0.7, 1], [LEG_R]: [1, 0.7, 1] },
    },
  },
  mouth: { at: head([0, 0.756, 0.222]), w: 0.04, drop: 0.015, color: 0x4a2c2a, r: 0.0042 },
  tapLeg: LEG_L,
  legs: [LEG_L, LEG_R],
  eyes: { at: head([0.058, 0.8, 0.162]), r: 0.038 * HEAD_SCALE, turn: 0.08, slit: 1.35, pupil: [0.32, 0.45, 0.2], lids: [0.95, 0.25, 0.85, 0.2, 0.04] },
  nose: head([0, 0.772, 0.209]),
  bare: [{ at: head([0, 0.772, 0.209]), r: 0.06 }],
  whiskers: { roots: [0, 1, 2, 3, 4].map((j) => head([0.03 + j * 0.005, 0.752 - j * 0.005, 0.2 - j * 0.004])), len: 0.22, droop: 0.022, color: 0xf4efe6 },
  fur: { shells: 28, len: 0.07, density: 150, gravity: [0, -0.9, -0.2], coat: "triple" },
  pattern: Pattern.tabby,
  headY: 0.66,
  camera: { pos: [0, 0.6, 2.6], look: [0, 0.5, 0], turn: -0.45 },
  shadow: { at: [0.05, 0.001, -0.08], size: [1.0, 0.9] },
  bounds: { centre: [0, 0.45, -0.1], r: 0.75 },
  palette: (c) => ({
    base: c.coatBase ?? "#ecd3ad",
    shade: c.coatShade ?? "#c49a68",
    light: c.coatHighlight ?? "#f8eedd",
    pink: "#d97a80",
    dark: "#1a1412",
    accent: "#000000",
    accent2: "#000000",
    iris: "#c8701f",
    irisOuter: "#6a4a1c",
    lid: c.coatBase ?? "#ecd3ad",
    stripes: 0,
    tipping: 0.3,
  }),
};
