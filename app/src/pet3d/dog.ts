// The 3D Pembroke Welsh Corgi: an original sitting corgi built from distance-field primitives, 11 bones.
// A low, wide body with a huge fluffy white bib and very short thick white legs; a broad round head with
// big wide-set ears, a ginger mask round dark almond eyes, a white blaze running into a white muzzle, a
// black nose, and a smile; the tongue comes out to pant when it's happy (and now and then otherwise) or
// for a quick lick; a small fluffy bobtail.
import * as THREE from "three";
import { build, type Mesh, type Prim, type V3 } from "./sdf";
import { both, cone, ell, mat, Pattern, type SpeciesDef } from "./species";

const HIPS = 0, SPINE = 1, NECK = 2, HEAD = 3, EAR_L = 4, EAR_R = 5, TAIL = 6, LEG_L = 8, LEG_R = 9, JAW = 10;
const TAIL_PTS: V3[] = [[0, 0.12, -0.37], [0.01, 0.15, -0.42], [0.02, 0.17, -0.45]];
/** The head is modelled around (0, 0.66, 0.1); this scales it up and seats it low on the shoulders. */
const HEAD_SCALE = 1.15;
const head = (p: V3): V3 => [p[0] * HEAD_SCALE, (p[1] - 0.66) * HEAD_SCALE + 0.6, (p[2] - 0.1) * HEAD_SCALE + 0.1];
const PIVOT: V3[] = [
  [0, 0.16, -0.14], [0, 0.3, -0.02], [0, 0.45, 0.06], head([0, 0.66, 0.1]), head([0.11, 0.79, 0.07]), head([-0.11, 0.79, 0.07]),
  TAIL_PTS[0], TAIL_PTS[1], [0.085, 0.32, 0.11], [-0.085, 0.32, 0.11], head([0, 0.625, 0.2]),
];
const PARENT = [-1, HIPS, SPINE, NECK, HEAD, HEAD, HIPS, TAIL, SPINE, SPINE, HEAD];

const FUR = mat({ fur: 1 }), PANTS = mat({ fur: 1.2 }), BIB = mat({ fur: 1.3, light: 1 }), SOCK = mat({ fur: 0.55, light: 1 });
const HEAD_FUR = mat({ fur: 0.5 }), JOWL = mat({ fur: 0.6, light: 1 }), MUZZLE = mat({ fur: 0.3, light: 1 }), BLAZE = mat({ fur: 0.4, light: 1 });
const NOSE = mat({ dark: 1 }), MOUTH = mat({ dark: 1 }), EAR = mat({ fur: 0.45 });
const INNER_EAR = mat({ fur: 0.7, light: 0.8 }), SOCKET = mat({ fur: 0.03, dark: 0.7 }), BOB = mat({ fur: 1.4 });

/** A head part: its centre goes through `head` and its size is scaled to match. */
const onHead = <T extends unknown[]>(make: (h: (p: V3) => V3, k: number) => T) => make(head, HEAD_SCALE);

function prims(): Prim[] {
  return [
    // a low wide loaf of a body: fluffy ginger haunches, and a huge white bib down the whole front
    ell([0, 0.17, -0.15], [0.27, 0.2, 0.26], HIPS, FUR, 0),
    ...both((s) => [ell([0.19 * s, 0.12, -0.06], [0.11, 0.12, 0.17], HIPS, PANTS, 0.06)]),
    ...both((s) => [ell([0.15 * s, 0.03, 0.07], [0.06, 0.034, 0.08], HIPS, SOCK, 0.04)]),
    cone([0, 0.22, -0.12], [0, 0.42, 0.06], 0.24, 0.2, SPINE, FUR, 0.08),
    ell([0, 0.36, 0.105], [0.2, 0.24, 0.15], SPINE, BIB, 0.13),
    cone([0, 0.42, 0.06], [0, 0.53, 0.1], 0.17, 0.15, NECK, FUR, 0.06),
    // very short, thick white front legs on big paws
    ...both((s) => [
      cone([0.085 * s, 0.3, 0.12], [0.085 * s, 0.06, 0.15], 0.062, 0.055, s > 0 ? LEG_L : LEG_R, SOCK, 0.05),
      ell([0.085 * s, 0.034, 0.175], [0.062, 0.036, 0.07], s > 0 ? LEG_L : LEG_R, SOCK, 0.03),
    ]),
    // a broad round head: a ginger mask round the eyes, white jowls, a white blaze into a white muzzle
    ...onHead((h, k) => {
      const e = (c: V3, r: V3, bone: number, m: number[], kk: number, sub = false) => ell(h(c), [r[0] * k, r[1] * k, r[2] * k], bone, m, kk * k, sub);
      const c = (a: V3, b: V3, r1: number, r2: number, bone: number, m: number[], kk: number, sq = 1, sub = false) =>
        cone(h(a), h(b), r1 * k, r2 * k, bone, m, kk * k, sq, sub);
      return [
        e([0, 0.71, 0.1], [0.16, 0.13, 0.13], HEAD, HEAD_FUR, 0.05),
        ...both((s) => [
          e([0.085 * s, 0.67, 0.13], [0.085, 0.072, 0.075], HEAD, HEAD_FUR, 0.05),
          e([0.06 * s, 0.632, 0.16], [0.066, 0.05, 0.062], HEAD, JOWL, 0.04),
        ]),
        e([0, 0.735, 0.205], [0.026, 0.07, 0.03], HEAD, BLAZE, 0.03),
        c([0, 0.665, 0.17], [0, 0.648, 0.255], 0.062, 0.04, HEAD, MUZZLE, 0.035),
        e([0, 0.664, 0.275], [0.032, 0.024, 0.02], HEAD, NOSE, 0.012),
        // a smile: a dark mouth line above a white chin (the tongue is its own mesh, see `extras`)
        e([0, 0.628, 0.24], [0.046, 0.012, 0.038], HEAD, MOUTH, 0.004, true),
        e([0, 0.6, 0.215], [0.045, 0.02, 0.045], JAW, MUZZLE, 0.02),
        ...both((s) => [e([0.06 * s, 0.716, 0.198], [0.03, 0.026, 0.024], HEAD, SOCKET, 0.01, true)]),
        // big triangular ears, wide-set and leaning out, rounded at the tips, with pale fluff inside
        ...both((s) => [
          c([0.1 * s, 0.77, 0.07], [0.18 * s, 0.97, 0.05], 0.088, 0.022, s > 0 ? EAR_L : EAR_R, EAR, 0.035, 2.4),
          c([0.103 * s, 0.787, 0.085], [0.173 * s, 0.953, 0.064], 0.066, 0.015, s > 0 ? EAR_L : EAR_R, INNER_EAR, 0.006, 2.4, true),
        ]),
      ];
    }),
    // a small fluffy bobtail
    cone(TAIL_PTS[0], TAIL_PTS[1], 0.055, 0.05, TAIL, BOB, 0.05, 1, false, 0.3),
    cone(TAIL_PTS[1], TAIL_PTS[2], 0.05, 0.04, TAIL + 1, BOB, 0.03, 1, false, 0.8),
  ];
}

let shape: Mesh | undefined;

export const dog: SpeciesDef = {
  shape: () => (shape ??= build(prims(), [-0.38, -0.03, -0.52], [0.38, 1.02, 0.38], 0.0105)),
  pivot: PIVOT,
  wear: { top: head([0, 0.83, 0.1]), r: 0.18, neck: [0, 0.46, 0.07], neckR: 0.25, chest: [0.04, 0.36, 0.335], badge: 0.05 },
  parent: PARENT,
  hips: HIPS,
  spine: SPINE,
  neck: NECK,
  head: HEAD,
  ears: [EAR_L, EAR_R],
  tail: { bones: [TAIL, TAIL + 1], pts: TAIL_PTS, style: "wag" },
  // hungry: hunched low; gloomy: belly down, front legs out, head flat between the paws
  poses: {
    crouch: { drop: 0.08, rot: { [SPINE]: [0.55, 0, 0], [NECK]: [-0.25, 0, 0], [HEAD]: [-0.2, 0, 0], [LEG_L]: [-0.55, 0, 0], [LEG_R]: [-0.55, 0, 0], [EAR_L]: [-0.3, 0, 0.35], [EAR_R]: [-0.3, 0, -0.35] } },
    down: { drop: 0.12, fwd: 0.02, rot: { [SPINE]: [1.15, 0, 0], [NECK]: [-0.35, 0, 0], [HEAD]: [-0.45, 0, 0], [LEG_L]: [-2.35, 0, 0], [LEG_R]: [-2.35, 0, 0], [TAIL]: [0.5, 0, 0], [EAR_L]: [-0.9, 0, 0.55], [EAR_R]: [-0.9, 0, -0.55] } },
  },
  tapLeg: LEG_L,
  eyes: { at: head([0.06, 0.716, 0.186]), r: 0.026 * HEAD_SCALE, turn: 0.15, slit: 1, pupil: [0.42, 0.48, 0.36], lids: [0.75, 0.35, 0.62, 0.25, 0.1] },
  nose: head([0, 0.664, 0.275]),
  bare: [{ at: head([0, 0.664, 0.275]), r: 0.055 }, { at: head([0, 0.626, 0.255]), r: 0.042 }],
  fur: { shells: 20, len: 0.04, density: 165, gravity: [0, -0.55, -0.25], coat: "triple" },
  pattern: Pattern.plain,
  headY: 0,
  camera: { pos: [0, 0.55, 2.6], look: [0, 0.46, 0], turn: -0.4 },
  shadow: { at: [0, 0.001, -0.08], size: [1.05, 1.0] },
  bounds: { centre: [0, 0.42, -0.08], r: 0.75 },
  palette: (c) => ({
    base: c.coatBase ?? "#d98a4a",
    shade: c.coatShade ?? "#b86c2e",
    light: "#f8f4ec",
    pink: "#e07888",
    dark: "#120e0d",
    accent: "#000000",
    accent2: "#000000",
    iris: "#5a3418",
    irisOuter: "#24140a",
    lid: c.coatBase ?? "#d98a4a",
    stripes: 0,
    tipping: 0.2,
  }),
  // a tongue: a soft glossy pink capsule on the jaw, hanging from the mouth; animate scales it in and out
  extras: (_root, bones, palette, keep) => {
    const geo = new THREE.CapsuleGeometry(0.022, 0.062, 6, 12);
    geo.translate(0, -0.042, 0); // hang from its top, so scaling pulls it up into the mouth
    const m = new THREE.MeshPhysicalMaterial({ color: palette.pink, roughness: 0.35, clearcoat: 0.4 });
    const tongue = new THREE.Mesh(geo, m);
    const jaw = PIVOT[JAW], at = head([0, 0.624, 0.272]);
    tongue.position.set(at[0] - jaw[0], at[1] - jaw[1], at[2] - jaw[2]);
    tongue.scale.set(1.2, 1, 0.42);
    tongue.rotation.x = 0.35;
    bones[JAW].add(tongue);
    bones[JAW].userData.tongue = tongue;
    keep(geo);
    keep(m);
  },
  // the tongue only comes out to pant (happy, or now and then when content) or for a quick lick; the
  // rest of the time the mouth is closed, and the tongue shrinks back into it
  animate: (c) => {
    const { bones: b, mem, t } = c;
    const lively = !c.still && !c.asleep;
    mem.nextPant ??= t + 6 + Math.random() * 10;
    if (lively && t > mem.nextPant) {
      mem.nextPant = t + 12 + Math.random() * 18;
      mem.pantEnd = t + 2 + Math.random() * 3;
      mem.lick = Math.random() < 0.4 ? 1 : 0;
    }
    const bout = lively && t < (mem.pantEnd ?? 0);
    const licking = bout && mem.lick === 1 && t > (mem.pantEnd ?? 0) - 0.45;
    // no panting while it crouches or lies sulking
    const panting = lively && c.down < 0.3 && c.crouch < 0.3 && (c.joyful || (bout && mem.lick !== 1));
    const want = panting ? 1 : licking ? 0.7 : 0;
    mem.out = (mem.out ?? 0) + (want - (mem.out ?? 0)) * Math.min(1, c.dt * 8);
    const out = mem.out;
    // the jaw drops to open the mouth (its dark inside shows in the gap), and the tongue hangs out of it
    b[JAW].rotation.x = c.asleep ? 0 : out * (0.2 + (panting ? 0.05 * Math.sin(t * 15) : 0));
    const tongue = b[JAW].userData.tongue as THREE.Object3D | undefined;
    if (tongue) {
      tongue.visible = out > 0.03;
      const s = Math.max(0.001, out);
      tongue.scale.set(1.2 * s, s, 0.42 * s);
    }
    c.fast ||= panting || licking || (out > 0.02 && out < 0.98);
  },
};
