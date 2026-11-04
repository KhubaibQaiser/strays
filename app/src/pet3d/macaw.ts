// The 3D macaw (the app's "parrot"): an original macaw on a stand perch, built from distance-field
// primitives, 8 bones. Sleek layered feathers instead of fur; a long slim body in the coat colour with a chest in the
// highlight colour (yellow for a blue-and-gold); folded wings that run from the coat colour through a
// yellow band to blue flight feathers; a very long tail ending in blue; a small head with a bare white
// face patch round small pale eyes; a deep, narrow, hooked ivory beak over a black lower beak; dark grey
// feet. Birds move in quick snaps, not slow sways: head turns and tilts jump and hold, and it bobs when happy.
import * as THREE from "three";
import { build, type Mesh, type Prim, type V3 } from "./sdf";
import { both, cone, ell, mat, Pattern, type SpeciesDef } from "./species";

const HIPS = 0, SPINE = 1, NECK = 2, HEAD = 3, WING_L = 4, WING_R = 5, TAIL = 6;
const TAIL_PTS: V3[] = [[0, 0.32, -0.07], [0, 0.17, -0.12], [0, 0.02, -0.15]];
const PIVOT: V3[] = [[0, 0.36, -0.02], [0, 0.48, 0], [0, 0.59, 0.01], [0, 0.69, 0.035], [0.068, 0.6, -0.01], [-0.068, 0.6, -0.01], TAIL_PTS[0], TAIL_PTS[1]];
const PARENT = [-1, HIPS, SPINE, NECK, SPINE, SPINE, HIPS, TAIL];
/** the perch's branch height; the feet grip it */
const PERCH_Y = 0.3;

// a light `barred` weight shades each feather's edge, so the plumage shows overlapping rows
const BODY = mat({ fur: 1, barred: 0.16 }), CHEST = mat({ fur: 1, light: 1, barred: 0.16 }), HEAD_F = mat({ fur: 0.7, barred: 0.12 });
const FACE = mat({ fur: 0.08, white: 1, barred: 0.18 }), BEAK = mat({ pink: 1 }), LOWER_BEAK = mat({ dark: 1 });
const SHOULDER = mat({ fur: 0.9, barred: 0.35 }), BAND = mat({ fur: 1, accent: 1, barred: 0.4 }), FLIGHT = mat({ fur: 1, accent2: 1, barred: 0.4 });
const TAIL_F = mat({ fur: 0.5, barred: 0.3 }), TAIL_TIP = mat({ fur: 0.4, accent2: 1, barred: 0.3 }), FEET = mat({ fur: 0.03, dark: 0.55, pink: 0.25 });
const SOCKET = mat({ fur: 0.03, white: 1 });

/** The upper beak as a curved blade: its ridge arcs from the forehead down into the hook, its cutting edge
 *  runs underneath, and they meet at the tip. Each pair is joined, so the beak is solid between them. */
const RIDGE: V3[] = [[0, 0.712, 0.066], [0, 0.723, 0.098], [0, 0.716, 0.128], [0, 0.697, 0.153], [0, 0.668, 0.167], [0, 0.638, 0.162], [0, 0.616, 0.145]];
const EDGE: V3[] = [[0, 0.67, 0.08], [0, 0.668, 0.106], [0, 0.664, 0.13], [0, 0.656, 0.148], [0, 0.642, 0.156], [0, 0.627, 0.152], [0, 0.616, 0.145]];
const BEAK_R = [0.026, 0.024, 0.02, 0.016, 0.012, 0.008, 0.004];

function beak(): Prim[] {
  const out: Prim[] = [];
  RIDGE.forEach((top, i) => {
    // (at the tip the ridge and edge meet, and a zero-length cone would be undefined)
    if (i + 1 < RIDGE.length) out.push(cone(top, EDGE[i], BEAK_R[i], BEAK_R[i] * 0.8, HEAD, BEAK, 0.006, 1, false, undefined, 1.6));
    if (i + 1 < RIDGE.length) out.push(cone(top, RIDGE[i + 1], BEAK_R[i], BEAK_R[i + 1], HEAD, BEAK, 0.006, 1, false, undefined, 1.6));
  });
  // the lower beak: a chunky black scoop under the cutting edge, tipped up at the front
  out.push(cone([0, 0.664, 0.084], [0, 0.645, 0.112], 0.024, 0.018, HEAD, LOWER_BEAK, 0.008, 1, false, undefined, 1.25));
  out.push(cone([0, 0.645, 0.112], [0, 0.648, 0.134], 0.018, 0.009, HEAD, LOWER_BEAK, 0.006, 1, false, undefined, 1.25));
  return out;
}

function prims(): Prim[] {
  return [
    // a long, slim body with a chest in the highlight colour, tapering to the vent
    ell([0, 0.47, 0], [0.08, 0.19, 0.085], SPINE, BODY, 0),
    ell([0, 0.47, 0.04], [0.064, 0.15, 0.05], SPINE, CHEST, 0.05),
    cone([0, 0.37, -0.02], [0, 0.31, -0.07], 0.075, 0.045, HIPS, BODY, 0.05),
    // a small head on a neck, with a bare white face patch round small pale eyes
    cone([0, 0.56, 0.005], [0, 0.66, 0.03], 0.072, 0.058, NECK, HEAD_F, 0.07),
    ell([0, 0.7, 0.035], [0.068, 0.07, 0.075], HEAD, HEAD_F, 0.04),
    ...both((s) => [ell([0.046 * s, 0.693, 0.068], [0.022, 0.026, 0.028], HEAD, FACE, 0.012)]),
    ...both((s) => [ell([0.06 * s, 0.703, 0.062], [0.014, 0.014, 0.012], HEAD, SOCKET, 0.006, true)]),
    // the beak: a deep, narrow, hooked upper blade over a black lower scoop
    ...beak(),
    // long folded wings: coat-coloured shoulders, a yellow band, blue flight feathers
    ...both((s) => [
      cone([0.068 * s, 0.6, -0.01], [0.066 * s, 0.5, -0.045], 0.042, 0.04, s > 0 ? WING_L : WING_R, SHOULDER, 0.03),
      cone([0.066 * s, 0.5, -0.045], [0.058 * s, 0.4, -0.078], 0.04, 0.032, s > 0 ? WING_L : WING_R, BAND, 0.025),
      cone([0.058 * s, 0.4, -0.078], [0.04 * s, 0.25, -0.115], 0.032, 0.012, s > 0 ? WING_L : WING_R, FLIGHT, 0.025),
    ]),
    // a long tail, coat-coloured then blue at the tip
    cone(TAIL_PTS[0], TAIL_PTS[1], 0.036, 0.024, TAIL, TAIL_F, 0.03, 1, false, 0.25),
    cone(TAIL_PTS[1], TAIL_PTS[2], 0.024, 0.008, TAIL + 1, TAIL_TIP, 0.02, 1, false, 0.75),
    // dark grey feet gripping the branch
    ...both((s) => [
      cone([0.035 * s, 0.35, 0.015], [0.038 * s, PERCH_Y + 0.022, 0.04], 0.016, 0.012, HIPS, FEET, 0.012),
      ell([0.038 * s, PERCH_Y + 0.018, 0.058], [0.022, 0.011, 0.018], HIPS, FEET, 0.006),
    ]),
  ];
}

let shape: Mesh | undefined;

export const macaw: SpeciesDef = {
  shape: () => (shape ??= build(prims(), [-0.18, -0.03, -0.22], [0.18, 0.8, 0.22], 0.0075)),
  pivot: PIVOT,
  wear: { top: [0, 0.772, 0.03], r: 0.075, neck: [0, 0.6, 0.02], neckR: 0.095, chest: [0, 0.47, 0.11], badge: 0.03 },
  parent: PARENT,
  hips: HIPS,
  spine: SPINE,
  neck: NECK,
  head: HEAD,
  tail: { bones: [TAIL, TAIL + 1], pts: TAIL_PTS, style: "bob" },
  eyes: { at: [0.058, 0.703, 0.054], r: 0.012, turn: 1.0, slit: 1, pupil: [0.4, 0.46, 0.34], lids: [1.2, 0.1, 1.1, 0.1, 0] },
  nose: [0, 0.69, 0.1],
  bare: [{ at: [0, 0.69, 0.1], r: 0.045 }],
  fur: { shells: 5, len: 0.008, density: 110, gravity: [0, -0.5, -0.7], coat: "feather" },
  pattern: Pattern.bars,
  headY: 0,
  camera: { pos: [0, 0.46, 2.4], look: [0, 0.43, 0], turn: -0.95 },
  shadow: { at: [0.05, 0.001, 0], size: [0.7, 0.5] },
  bounds: { centre: [0, 0.4, -0.02], r: 0.55 },
  palette: (c) => ({
    base: c.coatBase ?? "#d2232a",
    shade: c.coatShade ?? "#a3161c",
    light: c.coatHighlight ?? "#e9503f",
    pink: "#e8d9b8",
    white: "#f6f4ef",
    dark: "#1b1b1d",
    accent: "#f2c12e",
    accent2: "#1f5fc0",
    iris: "#f0e6b0",
    irisOuter: "#d6c78e",
    lid: "#f6f4ef",
    stripes: 0,
    tipping: 0,
  }),
  // a wooden stand perch: a branch across, on a post with a foot
  extras: (root, _bones, _palette, keep) => {
    const m = new THREE.MeshStandardMaterial({ color: 0x7a5a3e, roughness: 0.92 });
    const parts: [THREE.BufferGeometry, [number, number, number], number][] = [
      [new THREE.CylinderGeometry(0.03, 0.034, 0.56, 14), [0, PERCH_Y, 0.03], Math.PI / 2],
      [new THREE.CylinderGeometry(0.024, 0.028, PERCH_Y, 12), [0.22, PERCH_Y / 2, 0.03], 0],
      [new THREE.CylinderGeometry(0.08, 0.09, 0.02, 20), [0.22, 0.01, 0.03], 0],
    ];
    for (const [geo, at, rz] of parts) {
      const mesh = new THREE.Mesh(geo, m);
      mesh.position.set(...at);
      mesh.rotation.z = rz;
      root.add(mesh);
      keep(geo);
    }
    keep(m);
  },
  animate: (c) => {
    const { bones: b, mem, t, dt } = c;
    const lively = !c.still && !c.asleep;
    // the head jumps to a new angle and holds it, often with a tilt
    if (lively && t > (mem.snapAt ?? 0)) {
      mem.snapAt = t + 0.6 + Math.random() * 2;
      mem.yaw = (Math.random() - 0.5) * 1.2;
      mem.tilt = Math.random() < 0.5 ? (Math.random() - 0.5) * 0.6 : 0;
    }
    if (!lively) mem.yaw = mem.tilt = 0;
    const k = Math.min(1, dt * 20);
    mem.hy = (mem.hy ?? 0) + ((mem.yaw ?? 0) - (mem.hy ?? 0)) * k;
    mem.hz = (mem.hz ?? 0) + ((mem.tilt ?? 0) - (mem.hz ?? 0)) * k;
    // happy parrots bob their heads
    const bob = lively && c.joyful ? Math.max(0, Math.sin(t * 2)) * Math.sin(t * 20) : 0;
    b[HEAD].rotation.y = mem.hy;
    b[HEAD].rotation.z = mem.hz;
    b[HEAD].rotation.x += bob * 0.1;
    b[NECK].rotation.y = 0;
    // celebrating, it flaps; waking, it stretches its wings; otherwise they rest with a faint rustle
    const flap = c.action === 3 && c.at < 1.8 ? Math.sin(c.at * 24) * 0.5 + 0.5 : c.action === 8 && c.at < 1.5 ? 0.6 : 0;
    const rustle = lively ? 0.01 * Math.sin(t * 3) : 0;
    b[WING_L].rotation.set(0, 0, flap * 1.0 + rustle);
    b[WING_R].rotation.set(0, 0, -flap * 1.0 - rustle);
    // asleep, it fluffs up
    b[SPINE].scale.multiplyScalar(1 + c.sleepy * 0.05);
    c.fast = Math.abs((mem.yaw ?? 0) - mem.hy) + Math.abs((mem.tilt ?? 0) - mem.hz) > 0.02 || bob !== 0 || flap > 0;
  },
};
