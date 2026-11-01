// Recolouring rules (spec §10.4). Pure functions, so no page is loaded.
import { expect, test } from "@playwright/test";
import { LOOKS, contrast, deriveLook, fromOklch, toOklch } from "../src/ui/look";

test("OKLCH round-trips sRGB colours", () => {
  for (const h of ["#EFE1CC", "#34343A", "#E8A15C", "#000000", "#FFFFFF", "#3366CC"]) {
    expect(fromOklch(toOklch(h))).toBe(h);
  }
});

test("shade and highlight step lightness by the spec's amounts", () => {
  const base = "#B07040";
  const [L] = toOklch(base);
  const look = deriveLook(base);
  expect(toOklch(look.coatShade)[0]).toBeCloseTo(L - 0.12, 2);
  expect(toOklch(look.coatHighlight)[0]).toBeCloseTo(L + 0.1, 2);
});

test("outline contrasts 3:1 with the coat for every curated look", () => {
  for (const look of LOOKS) expect(contrast(look.outline, look.coatBase)).toBeGreaterThanOrEqual(3);
});

test("outline contrasts 3:1 with the coat for 500 random colours", () => {
  // fixed seed, so a failure can be reproduced
  let seed = 42;
  const rand = () => ((seed = (seed * 1103515245 + 12345) % 2 ** 31) / 2 ** 31);
  const failures: string[] = [];
  for (let i = 0; i < 500; i++) {
    const base = "#" + Math.floor(rand() * 0xffffff).toString(16).padStart(6, "0").toUpperCase();
    const look = deriveLook(base);
    if (contrast(look.outline, look.coatBase) < 3) failures.push(`${base} → ${look.outline}`);
  }
  expect(failures).toEqual([]);
});
