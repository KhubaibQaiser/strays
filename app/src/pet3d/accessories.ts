// The five graduation accessories in 3D (spec §10.2, art/STYLE.md; colours from art/tools/pet.py), in
// the order of the rig's `accessory` number: 1 cap, 2 scarf, 3 bow, 4 star badge, 5 flower crown.
// Each is made of a few plain shapes sized to the species' head (`Wear`), and rides on its bones.
import * as THREE from "three";
import type { V3 } from "./sdf";

/** Where a species wears things, in model space: the top of its head (and the head's radius), the
 *  middle of its neck (and the neck's radius, fur included), and the front of its chest. */
export type Wear = { top: V3; r: number; neck: V3; neckR: number; chest: V3; badge: number };

const CAP = 0x2e2e3a, TASSEL = 0xf2c14e, SCARF = 0xd9534f, STRIPE = 0xf6f1e7, BOW = 0xe77a8f, STAR = 0xf2c14e, LEAF = 0x7bc47f;
const FLOWERS = [0xf29bb0, 0xffffff, 0xb9a2e0];

/** A piece of an accessory and the body part it rides on. */
export type Piece = { on: "head" | "neck" | "chest"; at: V3; obj: THREE.Object3D };

/** Its colour, or `other` when that's too close to one of the pet's own colours (a red scarf on a
 *  scarlet macaw, a gold star on a yellow chest) to be seen. */
function against(color: number, other: number, coat: THREE.Color[]): number {
  const c = new THREE.Color(color);
  const close = coat.some((k) => Math.hypot(k.r - c.r, k.g - c.g, k.b - c.b) < 0.3);
  return close ? other : color;
}

export function accessory(n: number, w: Wear, coat: THREE.Color[], keep: (d: { dispose(): void }) => void): Piece[] {
  const mat = (color: number, roughness = 0.75) => {
    const m = new THREE.MeshStandardMaterial({ color, roughness });
    keep(m);
    return m;
  };
  const mesh = (geo: THREE.BufferGeometry, m: THREE.Material, at: V3 = [0, 0, 0], rot: V3 = [0, 0, 0]) => {
    keep(geo);
    const o = new THREE.Mesh(geo, m);
    o.position.set(...at);
    o.rotation.set(...rot);
    return o;
  };
  const group = (...parts: THREE.Object3D[]) => new THREE.Group().add(...parts);
  const r = w.r;
  switch (n) {
    case 1: {
      // a mortarboard: a flat square board on a skull cap, a tassel hanging off one corner
      const cap = mat(CAP, 0.6), tassel = mat(TASSEL, 0.5);
      const board = mesh(new THREE.BoxGeometry(r * 1.9, r * 0.07, r * 1.9), cap, [0, r * 0.3, 0], [0, Math.PI / 4, 0]);
      const skull = mesh(new THREE.CylinderGeometry(r * 0.62, r * 0.7, r * 0.34, 24), cap, [0, r * 0.12, 0]);
      const button = mesh(new THREE.SphereGeometry(r * 0.08, 12, 8), tassel, [0, r * 0.36, 0]);
      const cord = mesh(new THREE.CylinderGeometry(r * 0.025, r * 0.025, r * 0.75, 6), tassel, [r * 0.62, r * 0.0, r * 0.2], [0.2, 0, 0.15]);
      const end = mesh(new THREE.SphereGeometry(r * 0.1, 12, 8).scale(1, 1.6, 1), tassel, [r * 0.68, -r * 0.38, r * 0.27]);
      return [{ on: "head", at: w.top, obj: group(board, skull, button, cord, end) }];
    }
    case 2: {
      // a red scarf round the neck with a pale stripe, one end hanging down the front
      const scarf = mat(against(SCARF, 0x5b8def, coat), 0.85), stripe = mat(STRIPE, 0.85);
      // tipped down at the front, the way a scarf settles on the chest
      const band = mesh(new THREE.TorusGeometry(w.neckR, w.neckR * 0.14, 12, 40), scarf, [0, 0, 0], [Math.PI / 2 + 0.3, 0, 0]);
      const line = mesh(new THREE.TorusGeometry(w.neckR * 1.01, w.neckR * 0.04, 6, 40), stripe, [0, 0, 0], [Math.PI / 2 + 0.3, 0, 0]);
      const end = mesh(new THREE.BoxGeometry(w.neckR * 0.4, w.neckR * 0.9, w.neckR * 0.1), scarf, [w.neckR * 0.35, -w.neckR * 0.75, w.neckR * 0.9], [0.3, 0, -0.15]);
      return [{ on: "neck", at: w.neck, obj: group(band, line, end) }];
    }
    case 3: {
      // a pink bow on one side of the head
      const bow = mat(against(BOW, 0xb9a2e0, coat), 0.55);
      const loop = (s: number) => mesh(new THREE.SphereGeometry(r * 0.26, 16, 12).scale(1.25, 0.8, 0.55), bow, [s * r * 0.27, 0, 0], [0, 0, s * 0.35]);
      const knot = mesh(new THREE.SphereGeometry(r * 0.12, 12, 10), bow);
      const obj = group(loop(1), loop(-1), knot);
      obj.position.set(r * 0.55, -r * 0.12, 0);
      obj.rotation.set(0, 0, -0.5);
      return [{ on: "head", at: w.top, obj: group(obj) }];
    }
    case 4: {
      // a gold star badge pinned to the chest
      const shape = new THREE.Shape();
      for (let i = 0; i < 10; i++) {
        const a = -Math.PI / 2 + (i * Math.PI) / 5, rr = i % 2 ? w.badge * 0.42 : w.badge;
        if (i) shape.lineTo(Math.cos(a) * rr, -Math.sin(a) * rr);
        else shape.moveTo(Math.cos(a) * rr, -Math.sin(a) * rr);
      }
      const geo = new THREE.ExtrudeGeometry(shape, { depth: w.badge * 0.18, bevelEnabled: true, bevelSize: w.badge * 0.06, bevelThickness: w.badge * 0.06, bevelSegments: 2 });
      return [{ on: "chest", at: w.chest, obj: group(mesh(geo, mat(against(STAR, 0xe2725b, coat), 0.35), [0, 0, 0], [-0.2, 0, 0])) }];
    }
    case 5: {
      // a crown of small flowers on a green vine, sitting round the top of the head
      const vine = mesh(new THREE.TorusGeometry(r * 0.72, r * 0.05, 8, 40), mat(LEAF), [0, 0, 0], [Math.PI / 2 - 0.15, 0, 0]);
      const flowers = Array.from({ length: 7 }, (_, i) => {
        const a = (i / 7) * Math.PI * 2;
        const f = mesh(new THREE.IcosahedronGeometry(r * 0.15, 1), mat(FLOWERS[i % FLOWERS.length], 0.6));
        f.position.set(Math.cos(a) * r * 0.72, Math.sin(a) * r * 0.72 * Math.sin(0.15), Math.sin(a) * r * 0.72 * Math.cos(0.15));
        return f;
      });
      const crown = group(vine, ...flowers);
      crown.position.y = -r * 0.08;
      return [{ on: "head", at: w.top, obj: group(crown) }];
    }
    default:
      return [];
  }
}
