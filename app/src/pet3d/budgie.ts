// The 3D budgie (the app's "parrot"): an original budgerigar perched on a branch, built from
// distance-field primitives, 8 bones. Soft layered feathers instead of fur, black-scalloped yellow wings
// and a barred crown and nape, a yellow face with black throat spots and violet cheek patches, a blue
// cere over a hooked horn beak, dark eyes with a white ring, a long blue tail and grey-pink feet.
// Birds move in quick snaps, not slow sways: head turns and tilts jump and hold, and it bobs when happy.
import * as THREE from "three";
import { build, type Mesh, type Prim, type V3 } from "./sdf";
import { both, cone, ell, mat, Pattern, type SpeciesDef } from "./species";

const HIPS = 0, SPINE = 1, NECK = 2, HEAD = 3, WING_L = 4, WING_R = 5, TAIL = 6;
/** The bird is modelled standing 0.12 higher; this sets it down on the branch. */
const at = (p: V3): V3 => [p[0], p[1] - 0.12, p[2]];
const TAIL_PTS: V3[] = [[0, 0.1, -0.07], [0, 0.045, -0.16], [0, 0.015, -0.27]];
const PIVOT: V3[] = [at([0, 0.3, -0.02]), at([0, 0.45, 0]), at([0, 0.58, 0.02]), at([0, 0.66, 0.04]), at([0.115, 0.58, 0]), at([-0.115, 0.58, 0]), TAIL_PTS[0], TAIL_PTS[1]];
const PARENT = [-1, HIPS, SPINE, NECK, SPINE, SPINE, HIPS, TAIL];

const BODY = mat({ fur: 1 }), HEAD_F = mat({ fur: 0.7, accent: 1 }), FACE = mat({ fur: 0.6, accent: 1 }), BARS = mat({ fur: 0.8, accent: 1, barred: 1 });
const WING = mat({ fur: 0.9, accent: 1, barred: 1 }), SPOT = mat({ fur: 0.3, dark: 0.75 }), CHEEK = mat({ fur: 0.3, accent2: 1 });
const CERE = mat({ accent2: 1 }), BEAK = mat({ pink: 1 }), FEET = mat({ fur: 0.05, pink: 0.8, dark: 0.15 }), TAIL_F = mat({ fur: 0.4, accent2: 1, dark: 0.35 });
const SOCKET = mat({ fur: 0.03, dark: 0.4 });

function prims(): Prim[] {
  return [
    // a plump upright body tapering to the vent
    ell(at([0, 0.42, 0]), [0.14, 0.21, 0.14], SPINE, BODY, 0),
    cone(at([0, 0.32, -0.02]), at([0, 0.22, -0.06]), 0.11, 0.06, HIPS, BODY, 0.06),
    // the head: a barred crown and nape, a yellow face with throat spots and violet cheek patches
    ell(at([0, 0.68, 0.035]), [0.105, 0.1, 0.105], HEAD, HEAD_F, 0.06),
    ell(at([0, 0.69, -0.015]), [0.095, 0.085, 0.075], HEAD, BARS, 0.04),
    ell(at([0, 0.645, 0.09]), [0.085, 0.085, 0.06], HEAD, FACE, 0.05),
    ...both((s) => [
      ell(at([0.022 * s, 0.598, 0.128]), [0.008, 0.008, 0.006], HEAD, SPOT, 0.004),
      ell(at([0.05 * s, 0.602, 0.118]), [0.008, 0.008, 0.006], HEAD, SPOT, 0.004),
      ell(at([0.074 * s, 0.612, 0.098]), [0.008, 0.008, 0.006], HEAD, SPOT, 0.004),
      ell(at([0.084 * s, 0.628, 0.075]), [0.016, 0.022, 0.012], HEAD, CHEEK, 0.008),
    ]),
    // a blue cere over a short hooked beak
    ell(at([0, 0.68, 0.13]), [0.03, 0.016, 0.015], HEAD, CERE, 0.008),
    cone(at([0, 0.666, 0.132]), at([0, 0.612, 0.162]), 0.029, 0.007, HEAD, BEAK, 0.006),
    ...both((s) => [ell(at([0.07 * s, 0.695, 0.08]), [0.024, 0.024, 0.02], HEAD, SOCKET, 0.008, true)]),
    // folded wings along the sides, scalloped black on yellow
    ...both((s) => [cone(at([0.1 * s, 0.57, -0.03]), at([0.065 * s, 0.27, -0.13]), 0.055, 0.02, s > 0 ? WING_L : WING_R, WING, 0.03)]),
    // a long tapering tail, and feet gripping the branch
    cone(TAIL_PTS[0], TAIL_PTS[1], 0.04, 0.026, TAIL, TAIL_F, 0.03, 1, false, 0.25),
    cone(TAIL_PTS[1], TAIL_PTS[2], 0.026, 0.01, TAIL + 1, TAIL_F, 0.02, 1, false, 0.75),
    ...both((s) => [
      cone(at([0.04 * s, 0.25, 0.02]), at([0.045 * s, 0.205, 0.045]), 0.018, 0.013, HIPS, FEET, 0.015),
      ell(at([0.045 * s, 0.198, 0.06]), [0.022, 0.011, 0.016], HIPS, FEET, 0.008),
    ]),
  ];
}

let shape: Mesh | undefined;

export const budgie: SpeciesDef = {
  shape: () => (shape ??= build(prims(), [-0.25, -0.03, -0.33], [0.25, 0.75, 0.24], 0.0085)),
  pivot: PIVOT,
  parent: PARENT,
  hips: HIPS,
  spine: SPINE,
  neck: NECK,
  head: HEAD,
  tail: { bones: [TAIL, TAIL + 1], pts: TAIL_PTS, style: "bob" },
  eyes: { at: at([0.07, 0.695, 0.07]), r: 0.021, turn: 0.95, slit: 1, pupil: [0.62, 0.66, 0.58], lids: [1.2, 0.1, 1.1, 0.1, 0] },
  nose: at([0, 0.68, 0.13]),
  bare: [{ at: at([0, 0.65, 0.145]), r: 0.055 }],
  fur: { shells: 8, len: 0.014, density: 120, gravity: [0, -0.5, -0.7], coat: "feather" },
  pattern: Pattern.bars,
  headY: 0,
  camera: { pos: [0, 0.46, 2.2], look: [0, 0.36, 0], turn: -0.5 },
  shadow: { at: [0, 0.001, -0.02], size: [0.75, 0.55] },
  bounds: { centre: [0, 0.35, -0.05], r: 0.55 },
  palette: (c) => ({
    base: c.coatBase ?? "#7dc44e",
    shade: c.coatShade ?? "#5ea233",
    light: c.coatHighlight ?? "#a6de7b",
    pink: "#d8bd94",
    dark: "#151515",
    accent: "#f2d649",
    accent2: "#5a6ad6",
    iris: "#f1ece2",
    irisOuter: "#d8d1c4",
    lid: "#f2d649",
    stripes: 0,
    tipping: 0,
  }),
  // a branch to sit on
  extras: (root, _bones, _palette, keep) => {
    const geo = new THREE.CylinderGeometry(0.038, 0.042, 0.62, 14);
    const m = new THREE.MeshStandardMaterial({ color: 0x7a5a3e, roughness: 0.92 });
    const branch = new THREE.Mesh(geo, m);
    branch.rotation.z = Math.PI / 2;
    branch.position.set(0, 0.04, 0.035);
    root.add(branch);
    keep(geo);
    keep(m);
  },
  animate: (c) => {
    const { bones: b, mem, t, dt } = c;
    const lively = !c.still && !c.asleep;
    // the head jumps to a new angle and holds it, often with a tilt
    if (lively && t > (mem.snapAt ?? 0)) {
      mem.snapAt = t + 0.5 + Math.random() * 1.8;
      mem.yaw = (Math.random() - 0.5) * 1.3;
      mem.tilt = Math.random() < 0.5 ? (Math.random() - 0.5) * 0.7 : 0;
    }
    if (!lively) mem.yaw = mem.tilt = 0;
    const k = Math.min(1, dt * 22);
    mem.hy = (mem.hy ?? 0) + ((mem.yaw ?? 0) - (mem.hy ?? 0)) * k;
    mem.hz = (mem.hz ?? 0) + ((mem.tilt ?? 0) - (mem.hz ?? 0)) * k;
    // happy budgies bob their heads quickly
    const bob = lively && c.joyful ? Math.max(0, Math.sin(t * 2.2)) * Math.sin(t * 26) : 0;
    b[HEAD].rotation.y = mem.hy;
    b[HEAD].rotation.z = mem.hz;
    b[HEAD].rotation.x += bob * 0.12;
    b[NECK].rotation.y = 0;
    // celebrating, it flaps; waking, it stretches a wing; otherwise the wings rest with a faint rustle
    const flap = c.action === 3 && c.at < 1.8 ? Math.sin(c.at * 30) * 0.5 + 0.5 : c.action === 8 && c.at < 1.5 ? 0.6 : 0;
    b[WING_L].rotation.set(0, 0, flap * 1.1 + (lively ? 0.01 * Math.sin(t * 3) : 0));
    b[WING_R].rotation.set(0, 0, -flap * 1.1 - (lively ? 0.01 * Math.sin(t * 3) : 0));
    // asleep, it fluffs up
    b[SPINE].scale.multiplyScalar(1 + c.sleepy * 0.06);
    c.fast = Math.abs((mem.yaw ?? 0) - mem.hy) + Math.abs((mem.tilt ?? 0) - mem.hz) > 0.02 || bob !== 0 || flap > 0;
  },
};
