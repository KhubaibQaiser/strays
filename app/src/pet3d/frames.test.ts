import { describe, expect, it } from "vitest";
import { cheaper, due, idleFor, rateFor } from "./frames";

const still = { fast: false, acting: false, asleep: false, software: false };

describe("frame rate", () => {
  it("follows the liveliness and what the pet is doing", () => {
    expect(rateFor("calm", still)).toBe(20);
    expect(rateFor("calm", { ...still, fast: true })).toBe(30);
    expect(rateFor("gentle", still)).toBe(30);
    expect(rateFor("gentle", { ...still, acting: true })).toBe(60);
    expect(rateFor("lively", still)).toBe(60);
    expect(rateFor("gentle", { ...still, asleep: true })).toBe(10);
    expect(rateFor("gentle", { ...still, software: true, fast: true })).toBe(8);
    expect(rateFor("gentle", still, 2)).toBe(15);
  });

  it("lands on whole display frames at 60 and 120 Hz", () => {
    const drawn = (hz: number, rate: number) => {
      let last = 0, n = 0;
      for (let f = 1; f <= hz; f++) {
        const now = (f * 1000) / hz;
        if (due(now, last, rate)) (last = now), n++;
      }
      return n;
    };
    for (const hz of [60, 120]) {
      expect(drawn(hz, 60)).toBe(60);
      expect(drawn(hz, 30)).toBe(30);
      expect(drawn(hz, 20)).toBe(20);
      expect(drawn(hz, 10)).toBe(10);
    }
  });

  it("sleeps between frames only when they're far apart", () => {
    expect(idleFor(60)).toBe(0);
    expect(idleFor(10)).toBe(76);
    expect(idleFor(10, 50)).toBe(26);
    expect(idleFor(10, 90)).toBe(0);
  });

  it("gives up coat layers, then sharpness, then frames", () => {
    let q = { shells: 28, dpr: 2, slow: 1 };
    const steps = [];
    for (let next = cheaper(q, 6); next; next = cheaper(q, 6)) steps.push((q = next));
    expect(steps).toEqual([
      { shells: 14, dpr: 2, slow: 1 },
      { shells: 7, dpr: 2, slow: 1 },
      { shells: 6, dpr: 2, slow: 1 },
      { shells: 6, dpr: 1, slow: 1 },
      { shells: 6, dpr: 1, slow: 2 },
      { shells: 6, dpr: 1, slow: 4 },
    ]);
  });
});
