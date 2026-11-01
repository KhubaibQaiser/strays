// Coat colours (§10.4): curated looks, or one picked base colour with the shade, highlight and
// outline derived from it so contrast stays readable.
export const LOOKS: Record<string, string>[] = [
  { coatBase: "#EFE1CC", coatShade: "#D9C4A6", coatHighlight: "#FFF8EE", outline: "#7A5C46" },
  { coatBase: "#34343A", coatShade: "#26262B", coatHighlight: "#55555E", outline: "#A9A9B6" },
  { coatBase: "#E8A15C", coatShade: "#CB8240", coatHighlight: "#FBE3C6", outline: "#7A4520" },
  { coatBase: "#B9C0CC", coatShade: "#9AA2B0", coatHighlight: "#E3E7EE", outline: "#4E5563" },
];

function toHsl(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => v / 255);
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  const h = max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return [h * 60, s, l];
}

function fromHsl(h: number, s: number, l: number): string {
  const k = (n: number) => (n + h / 30) % 12;
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
  return "#" + [f(0), f(8), f(4)].map((v) => Math.round(v * 255).toString(16).padStart(2, "0")).join("").toUpperCase();
}

const clamp = (v: number) => Math.min(0.97, Math.max(0.03, v));

/** Base → shade (darker), highlight (lighter), outline (dark on light coats, light on dark coats). */
export function deriveLook(base: string): Record<string, string> {
  const [h, s, l] = toHsl(base);
  return {
    coatBase: base.toUpperCase(),
    coatShade: fromHsl(h, s, clamp(l - 0.1)),
    coatHighlight: fromHsl(h, s, clamp(l + 0.18)),
    outline: l > 0.45 ? fromHsl(h, Math.min(1, s * 0.6), clamp(l - 0.5)) : fromHsl(h, s * 0.3, clamp(l + 0.42)),
  };
}

export function hexToArgb(hex: string): number {
  return (0xff000000 | parseInt(hex.slice(1), 16)) >>> 0;
}
