// The 3D corgi (Pembroke): an original sitting dog built from distance-field primitives, 11 bones. A
// fox-like head with big upright ears, a white blaze, muzzle, bib and socks, a black nose, a smiling
// open mouth with a tongue on a jaw bone (it pants when happy), a fluffy wagging tail and a double coat.
import { build, type Mesh, type Prim, type V3 } from "./sdf";
import { both, cone, ell, mat, Pattern, type SpeciesDef } from "./species";

const HIPS = 0, SPINE = 1, NECK = 2, HEAD = 3, EAR_L = 4, EAR_R = 5, TAIL = 6, LEG_L = 8, LEG_R = 9, JAW = 10;
const TAIL_PTS: V3[] = [[0, 0.13, -0.37], [0.015, 0.17, -0.44], [0.03, 0.21, -0.48]];
/** The head is modelled around (0, 0.66, 0.1); this scales it up for a cuter, chunkier corgi face. */
const HEAD_SCALE = 1.15;
const head = (p: V3): V3 => [p[0] * HEAD_SCALE, (p[1] - 0.66) * HEAD_SCALE + 0.65, (p[2] - 0.1) * HEAD_SCALE + 0.1];
const PIVOT: V3[] = [
  [0, 0.16, -0.14], [0, 0.34, -0.02], [0, 0.5, 0.06], head([0, 0.66, 0.1]), head([0.085, 0.83, 0.08]), head([-0.085, 0.83, 0.08]),
  TAIL_PTS[0], TAIL_PTS[1], [0.075, 0.4, 0.1], [-0.075, 0.4, 0.1], head([0, 0.648, 0.19]),
];
const PARENT = [-1, HIPS, SPINE, NECK, HEAD, HEAD, HIPS, TAIL, SPINE, SPINE, HEAD];

const FUR = mat({ fur: 1 }), BIB = mat({ fur: 1.2, light: 1 }), LEG = mat({ fur: 0.6 });
const SOCK = mat({ fur: 0.45, light: 1 }), HEAD_FUR = mat({ fur: 0.45 }), CHEEK = mat({ fur: 0.55, light: 0.3 });
const SNOUT = mat({ fur: 0.3 });
const MUZZLE = mat({ fur: 0.25, light: 1 }), BLAZE = mat({ fur: 0.4, light: 1 }), NOSE = mat({ dark: 1 }), MOUTH = mat({ dark: 1 });
const TONGUE = mat({ pink: 1 }), EAR = mat({ fur: 0.4 }), INNER_EAR = mat({ fur: 0.5, light: 0.35, pink: 0.3 });
const SOCKET = mat({ fur: 0.03, dark: 0.6 }), PLUME = mat({ fur: 1.5 });

/** A head part: its centre goes through `head` and its size is scaled to match. */
const onHead = <T extends unknown[]>(make: (h: (p: V3) => V3, k: number) => T) => make(head, HEAD_SCALE);

function prims(): Prim[] {
  return [
    // a long low body sitting on its haunches, with a deep white bib
    ell([0, 0.17, -0.16], [0.24, 0.19, 0.25], HIPS, FUR, 0),
    ...both((s) => [ell([0.16 * s, 0.13, -0.06], [0.1, 0.12, 0.17], HIPS, FUR, 0.06)]),
    ...both((s) => [ell([0.13 * s, 0.025, 0.07], [0.05, 0.028, 0.075], HIPS, SOCK, 0.04)]),
    cone([0, 0.22, -0.12], [0, 0.46, 0.06], 0.22, 0.17, SPINE, FUR, 0.08),
    ell([0, 0.4, 0.1], [0.17, 0.17, 0.13], SPINE, BIB, 0.06),
    cone([0, 0.48, 0.06], [0, 0.6, 0.1], 0.13, 0.11, NECK, FUR, 0.06),
    ell([0, 0.53, 0.13], [0.09, 0.08, 0.06], NECK, BIB, 0.05),
    // short sturdy front legs: ginger at the top, white socks below
    ...both((s) => [
      cone([0.075 * s, 0.38, 0.11], [0.075 * s, 0.2, 0.13], 0.056, 0.048, s > 0 ? LEG_L : LEG_R, LEG, 0.05),
      cone([0.075 * s, 0.2, 0.13], [0.075 * s, 0.06, 0.15], 0.048, 0.043, s > 0 ? LEG_L : LEG_R, SOCK, 0.03),
      ell([0.075 * s, 0.03, 0.17], [0.048, 0.03, 0.058], s > 0 ? LEG_L : LEG_R, SOCK, 0.025),
    ]),
    // a fox-like head: white cheeks and a long white muzzle, a blaze up the forehead, a black nose
    ...onHead((h, k) => {
      const e = (c: V3, r: V3, bone: number, m: number[], kk: number, sub = false) => ell(h(c), [r[0] * k, r[1] * k, r[2] * k], bone, m, kk * k, sub);
      const c = (a: V3, b: V3, r1: number, r2: number, bone: number, m: number[], kk: number, sq = 1, sub = false) =>
        cone(h(a), h(b), r1 * k, r2 * k, bone, m, kk * k, sq, sub);
      return [
        e([0, 0.74, 0.1], [0.135, 0.12, 0.12], HEAD, HEAD_FUR, 0.05),
        ...both((s) => [e([0.068 * s, 0.69, 0.15], [0.072, 0.062, 0.07], HEAD, CHEEK, 0.05)]),
        // a ginger bridge on top of the snout, white below it
        c([0, 0.69, 0.17], [0, 0.672, 0.285], 0.058, 0.036, HEAD, MUZZLE, 0.035),
        c([0, 0.712, 0.17], [0, 0.69, 0.275], 0.04, 0.022, HEAD, SNOUT, 0.03),
        e([0, 0.765, 0.2], [0.018, 0.055, 0.02], HEAD, BLAZE, 0.025),
        e([0, 0.688, 0.31], [0.03, 0.022, 0.018], HEAD, NOSE, 0.012),
        // a smiling open mouth: a dark gap above the jaw, and a tongue resting on it
        e([0, 0.651, 0.24], [0.036, 0.011, 0.048], HEAD, MOUTH, 0.008, true),
        e([0, 0.632, 0.22], [0.04, 0.02, 0.052], JAW, MUZZLE, 0.02),
        e([0, 0.626, 0.26], [0.022, 0.009, 0.03], JAW, TONGUE, 0.006),
        ...both((s) => [e([0.055 * s, 0.752, 0.19], [0.031, 0.029, 0.025], HEAD, SOCKET, 0.01, true)]),
        // big upright ears with a cup carved into the front
        ...both((s) => [
          c([0.075 * s, 0.82, 0.08], [0.13 * s, 1.0, 0.06], 0.06, 0.01, s > 0 ? EAR_L : EAR_R, EAR, 0.03, 2.6),
          c([0.077 * s, 0.84, 0.093], [0.125 * s, 0.985, 0.073], 0.045, 0.006, s > 0 ? EAR_L : EAR_R, INNER_EAR, 0.006, 2.6, true),
        ]),
      ];
    }),
    // a short fluffy tail
    cone(TAIL_PTS[0], TAIL_PTS[1], 0.05, 0.047, TAIL, PLUME, 0.05, 1, false, 0.25),
    cone(TAIL_PTS[1], TAIL_PTS[2], 0.047, 0.04, TAIL + 1, PLUME, 0.03, 1, false, 0.75),
  ];
}

let shape: Mesh | undefined;

export const dog: SpeciesDef = {
  shape: () => (shape ??= build(prims(), [-0.32, -0.03, -0.56], [0.32, 1.06, 0.4], 0.0105)),
  pivot: PIVOT,
  parent: PARENT,
  hips: HIPS,
  spine: SPINE,
  neck: NECK,
  head: HEAD,
  ears: [EAR_L, EAR_R],
  tail: { bones: [TAIL, TAIL + 1], pts: TAIL_PTS, style: "wag" },
  eyes: { at: head([0.055, 0.752, 0.178]), r: 0.027 * HEAD_SCALE, turn: 0.12, slit: 1, pupil: [0.36, 0.44, 0.3], lids: [0.9, 0.3, 0.8, 0.25, 0.05] },
  nose: head([0, 0.688, 0.31]),
  bare: [{ at: head([0, 0.688, 0.31]), r: 0.1 }, { at: head([0, 0.645, 0.25]), r: 0.08 }],
  fur: { shells: 20, len: 0.035, density: 170, gravity: [0, -0.5, -0.3], coat: "triple" },
  pattern: Pattern.plain,
  headY: 0,
  camera: { pos: [0, 0.6, 2.6], look: [0, 0.5, 0], turn: -0.45 },
  shadow: { at: [0, 0.001, -0.1], size: [0.9, 1.0] },
  bounds: { centre: [0, 0.45, -0.1], r: 0.75 },
  palette: (c) => ({
    base: c.coatBase ?? "#d98a4a",
    shade: c.coatShade ?? "#b86c2e",
    light: "#f7f2ea",
    pink: "#d9737f",
    dark: "#120e0d",
    accent: "#000000",
    accent2: "#000000",
    iris: "#5a3418",
    irisOuter: "#24140a",
    lid: "#3a2a20",
    stripes: 0,
    tipping: 0.25,
  }),
  // a happy dog pants: the jaw opens and closes quickly; otherwise it rests in a soft smile
  animate: (c) => {
    const panting = !c.still && !c.asleep && c.joyful;
    c.bones[JAW].rotation.x = c.asleep ? 0 : panting ? 0.14 + 0.06 * Math.sin(c.t * 15) : 0.05;
  },
};
