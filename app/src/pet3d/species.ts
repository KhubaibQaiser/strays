// What a 3D species is: its shape (distance-field primitives on bones), its rig roles, eyes, coat and
// colours, how the camera frames it, and any motion of its own. The engine (pet.ts) does the rest.
import type * as THREE from "three";
import { ellipsoid, roundCone, type Mesh, type Prim, type V3 } from "./sdf";

/** Material slots for a primitive; anything left out is 0. */
export type Mat = { fur?: number; light?: number; pink?: number; dark?: number; accent?: number; accent2?: number; barred?: number; white?: number };
export const mat = (m: Mat): number[] => [m.fur ?? 0, m.light ?? 0, m.pink ?? 0, m.dark ?? 0, m.accent ?? 0, m.accent2 ?? 0, m.barred ?? 0, m.white ?? 0];

export function ell(c: V3, r: V3, bone: number, m: number[], k: number, sub = false): Prim {
  return { d: ellipsoid(c, r), c, r: Math.max(...r), bone, m, k, sub };
}

export function cone(a: V3, b: V3, r1: number, r2: number, bone: number, m: number[], k: number, squashZ = 1, sub = false, t?: number, squashX = 1): Prim {
  const c: V3 = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2];
  const half = Math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2]) / 2;
  return { d: roundCone(a, b, r1, r2, squashZ, squashX), c, r: half + Math.max(r1, r2), bone, m, k, sub, t };
}

/** The pet's left (+x) and right (-x) copies of a part. */
export const both = (make: (s: number) => Prim[]) => [...make(1), ...make(-1)];

/** The colours a pet is drawn with. `stripes` 0..1 is the tabby strength; `tipping` darkens guard-hair tips. */
export type Palette = {
  base: string;
  shade: string;
  light: string;
  pink: string;
  dark: string;
  accent: string;
  accent2: string;
  /** bare white skin, e.g. a macaw's face patch (unused species can leave any colour) */
  white?: string;
  iris: string;
  /** the iris's outer ring (cats' eyes go greener outwards, dogs' darker) */
  irisOuter: string;
  /** eyelid colour, painted round the eye */
  lid: string;
  stripes: number;
  tipping: number;
};

export type Coat = "triple" | "feather";
/** coat patterns: tabby bands, feather bars (scalloped wing coverts, face lines), or plain */
export const Pattern = { tabby: 0, bars: 1, plain: 2 } as const;

export type AnimCtx = {
  t: number;
  dt: number;
  bones: THREE.Bone[];
  joyful: boolean;
  gloomy: boolean;
  asleep: boolean;
  still: boolean;
  action: number;
  /** seconds since the action started */
  at: number;
  /** 0..1, how far asleep */
  sleepy: number;
  /** extra eye closing (0..1) the species wants, e.g. a hamster grooming */
  close: number;
  /** this pet's own scratch values, kept between frames (timers, phases) */
  mem: Record<string, number>;
  /** set true while the species' own motion is quick, so the page draws more frames */
  fast: boolean;
};

export type SpeciesDef = {
  shape: () => Mesh;
  pivot: V3[];
  parent: number[];
  hips: number;
  spine: number;
  neck: number;
  head: number;
  ears?: [number, number];
  /** tail bones with their rest points (one more point than bones), and how it moves */
  tail?: { bones: number[]; pts: V3[]; style: "curl" | "wag" | "bob" };
  eyes: {
    at: V3;
    r: number;
    turn: number;
    /** pupil height / width: 1 round, larger is taller */
    slit: number;
    /** pupil width at rest, joyful and gloomy */
    pupil: [number, number, number];
    /** lid opening: [upper open, upper curve, lower open, lower curve, slant] */
    lids: [number, number, number, number, number];
  };
  nose: V3;
  /** spots kept free of fur (the nose, a mouth): centre and the radius where fur is fully back */
  bare: { at: V3; r: number }[];
  whiskers?: { roots: V3[]; len: number; droop: number; color: number };
  fur: { shells: number; len: number; density: number; gravity: V3; coat: Coat };
  pattern: number;
  /** the tabby's head bands start above this height */
  headY: number;
  camera: { pos: V3; look: V3; turn: number };
  shadow: { at: V3; size: [number, number] };
  bounds: { centre: V3; r: number };
  palette: (colors: Record<string, string>) => Palette;
  /** where graduation accessories sit (accessories.ts) */
  wear: import("./accessories").Wear;
  /** extra meshes (a perch), added once */
  extras?: (root: THREE.Group, bones: THREE.Bone[], palette: Palette, keep: (d: { dispose(): void }) => void) => void;
  /** the species' own motion, run after the shared motion each frame */
  animate?: (c: AnimCtx) => void;
};
