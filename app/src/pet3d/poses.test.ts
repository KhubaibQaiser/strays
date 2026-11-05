import { describe, expect, it } from "vitest";
import { SPECIES_3D } from "./species3d";

// The mood poses are hand-tuned numbers; these keep a typo from bending a bone inside out or sinking a pet
// through the floor.
describe("mood poses", () => {
  for (const [name, def] of Object.entries(SPECIES_3D)) {
    it(`${name}: crouch and lie stay within sane limits`, () => {
      expect(def.poses, `${name} has no poses`).toBeDefined();
      for (const pose of [def.poses!.crouch, def.poses!.down]) {
        expect(pose.drop).toBeGreaterThanOrEqual(0);
        // the hips never sink more than their own height
        expect(pose.drop).toBeLessThan(def.pivot[def.hips][1]);
        for (const [bone, r] of Object.entries(pose.rot)) {
          expect(+bone).toBeLessThan(def.pivot.length);
          for (const a of r!) expect(Math.abs(a)).toBeLessThanOrEqual(Math.PI);
        }
        for (const s of Object.values(pose.scale ?? {})) for (const k of s!) expect(k).toBeGreaterThan(0.5);
      }
      // lying down goes further than crouching
      expect(def.poses!.down.drop).toBeGreaterThanOrEqual(def.poses!.crouch.drop);
    });
  }
});
