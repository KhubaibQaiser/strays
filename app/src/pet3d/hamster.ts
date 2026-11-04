// The 3D hamster (golden Syrian): an original sitting hamster built from distance-field primitives, 10
// bones. An upright potato body that runs straight into a big round head, chubby cheeks that puff out
// when it eats, black bead eyes, small round pink ears, a white belly and muzzle, tiny pink hands held
// at the chest (it grooms its face with them), whiskers, quick sniffing and fast breathing.
import { build, type Mesh, type Prim, type V3 } from "./sdf";
import { both, cone, ell, mat, Pattern, type SpeciesDef } from "./species";

const HIPS = 0, SPINE = 1, NECK = 2, HEAD = 3, EAR_L = 4, EAR_R = 5, PAW_L = 6, PAW_R = 7, CHEEK_L = 8, CHEEK_R = 9;
const PIVOT: V3[] = [
  [0, 0.2, -0.05], [0, 0.36, 0], [0, 0.5, 0.03], [0, 0.58, 0.05], [0.11, 0.76, 0], [-0.11, 0.76, 0],
  [0.085, 0.42, 0.12], [-0.085, 0.42, 0.12], [0.09, 0.55, 0.1], [-0.09, 0.55, 0.1],
];
const PARENT = [-1, HIPS, SPINE, NECK, HEAD, HEAD, SPINE, SPINE, HEAD, HEAD];

const FUR = mat({ fur: 1 }), BELLY = mat({ fur: 0.85, light: 1 }), HEAD_FUR = mat({ fur: 0.7 }), CHEEK = mat({ fur: 0.8, light: 0.45 });
const MUZZLE = mat({ fur: 0.4, light: 1 }), NOSE = mat({ pink: 1 }), SKIN = mat({ fur: 0.05, pink: 1 });
const EAR = mat({ fur: 0.2, pink: 0.35 }), INNER_EAR = mat({ fur: 0.08, pink: 0.85 }), SOCKET = mat({ fur: 0.03, dark: 0.5 });

function prims(): Prim[] {
  return [
    // an upright pear of a body with a white belly, and pink feet peeking out
    ell([0, 0.25, -0.04], [0.24, 0.25, 0.22], HIPS, FUR, 0),
    ell([0, 0.27, 0.09], [0.17, 0.2, 0.11], SPINE, BELLY, 0.06),
    ...both((s) => [ell([0.12 * s, 0.02, 0.13], [0.045, 0.02, 0.06], HIPS, SKIN, 0.03)]),
    // a big round head with no neck to speak of, full cheeks and a short white muzzle
    ell([0, 0.6, 0.04], [0.2, 0.18, 0.18], HEAD, HEAD_FUR, 0.12),
    ...both((s) => [ell([0.1 * s, 0.54, 0.1], [0.1, 0.085, 0.09], s > 0 ? CHEEK_L : CHEEK_R, CHEEK, 0.06)]),
    ell([0, 0.55, 0.2], [0.06, 0.05, 0.05], HEAD, MUZZLE, 0.04),
    ell([0, 0.566, 0.244], [0.017, 0.012, 0.011], HEAD, NOSE, 0.008),
    ...both((s) => [ell([0.1 * s, 0.645, 0.165], [0.04, 0.04, 0.03], HEAD, SOCKET, 0.012, true)]),
    // small rounded ears with a pink cup
    ...both((s) => [
      cone([0.1 * s, 0.73, 0.0], [0.14 * s, 0.81, -0.01], 0.052, 0.046, s > 0 ? EAR_L : EAR_R, EAR, 0.03, 3),
      cone([0.1 * s, 0.738, 0.012], [0.137 * s, 0.805, 0.004], 0.04, 0.034, s > 0 ? EAR_L : EAR_R, INNER_EAR, 0.005, 3, true),
    ]),
    // short arms with tiny pink hands held together at the chest
    ...both((s) => [
      cone([0.09 * s, 0.43, 0.12], [0.05 * s, 0.38, 0.19], 0.036, 0.026, s > 0 ? PAW_L : PAW_R, FUR, 0.04),
      ell([0.04 * s, 0.375, 0.205], [0.023, 0.018, 0.016], s > 0 ? PAW_L : PAW_R, SKIN, 0.012),
    ]),
  ];
}

let shape: Mesh | undefined;

export const hamster: SpeciesDef = {
  shape: () => (shape ??= build(prims(), [-0.3, -0.03, -0.3], [0.3, 0.9, 0.32], 0.0095)),
  pivot: PIVOT,
  parent: PARENT,
  hips: HIPS,
  spine: SPINE,
  neck: NECK,
  head: HEAD,
  ears: [EAR_L, EAR_R],
  eyes: { at: [0.1, 0.645, 0.152], r: 0.034, turn: 0.55, slit: 1, pupil: [0.75, 0.8, 0.7], lids: [1.2, 0.1, 1.1, 0.1, 0] },
  nose: [0, 0.566, 0.244],
  bare: [{ at: [0, 0.566, 0.244], r: 0.045 }],
  whiskers: { roots: [0, 1, 2, 3].map((j) => [0.03 + j * 0.004, 0.555 - j * 0.006, 0.225 - j * 0.004] as V3), len: 0.2, droop: 0.02, color: 0xf8f4ee },
  fur: { shells: 18, len: 0.028, density: 200, gravity: [0, -0.3, -0.1], coat: "triple" },
  pattern: Pattern.plain,
  headY: 0,
  camera: { pos: [0, 0.55, 2.6], look: [0, 0.45, 0], turn: -0.35 },
  shadow: { at: [0, 0.001, 0], size: [0.75, 0.7] },
  bounds: { centre: [0, 0.4, 0], r: 0.6 },
  palette: (c) => ({
    base: c.coatBase ?? "#e3a564",
    shade: c.coatShade ?? "#c4843f",
    light: "#fbf5ec",
    pink: "#efa3a7",
    dark: "#141010",
    accent: "#000000",
    accent2: "#000000",
    iris: "#241a18",
    irisOuter: "#0a0707",
    lid: c.coatBase ?? "#e3a564",
    stripes: 0,
    tipping: 0.2,
  }),
  animate: (c) => {
    const { bones: b, mem, t } = c;
    const lively = !c.still && !c.asleep;
    // sniffing: bursts of quick nose-and-cheek twitches every few seconds
    if (lively && t > (mem.sniffAt ?? 0)) {
      mem.sniffAt = t + 2 + Math.random() * 4;
      mem.sniffEnd = t + 0.6 + Math.random() * 0.8;
    }
    const sniffing = lively && t < (mem.sniffEnd ?? 0);
    const twitch = sniffing ? Math.sin(t * 38) * 0.5 + 0.5 : 0;
    // grooming: now and then, or when petted, the hands come up and rub the face with the eyes shut
    mem.groomAt ??= t + 8 + Math.random() * 10;
    if (lively && t > mem.groomAt) {
      mem.groomAt = t + 15 + Math.random() * 20;
      mem.groomEnd = t + 2.2;
    }
    const grooming = (lively && t < (mem.groomEnd ?? 0)) || c.action === 2;
    mem.groom = (mem.groom ?? 0) + ((grooming ? 1 : 0) - (mem.groom ?? 0)) * Math.min(1, c.dt * 6);
    const g = mem.groom;
    for (const [paw, s] of [[PAW_L, 1], [PAW_R, -1]] as const) {
      b[paw].rotation.set(-1.15 * g + g * 0.2 * Math.sin(t * 16 + s), 0, -0.25 * g * s);
    }
    // cheeks puff out while eating and twitch when sniffing; the body breathes fast
    const puff = c.action === 1 ? 1.28 + 0.04 * Math.sin(t * 14) : 1;
    for (const cheek of [CHEEK_L, CHEEK_R]) b[cheek].scale.setScalar(puff + twitch * 0.025);
    b[HEAD].rotation.x += twitch * 0.025;
    const quick = 1 + 0.012 * Math.sin(t * 7.5) * (c.asleep ? 0.4 : 1);
    b[SPINE].scale.x *= quick;
    b[SPINE].scale.z *= quick;
    c.close = g * 0.9;
    c.fast = sniffing || g > 0.05;
  },
};
