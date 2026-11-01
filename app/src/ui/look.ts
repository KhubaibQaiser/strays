// Coat colours (spec §10.4): curated looks, or any base colour with the shade, highlight and outline
// derived from it in OKLCH, so lightness steps look even across hues.
export const LOOKS: Record<string, string>[] = [
  { coatBase: "#EFE1CC", coatShade: "#D9C4A6", coatHighlight: "#FFF8EE", outline: "#7A5C46" },
  { coatBase: "#34343A", coatShade: "#26262B", coatHighlight: "#55555E", outline: "#A9A9B6" },
  { coatBase: "#E8A15C", coatShade: "#CB8240", coatHighlight: "#FBE3C6", outline: "#7A4520" },
  { coatBase: "#B9C0CC", coatShade: "#9AA2B0", coatHighlight: "#E3E7EE", outline: "#4E5563" },
];

type Lch = [number, number, number];

const toLinear = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const toGamma = (c: number) => (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055);

function rgb(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => v / 255) as [number, number, number];
}

function hex([r, g, b]: number[]): string {
  return "#" + [r, g, b].map((v) => Math.round(Math.min(1, Math.max(0, v)) * 255).toString(16).padStart(2, "0")).join("").toUpperCase();
}

/** sRGB hex → OKLCH (Björn Ottosson's OKLab). */
export function toOklch(h: string): Lch {
  const [r, g, b] = rgb(h).map(toLinear);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  const L = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s;
  const A = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
  const B = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  return [L, Math.hypot(A, B), Math.atan2(B, A)];
}

function oklchToLinear([L, C, H]: Lch): number[] {
  const A = C * Math.cos(H);
  const B = C * Math.sin(H);
  const l = (L + 0.3963377774 * A + 0.2158037573 * B) ** 3;
  const m = (L - 0.1055613458 * A - 0.0638541728 * B) ** 3;
  const s = (L - 0.0894841775 * A - 1.291485548 * B) ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
}

/** OKLCH → sRGB hex; out-of-gamut colours lose chroma until they fit, keeping lightness and hue. */
export function fromOklch([L, C, H]: Lch): string {
  const Lc = Math.min(1, Math.max(0, L));
  let c = C;
  for (let i = 0; i < 30; i++) {
    const lin = oklchToLinear([Lc, c, H]);
    if (lin.every((v) => v >= -1e-4 && v <= 1 + 1e-4)) return hex(lin.map(toGamma));
    c *= 0.85;
  }
  return hex(oklchToLinear([Lc, 0, H]).map(toGamma));
}

/** WCAG 2 contrast ratio between two sRGB hex colours. */
export function contrast(a: string, b: string): number {
  const lum = (h: string) => {
    const [r, g, bl] = rgb(h).map(toLinear);
    return 0.2126 * r + 0.7152 * g + 0.0722 * bl;
  };
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}

/**
 * Base → shade (L −0.12), highlight (L +0.10), outline (L −0.35). On dark coats the outline goes
 * lighter instead. Spec §10.4 flips below L 0.3; here the darker or lighter outline with more
 * contrast wins, and it's pushed further if needed to reach 3:1, because a fixed threshold fails
 * for coats just above 0.3.
 */
export function deriveLook(base: string): Record<string, string> {
  const [L, C, H] = toOklch(base);
  const outlineAt = (l: number) => fromOklch([l, Math.min(C, 0.08), H]);
  const candidates = [L - 0.35, L + 0.35, 0.18, 0.92, 0.05, 0.98].map((l) => outlineAt(Math.min(0.98, Math.max(0.05, l))));
  const firstGood = candidates.find((c) => contrast(c, base) >= 3);
  const outline = firstGood ?? candidates.reduce((best, c) => (contrast(c, base) > contrast(best, base) ? c : best));
  return {
    coatBase: base.toUpperCase(),
    coatShade: fromOklch([L - 0.12, C, H]),
    coatHighlight: fromOklch([L + 0.1, C, H]),
    outline,
  };
}

export function hexToArgb(hex: string): number {
  return (0xff000000 | parseInt(hex.slice(1), 16)) >>> 0;
}
