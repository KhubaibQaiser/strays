// The 3D pet engine: draws any species definition (species.ts) with shell fur or feathers, painted-lid
// eyes, whiskers and a soft shadow, and moves it procedurally with springs, so nothing ever snaps.
import * as THREE from "three";
import { accessory } from "./accessories";
import { Props } from "./props";
import { RoomEnvironment } from "three/examples/jsm/environments/RoomEnvironment.js";
import { mergeGeometries } from "three/examples/jsm/utils/BufferGeometryUtils.js";
import type { AnimCtx, Palette, Pose, SpeciesDef } from "./species";

// ---- shaders ---------------------------------------------------------------------------------------

const NOISE = /* glsl */ `
vec3 hash3(vec3 p) {
  p = fract(p * vec3(443.897, 441.423, 437.195));
  p += dot(p, p.yxz + 19.19);
  return fract((p.xxy + p.yxx) * p.zyx);
}
float vnoise(vec3 p) {
  vec3 i = floor(p), f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  return mix(mix(mix(hash3(i).x, hash3(i + vec3(1, 0, 0)).x, f.x), mix(hash3(i + vec3(0, 1, 0)).x, hash3(i + vec3(1, 1, 0)).x, f.x), f.y),
             mix(mix(hash3(i + vec3(0, 0, 1)).x, hash3(i + vec3(1, 0, 1)).x, f.x), mix(hash3(i + vec3(0, 1, 1)).x, hash3(i + vec3(1, 1, 1)).x, f.x), f.y), f.z);
}
`;

const col = (hex: string) => ({ value: new THREE.Color(hex) });

function furMaterial(def: SpeciesDef, p: Palette) {
  const e = def.eyes.at;
  const u = {
    uFurLen: { value: def.fur.len },
    uGravity: { value: new THREE.Vector3(...def.fur.gravity) },
    uDensity: { value: def.fur.density },
    uFeather: { value: def.fur.coat === "feather" ? 1 : 0 },
    uPattern: { value: def.pattern },
    uHeadY: { value: def.headY },
    uBase: col(p.base),
    uShade: col(p.shade),
    uLight: col(p.light),
    uPink: col(p.pink),
    uDark: col(p.dark),
    uAccent: col(p.accent),
    uAccent2: col(p.accent2),
    uWhite: col(p.white ?? "#ffffff"),
    uEyeL: { value: new THREE.Vector3(...e) },
    uEyeR: { value: new THREE.Vector3(-e[0], e[1], e[2]) },
    uEyeSize: { value: def.eyes.r },
    // up to two fur-free spots (xyz, radius); unused ones sit far away
    uBare: { value: [0, 1].map((i) => (def.bare[i] ? new THREE.Vector4(...def.bare[i].at, def.bare[i].r) : new THREE.Vector4(9, 9, 9, 0.01))) },
    uStripes: { value: p.stripes },
    uTipping: { value: p.tipping },
  };
  const m = new THREE.MeshStandardMaterial({ roughness: 0.88, metalness: 0 });
  m.onBeforeCompile = (sh) => {
    Object.assign(sh.uniforms, u);
    sh.vertexShader = sh.vertexShader
      .replace(
        "#include <common>",
        `#include <common>
        attribute float aShell;
        attribute vec4 aMat, aMat2, aNoise;
        attribute float aMat3;
        varying float vWhite;
        uniform float uFurLen, uEyeSize;
        uniform vec3 uGravity, uEyeL, uEyeR;
        uniform vec4 uBare[2];
        varying vec3 vRest, vRestN;
        varying float vShell, vFurK;
        varying vec4 vMat, vMat2, vNoise;
        varying float vFloorY;`,
      )
      .replace(
        "#include <skinning_vertex>",
        `#include <skinning_vertex>
        vRest = position;
        vRestN = normal;
        vNoise = aNoise;
        vShell = aShell;
        vMat = aMat;
        vMat2 = aMat2;
        vWhite = aMat3;
        // fur thins out round the eyes, nose and mouth, so the face stays open
        float clear = smoothstep(uEyeSize * 1.05, uEyeSize * 2.4, min(distance(position, uEyeL), distance(position, uEyeR)));
        for (int i = 0; i < 2; i++) clear *= smoothstep(uBare[i].w * 0.45, uBare[i].w, distance(position, uBare[i].xyz));
        // a tail's fur shortens towards the tip, so it ends in a plume rather than a brush
        clear *= aMat.w > 0.0 ? mix(1.0, 0.5, aMat.w * aMat.w) : 1.0;
        vFurK = aMat.x * clear;
        float hgt = aShell * uFurLen * vFurK;
        transformed += normalize(objectNormal) * hgt + uGravity * hgt * aShell;
        vFloorY = transformed.y;`,
      );
    sh.fragmentShader = sh.fragmentShader
      .replace(
        "#include <common>",
        `#include <common>
        uniform vec3 uBase, uShade, uLight, uPink, uDark, uAccent, uAccent2, uWhite;
        varying float vWhite;
        uniform float uDensity, uStripes, uTipping, uFeather, uPattern, uHeadY, uFurLen;
        varying vec3 vRest, vRestN;
        varying float vShell, vFurK;
        varying vec4 vMat, vMat2, vNoise;
        varying float vFloorY;
        ${NOISE}`,
      )
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
        // a pet lying down sinks into the floor; nothing below it is drawn
        if (vFloorY < -0.004) discard;
        // fur: a dense lattice of short soft undercoat, and on a coarser lattice mid-length awn hairs plus
        // sparse long guard hairs; short fur (face, paws) gets finer strands. Feathers: soft, wide,
        // overlapping tufts instead of strands.
        vec3 q = vRest * uDensity * mix(2.2, 1.0, clamp(vMat.x, 0.0, 1.0));
        // a fixed warp keeps the hair grid from lining up with the surface (which shows as rings or stripes);
        // neighbouring hairs bend the same way, so long fur falls in soft clumps instead of standing straight
        q += vNoise.xyz * (2.5 + 1.2 * vShell * vShell);
        // strands sit on a 2D grid in the plane the surface mostly faces, so they run straight out of it;
        // a 3D grid cut by a gently curved surface leaves ring-shaped contours
        vec3 an = abs(vRestN);
        vec2 g = an.z > an.x && an.z > an.y ? q.xy : (an.x > an.y ? q.yz : q.xz);
        float face = an.z > an.x && an.z > an.y ? 0.0 : (an.x > an.y ? 1.0 : 2.0);
        vec3 rnd = hash3(vec3(floor(g), face));
        bool guard = uFeather < 0.5 && rnd.y > 0.8;
        float tall = guard ? 0.9 + 0.1 * rnd.z : 0.55 + 0.25 * rnd.z;
        float r = length(fract(g) - (0.25 + 0.5 * rnd.xy));
        float width = uFeather > 0.5 ? 0.8 * (1.0 - 0.45 * vShell / tall) : (guard ? 0.32 : 0.45) * (1.0 - 0.7 * vShell / tall);
        bool hair = vShell <= tall && r <= width;
        // the undercoat never grows past 0.45 of the fur length, so higher layers skip it
        vec2 g2 = g * 2.0 + 17.0;
        vec3 rnd2 = vShell < 0.45 && uFeather < 0.5 ? hash3(vec3(floor(g2), face + 5.0)) : vec3(0.0);
        float tall2 = 0.25 + 0.2 * rnd2.z;
        bool under = uFeather < 0.5 && vShell < 0.45 && vShell <= tall2 && length(fract(g2) - (0.25 + 0.5 * rnd2.xy)) <= 0.55 * (1.0 - 0.5 * vShell / tall2);
        // hair shorter than about a pixel (cleared skin round the nose, mouth and eyes) would only speckle it
        if (vShell > 0.0 && (uFurLen * vFurK < 0.0015 || !(hair || under))) discard;
        float furry = smoothstep(0.02, 0.15, vFurK);
        float warp = vNoise.w;
        vec3 coat = uBase;
        if (uPattern < 0.5) {
          // mackerel tabby: bands across the spine on the body, down the forehead, round the tail
          float coord = mix(dot(vRest.yz, vec2(0.55, 0.83)) * 46.0, vRest.x * 95.0, smoothstep(uHeadY, uHeadY + 0.06, vRest.y));
          coord = vMat.w > 0.0 ? vMat.w * 17.0 : coord;
          coat = mix(uBase, uShade, smoothstep(0.3, 0.7, sin(coord + warp * 4.0) * 0.5 + 0.5) * (1.0 - vMat.y) * uStripes);
        }
        // darker along the back and top, as real coats are
        coat *= mix(1.0, 0.75, smoothstep(0.3, 0.95, vRestN.y) * (1.0 - vMat.y) * (1.0 - vMat2.y));
        coat = mix(coat, uLight, vMat.y * (0.75 + 0.25 * warp));
        coat = mix(coat, uAccent, smoothstep(0.25, 0.7, vMat2.y));
        coat = mix(coat, uAccent2, smoothstep(0.25, 0.7, vMat2.z));
        coat = mix(coat, uPink, smoothstep(0.25, 0.55, vMat.z));
        coat = mix(coat, uWhite, smoothstep(0.25, 0.55, vWhite));
        // a sharp edge keeps small dark marks crisp instead of bleeding into a band
        coat = mix(coat, uDark, smoothstep(0.3, 0.5, vMat2.x));
        if (uPattern > 0.5 && uPattern < 1.5 && vMat2.w > 0.02) {
          // feathers in overlapping rows: small rows where the feathers are short (a macaw's face lines),
          // large on the wings; light weights shade each feather's edge, heavy ones draw dark bars
          float rows = mix(120.0, 40.0, clamp(vMat.x * 1.4, 0.0, 1.0));
          vec2 fp = vec2(vRest.z * rows * 0.9 + vRest.x * rows * 0.3, vRest.y * rows);
          fp.x += floor(fp.y) * 0.5;
          vec2 cell = fract(fp) - vec2(0.5, 0.42);
          float scallop = length(cell * vec2(1.0, 1.5));
          vec3 edge = mix(coat * 0.7, uDark, smoothstep(0.45, 0.8, vMat2.w));
          coat = mix(coat, edge, (1.0 - smoothstep(0.26, 0.36, scallop)) * smoothstep(0.05, 0.4, vMat2.w));
        }
        // per-hair variation; the undercoat is paler; guard hairs can carry dark tips (bare skin stays even)
        coat *= 1.0 + furry * (0.24 * (hair ? rnd.x : rnd2.x) - 0.12);
        if (!hair) coat = mix(coat, uLight, 0.25 * furry);
        else if (guard) coat = mix(coat, uShade, smoothstep(0.55, 1.0, vShell / tall) * uTipping);
        // darker near the skin, lighter at the tips; bare skin stays as it is
        float ao = mix(1.0, mix(uFeather > 0.5 ? 0.7 : 0.4, 1.08, pow(vShell, 0.7)), smoothstep(0.0, 0.2, vMat.x));
        diffuseColor.rgb = coat * ao;`,
      )
      .replace(
        "#include <lights_physical_fragment>",
        // bare skin (noses, beaks, feet) is a little glossy; fur and feathers are not
        `#include <lights_physical_fragment>
        material.roughness = mix(material.roughness, 0.45, (1.0 - smoothstep(0.0, 0.15, vMat.x)) * max(vMat.z, max(vMat2.x, vMat2.z)));
        // feathers have a soft sheen that fur doesn't
        material.roughness = mix(material.roughness, 0.55, uFeather);`,
      );
  };
  return { m, u };
}

// Slow-changing noise per vertex, computed once here instead of for every pixel of every fur layer:
// xyz bends neighbouring hairs the same way (clumps), w warps the tabby bands.
function vnoise(x: number, y: number, z: number) {
  const h = (a: number, b: number, c: number) => {
    const s = Math.sin(a * 127.1 + b * 311.7 + c * 74.7) * 43758.5453;
    return s - Math.floor(s);
  };
  const ix = Math.floor(x), iy = Math.floor(y), iz = Math.floor(z);
  const s = (t: number) => t * t * (3 - 2 * t);
  const fx = s(x - ix), fy = s(y - iy), fz = s(z - iz);
  const l = (a: number, b: number, t: number) => a + (b - a) * t;
  return l(
    l(l(h(ix, iy, iz), h(ix + 1, iy, iz), fx), l(h(ix, iy + 1, iz), h(ix + 1, iy + 1, iz), fx), fy),
    l(l(h(ix, iy, iz + 1), h(ix + 1, iy, iz + 1), fx), l(h(ix, iy + 1, iz + 1), h(ix + 1, iy + 1, iz + 1), fx), fy),
    fz,
  );
}
const fbm3 = (x: number, y: number, z: number) => 0.5 * vnoise(x, y, z) + 0.25 * vnoise(x * 2.03, y * 2.03, z * 2.03) + 0.125 * vnoise(x * 4.01, y * 4.01, z * 4.01);

function furNoise(position: Float32Array) {
  const out = new Float32Array((position.length / 3) * 4);
  for (let i = 0; i < position.length / 3; i++) {
    const x = position[i * 3], y = position[i * 3 + 1], z = position[i * 3 + 2];
    out[i * 4] = fbm3(x * 7, y * 7, z * 7) - 0.45;
    out[i * 4 + 1] = fbm3(x * 7 + 5.3, y * 7 + 5.3, z * 7 + 5.3) - 0.45;
    out[i * 4 + 2] = fbm3(x * 7 + 9.1, y * 7 + 9.1, z * 7 + 9.1) - 0.45;
    out[i * 4 + 3] = fbm3(x * 9, y * 9, z * 9);
  }
  return out;
}

/** An eye with its lids painted on: an opening shaped by the species' `lids`, dark rims, and lids that
 *  meet in the middle as `uOpen` goes to 0. `side` is +1 for the pet's left eye. */
function eyeMaterial(def: SpeciesDef, iris: string, irisOuter: string, lid: THREE.Color, side: number) {
  const [uo, uc, lo, lc, slant] = def.eyes.lids;
  const u = {
    uIris: { value: new THREE.Color(iris) },
    uIrisOuter: { value: new THREE.Color(irisOuter) },
    uPupil: { value: def.eyes.pupil[0] },
    uSlit: { value: def.eyes.slit },
    uOpen: { value: 1 },
    uLid: { value: lid },
    uSide: { value: side },
    uLids: { value: new THREE.Vector4(uo, uc, lo, lc) },
    uSlant: { value: slant },
    // happy crescent eyes (the lower lid rises in the middle) and sad ones (the inner corners lift)
    uSmile: { value: 0 },
    uSad: { value: 0 },
  };
  const m = new THREE.MeshPhysicalMaterial({ roughness: 0.25, clearcoat: 1, clearcoatRoughness: 0.03 });
  m.onBeforeCompile = (sh) => {
    Object.assign(sh.uniforms, u);
    sh.vertexShader = sh.vertexShader
      .replace("#include <common>", "#include <common>\nvarying vec3 vEyeN;")
      .replace("#include <begin_vertex>", "#include <begin_vertex>\nvEyeN = normal;");
    sh.fragmentShader = sh.fragmentShader
      .replace(
        "#include <common>",
        `#include <common>
        uniform vec3 uIris, uIrisOuter, uLid;
        uniform vec4 uLids;
        uniform float uPupil, uSlit, uOpen, uSide, uSlant, uSmile, uSad;
        varying vec3 vEyeN;
        ${NOISE}`,
      )
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
        vec3 n = normalize(vEyeN);
        float ang = acos(clamp(n.z, -1.0, 1.0));
        // iris: brighter at the pupil, darker outwards, radial fibres and a dark outer ring
        float fibres = 0.75 + 0.35 * vnoise(vec3(atan(n.y, n.x) * 14.0, ang * 9.0, 0.0));
        vec3 col = mix(uIris * 1.35, uIrisOuter, smoothstep(0.15, 1.0, ang)) * fibres;
        col *= 1.0 - smoothstep(0.9, 1.1, ang) * 0.8;
        float slit = length(vec2(n.x / uPupil, n.y / (uPupil * uSlit)));
        col *= smoothstep(1.0, 1.25, slit) * 0.92 + 0.08 * smoothstep(0.9, 1.0, slit); // soft dark rim round the pupil
        col = mix(vec3(0.012), col, smoothstep(0.92, 1.0, slit));
        float x = n.x * uSide;
        float upper = mix(-0.05, uLids.x, uOpen) - uLids.y * x * x + uSlant * x - uSad * (0.2 + 0.35 * x);
        float lower = mix(-0.05, -uLids.z, uOpen) + uLids.w * x * x + uSlant * 0.75 * x + uSmile * uOpen * (0.48 - 1.3 * x * x);
        float edge = n.z < 0.0 ? -1.0 : min(upper - n.y, n.y - lower);
        col = mix(vec3(0.02), col, smoothstep(0.0, 0.07, edge));
        diffuseColor.rgb = edge < 0.0 ? uLid : col;`,
      )
      .replace(
        "#include <lights_physical_fragment>",
        `#include <lights_physical_fragment>
        if (edge < 0.0) { material.roughness = 0.9; material.clearcoat = 0.0; }`,
      );
  };
  return { m, u };
}

/** The drawn mouth: an inverted "Y" under the nose (a short line down, then a curve out to each side), as
 *  thin tubes that morph between neutral, smile and frown, plus a dark opening and a pink tongue that
 *  scale in to meow, eat or lick. Everything sits on the head bone. */
function buildMouth(mouth: NonNullable<SpeciesDef["mouth"]>, headAt: number[], headBone: THREE.Bone, keep: (d: { dispose(): void }) => void) {
  const { at, w, drop, color, r } = mouth;
  // the face curves back at the sides, so the ends of the mouth sit a little further in
  const p = (x: number, y: number) => new THREE.Vector3(at[0] + x - headAt[0], at[1] + y - headAt[1], at[2] - x * x * 4 - headAt[2]);
  const shape = (endY: number, ctrlY: number) =>
    mergeGeometries([
      new THREE.TubeGeometry(new THREE.LineCurve3(p(0, 0), p(0, -drop)), 2, r, 5),
      ...[1, -1].map((s) => new THREE.TubeGeometry(new THREE.QuadraticBezierCurve3(p(0, -drop), p(s * w * 0.5, ctrlY), p(s * w, endY)), 10, r, 5)),
    ])!;
  const neutral = shape(-drop * 0.75, -drop * 1.55);
  const smile = shape(-drop * 0.05, -drop * 1.9);
  const frown = shape(-drop * 1.7, -drop * 1.05);
  neutral.morphAttributes.position = [smile.getAttribute("position"), frown.getAttribute("position")];
  smile.dispose();
  frown.dispose();
  const lineMat = new THREE.MeshBasicMaterial({ color });
  const lines = new THREE.Mesh(neutral, lineMat);
  lines.updateMorphTargets();
  // the opening hangs from the bottom of the "Y"; the tongue sits in it and pokes out to lick
  const holeGeo = new THREE.SphereGeometry(w * 0.6, 16, 10);
  const holeMat = new THREE.MeshBasicMaterial({ color: 0x3a1c22 });
  const hole = new THREE.Mesh(holeGeo, holeMat);
  hole.position.copy(p(0, -drop * 1.25)).setZ(hole.position.z - w * 0.35);
  hole.scale.z = 0.4;
  const tongueGeo = new THREE.SphereGeometry(w * 0.42, 14, 10).scale(1, 0.7, 0.8);
  const tongueMat = new THREE.MeshStandardMaterial({ color: 0xe8848f, roughness: 0.35 });
  const tongue = new THREE.Mesh(tongueGeo, tongueMat);
  const tongueAt = new THREE.Group().add(tongue);
  tongueAt.position.copy(p(0, -drop * 1.3));
  headBone.add(lines, hole, tongueAt);
  [neutral, lineMat, holeGeo, holeMat, tongueGeo, tongueMat].forEach(keep);
  hole.visible = tongue.visible = false;
  return { lines, hole, tongue, at: tongueAt.position.clone() };
}

function shadowTexture() {
  const c = document.createElement("canvas");
  c.width = c.height = 64;
  const g = c.getContext("2d")!;
  const grad = g.createRadialGradient(32, 32, 0, 32, 32, 32);
  grad.addColorStop(0, "rgba(0,0,0,0.55)");
  grad.addColorStop(0.6, "rgba(0,0,0,0.25)");
  grad.addColorStop(1, "rgba(0,0,0,0)");
  g.fillStyle = grad;
  g.fillRect(0, 0, 64, 64);
  return new THREE.CanvasTexture(c);
}

// ---- motion ----------------------------------------------------------------------------------------

/** A damped spring: `x` follows `target` smoothly; kicking `v` makes a twitch that settles by itself. */
export class Spring {
  x = 0;
  v = 0;
  target = 0;
  constructor(private w: number, private z: number) {}
  step(dt: number) {
    this.v += (this.w * this.w * (this.target - this.x) - 2 * this.z * this.w * this.v) * dt;
    this.x += this.v * dt;
  }
}

export const rand = (lo: number, hi: number) => lo + Math.random() * (hi - lo);

/** The app's action numbers (the Rive rigs' `action` view-model value, art/STYLE.md). */
export const Action = { none: 0, eat: 1, petted: 2, celebrate: 3, exit: 4, enter: 5, fallAsleep: 6, sleep: 7, wake: 8, postcard: 9 } as const;
export type Action = (typeof Action)[keyof typeof Action];

export type Mood = "joyful" | "content" | "peckish" | "hungry" | "gloomy";
/** How low each mood sits (0 = content, 1 = gloomy): it lowers the head, ears, lids and tail by that much. */
const LOW: Record<Mood, number> = { joyful: 0, content: 0, peckish: 0.3, hungry: 0.6, gloomy: 1 };
/** Seconds between glances: a peckish pet looks around for food, a gloomy one barely looks up. */
const GLANCE: Record<Mood, [number, number]> = { joyful: [1.8, 5.5], content: [1.8, 5.5], peckish: [1, 3], hungry: [1.4, 3.8], gloomy: [4, 9] };

// ---- the pet ---------------------------------------------------------------------------------------

export class Pet3D {
  readonly renderer: THREE.WebGLRenderer;
  /** the software renderer's name when there's no GPU, else null */
  readonly software: string | null;
  private scene = new THREE.Scene();
  readonly camera = new THREE.PerspectiveCamera(27, 1, 0.1, 10);
  private bones: THREE.Bone[] = [];
  private eyes: ReturnType<typeof eyeMaterial>[] = [];
  private disposables: { dispose(): void }[] = [];
  private tailAxes: THREE.Vector3[] = [];
  private q = new THREE.Quaternion();
  private q2 = new THREE.Quaternion();
  private up = new THREE.Vector3(0, 1, 0);

  // motion state
  private t = Math.random() * 100;
  private yaw = new Spring(4.5, 0.85);
  private pitch = new Spring(4.5, 0.85);
  private roll = new Spring(3, 0.8);
  private ears = [new Spring(22, 0.3), new Spring(22, 0.3)];
  private flick = new Spring(7, 0.35);
  private pupil: Spring;
  private blink = { start: -1, len: 0.16, next: rand(1, 4) };
  private next = { glance: rand(1, 3), ear: rand(3, 8), flick: rand(4, 10), tilt: rand(6, 14) };
  private sleepy = new Spring(1.6, 1);
  private scale = Object.assign(new Spring(7, 0.55), { x: 1, target: 1 });
  private turn = new Spring(2.5, 0.9);
  // mood body language: the hungry crouch and the gloomy lie, happy and sad eyes, the mouth's curve
  // (-1 frown .. 1 smile) and opening, a lick of the lips and a paw tapping the bowl
  private crouch = new Spring(2.4, 1);
  private down = new Spring(2.0, 1);
  private eyeSmile = new Spring(5, 1);
  private eyeSad = new Spring(3, 1);
  private curve = new Spring(5, 1);
  private open = new Spring(16, 0.8);
  private lick = new Spring(18, 0.7);
  /** when each mood moment last started and is next due (a hop, a sigh, a meow, a lick, a tap, a heart) */
  private moment: Record<string, number> = {};
  /** bones the mood poses move, which start each frame at rest */
  private poseBones: number[] = [];
  private mouth: { lines: THREE.Mesh; hole: THREE.Object3D; tongue: THREE.Object3D; at: THREE.Vector3 } | null = null;
  private headPos = new THREE.Vector3();
  private root: THREE.Group;
  private furGeo: THREE.InstancedBufferGeometry;
  private action: Action = Action.none;
  private actionT0 = 0;
  private wagging = false;
  private mem: Record<string, number> = {};
  private ownFast = false;
  private ownClose = 0;
  /** why the graphics card refused to draw this pet, if it did */
  shaderError: string | null = null;
  /** the accessory being worn (0 none) and what it's made of */
  private worn = { n: 0, pieces: [] as THREE.Object3D[], parts: [] as { dispose(): void }[] };
  private props: Props;
  private propsFast = false;
  /** how many fur layers are drawn now */
  shells = 0;
  mood: Mood = "content";
  /** Reduce Motion: only blinks and breathing, no glances, twitches or tail */
  still = false;
  /** where the user's cursor is, in this pet's view (-1..1), or null to let the pet look around */
  cursor: { x: number; y: number } | null = null;

  constructor(
    canvas: HTMLCanvasElement,
    private def: SpeciesDef,
    private palette: Palette,
  ) {
    this.renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true, powerPreference: "low-power" });
    this.renderer.setClearColor(0, 0);
    // three.js only prints a shader that won't compile to the console, and the pet draws nothing
    this.renderer.debug.onShaderError = (gl, program) => {
      this.shaderError = gl.getProgramInfoLog(program) || "a shader didn't compile";
    };
    // Neutral keeps the coat colours people pick; AgX washed light ginger out to cream
    this.renderer.toneMapping = THREE.NeutralToneMapping;
    this.renderer.toneMappingExposure = 1;
    this.pupil = Object.assign(new Spring(3, 1), { x: def.eyes.pupil[0] });

    // without a GPU (software WebGL, e.g. in a VM) the reflection map and a full coat would starve the
    // whole page, so it gets a plain sky light and fewer coat layers instead
    const gl = this.renderer.getContext();
    const info = gl.getExtension("WEBGL_debug_renderer_info");
    const gpu = String(info ? gl.getParameter(info.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER));
    this.software = /swiftshader|llvmpipe|softpipe|software/i.test(gpu) ? gpu : null;
    let env: THREE.Texture | null = null;
    if (this.software) this.scene.add(new THREE.HemisphereLight(0xffffff, 0x8a7a6a, 1.4));
    else {
      const pmrem = new THREE.PMREMGenerator(this.renderer);
      env = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
      pmrem.dispose();
      this.scene.environment = env;
      this.scene.environmentIntensity = 0.55;
    }
    const key = new THREE.DirectionalLight(0xfff3e6, 2.4);
    key.position.set(-1.2, 2, 2.2);
    const rim = new THREE.DirectionalLight(0xd8e4ff, 1.6);
    rim.position.set(1.5, 1.2, -2);
    this.scene.add(key, rim);

    this.camera.position.set(...def.camera.pos);
    this.camera.lookAt(...def.camera.look);

    const root = (this.root = new THREE.Group());
    root.rotation.y = def.camera.turn;
    this.scene.add(root);

    // skeleton: bones sit at their pivots with no rest rotation, so bind space is mesh space
    this.bones = def.pivot.map(() => new THREE.Bone());
    this.bones.forEach((b, i) => {
      const p = def.pivot[i], parent = def.parent[i] >= 0 ? def.pivot[def.parent[i]] : [0, 0, 0];
      b.position.set(p[0] - parent[0], p[1] - parent[1], p[2] - parent[2]);
      if (def.parent[i] >= 0) this.bones[def.parent[i]].add(b);
    });
    const pts = def.tail?.pts ?? [];
    for (let i = 0; i + 1 < pts.length; i++) {
      const a = new THREE.Vector3(...pts[i]), b = new THREE.Vector3(...pts[i + 1]);
      const dir = b.sub(a).setY(0).normalize();
      this.tailAxes.push(new THREE.Vector3().crossVectors(dir, this.up).normalize());
    }

    // the coat: the skin plus `shells` layers, drawn as instances of one skinned mesh
    const s = def.shape();
    const geo = (this.furGeo = new THREE.InstancedBufferGeometry());
    geo.setAttribute("position", new THREE.BufferAttribute(s.position, 3));
    geo.setAttribute("normal", new THREE.BufferAttribute(s.normal, 3));
    geo.setAttribute("skinIndex", new THREE.Uint16BufferAttribute(s.skinIndex, 4));
    geo.setAttribute("skinWeight", new THREE.BufferAttribute(s.skinWeight, 4));
    geo.setAttribute("aMat", new THREE.BufferAttribute(s.mat, 4));
    geo.setAttribute("aMat2", new THREE.BufferAttribute(s.mat2, 4));
    geo.setAttribute("aMat3", new THREE.BufferAttribute(s.mat3, 1));
    geo.setAttribute("aNoise", new THREE.BufferAttribute(furNoise(s.position), 4));
    geo.setIndex(new THREE.BufferAttribute(s.index, 1));
    this.setShells(this.software ? Math.min(6, def.fur.shells) : def.fur.shells);
    geo.boundingSphere = new THREE.Sphere(new THREE.Vector3(...def.bounds.centre), def.bounds.r);
    const fur = furMaterial(def, palette);
    const mesh = new THREE.SkinnedMesh(geo, fur.m);
    mesh.frustumCulled = false;
    mesh.add(this.bones[def.hips]);
    root.add(mesh);
    mesh.updateMatrixWorld(true);
    mesh.bind(new THREE.Skeleton(this.bones));
    this.disposables.push(geo, fur.m);
    if (env) this.disposables.push(env);

    // eyes ride on the head bone, set a little into their sockets
    const headAt = def.pivot[def.head];
    const eyeGeo = new THREE.SphereGeometry(def.eyes.r, 32, 24);
    this.disposables.push(eyeGeo);
    const headBone = this.bones[def.head];
    for (const side of [1, -1]) {
      const m = eyeMaterial(def, palette.iris, palette.irisOuter, new THREE.Color(palette.lid).multiplyScalar(0.85), side);
      const eye = new THREE.Mesh(eyeGeo, m.m);
      eye.position.set(def.eyes.at[0] * side - headAt[0], def.eyes.at[1] - headAt[1], def.eyes.at[2] - headAt[2]);
      eye.rotation.y = def.eyes.turn * side;
      headBone.add(eye);
      this.eyes.push(m);
      this.disposables.push(m.m);
    }

    // whiskers: gently drooping curves from the muzzle
    if (def.whiskers) {
      const { roots, len, droop, color } = def.whiskers;
      const pts: number[] = [];
      for (const side of [1, -1])
        roots.forEach((r, j) => {
          const start = new THREE.Vector3(r[0] * side, r[1], r[2]);
          const l = len - Math.abs(j - (roots.length - 1) / 2) * len * 0.08;
          const end = start.clone().add(new THREE.Vector3(side * l, l * 0.14 - j * droop, -l * 0.3));
          const mid = start.clone().lerp(end, 0.5).add(new THREE.Vector3(0, l * 0.09, 0));
          const curve = new THREE.QuadraticBezierCurve3(start, mid, end).getPoints(8);
          for (let k = 0; k < 8; k++) for (const p of [curve[k], curve[k + 1]]) pts.push(p.x - headAt[0], p.y - headAt[1], p.z - headAt[2]);
        });
      const wGeo = new THREE.BufferGeometry().setAttribute("position", new THREE.Float32BufferAttribute(pts, 3));
      const wMat = new THREE.LineBasicMaterial({ color, transparent: true, opacity: 0.7 });
      headBone.add(new THREE.LineSegments(wGeo, wMat));
      this.disposables.push(wGeo, wMat);
    }

    if (def.mouth) this.mouth = buildMouth(def.mouth, headAt, headBone, (d) => this.disposables.push(d));
    const posed = new Set<number>();
    for (const p of [def.poses?.crouch, def.poses?.down]) for (const i of Object.keys({ ...p?.rot, ...p?.scale })) posed.add(+i);
    if (def.tapLeg !== undefined) posed.add(def.tapLeg);
    this.poseBones = [...posed];

    // a soft contact shadow underneath
    const sTex = shadowTexture();
    const sGeo = new THREE.PlaneGeometry(...def.shadow.size);
    const sMat = new THREE.MeshBasicMaterial({ map: sTex, transparent: true, depthWrite: false });
    const shadow = new THREE.Mesh(sGeo, sMat);
    shadow.rotation.x = -Math.PI / 2;
    shadow.position.set(...def.shadow.at);
    root.add(shadow);
    this.disposables.push(sTex, sGeo, sMat);

    def.extras?.(root, this.bones, palette, (d) => this.disposables.push(d));
    this.props = new Props(this.scene, def, (d) => this.disposables.push(d));
  }

  /** Changes how many fur layers are drawn (fewer is cheaper; the fur keeps its length). */
  setShells(shells: number) {
    const layers = new Float32Array(shells + 1).map((_, i) => i / shells);
    this.shells = shells;
    // ponytail: three.js keeps the old layer list's small GPU buffer until the pet is disposed; this
    // runs a few times at most (the slow-frame guard), so that's a few bytes
    if (this.furGeo.getAttribute("aShell")) this.furGeo.deleteAttribute("aShell");
    this.furGeo.setAttribute("aShell", new THREE.InstancedBufferAttribute(layers, 1));
    this.furGeo.instanceCount = layers.length;
  }

  setSize(px: number, dpr: number) {
    this.renderer.setPixelRatio(dpr);
    this.renderer.setSize(px, px, false);
  }

  /** Puts on graduation accessory `n` (1 cap … 5 flower crown; 0 takes it off). */
  wear(n: number) {
    if (n === this.worn.n) return;
    this.worn.pieces.forEach((p) => p.removeFromParent());
    this.worn.parts.forEach((d) => d.dispose());
    const parts: { dispose(): void }[] = [];
    const bone = { head: this.def.head, neck: this.def.neck, chest: this.def.spine };
    const coat = [this.palette.base, this.palette.light, this.palette.accent].map((c) => new THREE.Color(c));
    const pieces = accessory(n, this.def.wear, coat, (d) => parts.push(d)).map(({ on, at, obj }) => {
      const pivot = this.def.pivot[bone[on]];
      obj.position.add(new THREE.Vector3(at[0] - pivot[0], at[1] - pivot[1], at[2] - pivot[2]));
      this.bones[bone[on]].add(obj);
      return obj;
    });
    this.worn = { n, pieces, parts };
  }

  /** Starts one of the app's actions (art/STYLE.md numbering); `Action.none` returns to idling. */
  act(a: Action) {
    this.action = a;
    this.actionT0 = this.t;
    if (a === Action.enter) this.scale.x = 0;
    if (a === Action.wake) this.ears[0].v += 20;
  }

  /** True while an action is running, so the page keeps the frame rate up. */
  get acting() {
    return this.action !== Action.none && this.action !== Action.sleep;
  }

  /** True while asleep: only breathing moves. */
  get asleep() {
    return this.action === Action.fallAsleep || this.action === Action.sleep;
  }

  /** Advances the motion by `dt` seconds. */
  update(dt: number) {
    dt = Math.min(dt, 1 / 20);
    const def = this.def;
    const t = (this.t += dt);
    const at = t - this.actionT0;
    const A = this.action;
    const low = LOW[this.mood];
    // species hooks (a dog's panting, a parrot's bobbing) treat hungry and gloomy as down
    const gloomy = low >= 0.6;
    const joyful = this.mood === "joyful" || A === Action.celebrate || A === Action.postcard;
    const asleep = A === Action.fallAsleep || A === Action.sleep;
    const lively = !this.still && !asleep;

    // decisions: glances, ear twitches, tail flicks, head tilts, blinks
    const m = this.mood;
    const posing = A === Action.none;
    if (lively && t > this.next.glance) {
      this.next.glance = t + rand(...GLANCE[m]);
      // a peckish or hungry pet keeps looking at its empty bowl, down by its front paw
      const atBowl = Math.random() < (m === "hungry" ? 0.65 : m === "peckish" ? 0.6 : 0);
      // a happy pet mostly looks out at you, chin up
      const atYou = joyful && Math.random() < 0.7;
      this.yaw.target = atBowl ? rand(0.45, 0.7) : atYou ? rand(0.25, 0.5) : rand(-0.55, 0.55);
      this.pitch.target = atBowl ? -0.42 : atYou ? rand(0.05, 0.15) : rand(-0.12, 0.18) - low * 0.22;
    }
    if (lively && this.cursor) {
      this.yaw.target = THREE.MathUtils.clamp(this.cursor.x * 0.9, -0.8, 0.8);
      this.pitch.target = THREE.MathUtils.clamp(this.cursor.y * 0.5, -0.3, 0.35);
    }
    if (lively && t > this.next.ear) {
      this.next.ear = t + rand(3, 11);
      this.ears[Math.random() < 0.5 ? 0 : 1].v += rand(14, 24);
    }
    if (lively && t > this.next.flick) {
      this.next.flick = t + (joyful ? rand(1.2, 3) : rand(4, 12));
      this.flick.v += rand(4, 8) * (Math.random() < 0.5 ? -1 : 1);
    }
    if (lively && t > this.next.tilt) {
      // a happy pet tilts its head playfully, and often
      this.next.tilt = t + (joyful ? rand(2, 5) : rand(5, 13));
      this.roll.target = joyful ? rand(0.15, 0.3) * (Math.random() < 0.5 ? -1 : 1) : Math.random() < 0.4 ? rand(-0.22, 0.22) : 0;
    }
    if (!asleep && t > this.blink.next) {
      // a content pet gives slow, trusting blinks
      const slow = Math.random() < (m === "content" ? 0.4 : 0.15);
      this.blink = { start: t, len: slow ? 0.9 : 0.17, next: t + rand(2.5, 7) };
    }

    // mood moments, each on its own timer: a happy hop, a gloomy sigh, a hungry (silent) meow, a peckish
    // lick of the lips and tap at the bowl, and now and then a heart over a happy pet
    const k = this.moment;
    const due = (name: string, on: boolean, lo: number, hi: number) => {
      k[name + "Next"] ??= t + rand(lo * 0.3, hi * 0.6);
      if (on && t > k[name + "Next"]) {
        k[name + "Next"] = t + rand(lo, hi);
        k[name] = t;
      }
      return t - (k[name] ?? -99);
    };
    const moving = lively && posing;
    const hopAt = due("hop", moving && joyful, 2.5, 5);
    const sighAt = due("sigh", moving && m === "gloomy", 6, 10);
    const meowAt = due("meow", moving && m === "hungry", 4, 7);
    const lickAt = due("lick", moving && m === "peckish", 3, 6);
    const tapAt = due("tap", moving && m === "peckish", 4, 8);
    const heartAt = due("heart", moving && joyful, 10, 20);
    const pulse = (since: number, len: number) => (since >= 0 && since < len ? Math.sin((since / len) * Math.PI) : 0);
    const sigh = pulse(sighAt, 2.4);
    const meow = pulse(meowAt, 0.7);
    const tapping = tapAt >= 0 && tapAt < 0.9 ? Math.max(0, Math.sin((tapAt / 0.9) * Math.PI * 2)) : 0;

    // actions steer the same springs, so every pose blends in and out
    let nod = 0, hop = 0.035 * pulse(hopAt, 0.45), purr = 0;
    this.sleepy.target = asleep ? 1 : 0;
    this.scale.target = A === Action.exit ? 0 : 1;
    this.turn.target = A === Action.exit ? 1.2 : 0;
    if (A === Action.eat) {
      this.pitch.target = -0.5;
      this.yaw.target = 0;
      nod = 0.05 * Math.sin(at * 14) * Math.min(1, at); // chewing
    } else if (A === Action.petted) {
      this.pitch.target = 0.2;
      this.roll.target = 0.18;
      purr = Math.min(1, at * 2);
    } else if (A === Action.celebrate) {
      this.yaw.target = 0.25 * Math.sin(at * 3);
      this.pitch.target = 0.15;
      hop = at < 2.4 ? 0.06 * Math.max(0, Math.sin((at * Math.PI * 2) / 0.6)) ** 1.5 : 0;
    } else if (A === Action.wake) {
      this.pitch.target = at < 1 ? 0.35 : 0; // a stretch up, then back to looking around
    } else if (A === Action.postcard) {
      this.yaw.target = 0;
      this.pitch.target = 0.05;
      if (at > 0.8 && at < 1.0 && this.blink.start < 0) this.blink = { start: t, len: 0.9, next: t + 4 };
    }
    if (asleep) {
      this.pitch.target = -0.38;
      this.yaw.target = 0.1;
      this.roll.target = 0.15;
    }
    // the mood's pose and face; actions (eating, celebrating, sleeping) sit the pet up first
    this.crouch.target = posing ? (m === "hungry" ? 1 : m === "peckish" ? 0.35 : 0) : 0;
    this.down.target = posing && m === "gloomy" ? 1 : 0;
    this.eyeSmile.target = joyful && !asleep ? 1 : 0;
    this.eyeSad.target = asleep || !posing ? 0 : m === "gloomy" ? 1 : m === "hungry" ? 0.7 : 0;
    this.curve.target = joyful || A === Action.petted ? 1 : A !== Action.none ? 0.3 : m === "content" ? 0.4 : m === "peckish" ? 0 : m === "hungry" ? -0.6 : -1;
    this.open.target = A === Action.eat ? 0.3 + 0.25 * Math.sin(at * 14) : A === Action.celebrate ? 0.7 : Math.max(meow, joyful ? 0.25 : 0);
    this.lick.target = pulse(lickAt, 0.6);
    const [restPupil, joyPupil, sadPupil] = def.eyes.pupil;
    this.pupil.target = joyful ? joyPupil : gloomy ? sadPupil : restPupil;
    for (const s of [this.yaw, this.pitch, this.roll, ...this.ears, this.flick, this.pupil, this.sleepy, this.scale, this.turn,
      this.crouch, this.down, this.eyeSmile, this.eyeSad, this.curve, this.open, this.lick]) s.step(dt);

    const b = this.bones;
    for (const i of this.poseBones) {
      b[i].rotation.set(0, 0, 0);
      b[i].scale.set(1, 1, 1);
    }
    const sleepy = this.sleepy.x;
    const cr = this.crouch.x, dn = this.down.x;
    this.root.scale.setScalar(Math.max(0.001, this.scale.x));
    this.root.rotation.y = def.camera.turn + this.turn.x;
    b[def.hips].position.set(def.pivot[def.hips][0], def.pivot[def.hips][1] + hop, def.pivot[def.hips][2]);
    // breathing: slower and deeper asleep, a long heavy breath when it sighs; the chest swells and the neck
    // undoes the scale for the head
    const breath = Math.sin((t * Math.PI * 2) / (3.4 + sleepy * 1.6)) * (1 + sleepy * 0.6) + purr * 0.25 * Math.sin(t * 50) + sigh * 2.2;
    b[def.spine].scale.set(1 + 0.012 * breath, 1 + 0.006 * breath, 1 + 0.02 * breath);
    b[def.neck].scale.set(1 / b[def.spine].scale.x, 1 / b[def.spine].scale.y, 1 / b[def.spine].scale.z);
    b[def.hips].rotation.z = this.still ? 0 : 0.012 * Math.sin(t * 0.55);
    // a meow and a sigh lift the head a little
    const lift = meow * 0.14 + sigh * 0.12;
    b[def.neck].rotation.set(-this.pitch.x * 0.35, this.yaw.x * 0.35, 0);
    b[def.head].rotation.set(-this.pitch.x * 0.6 + nod - lift, this.yaw.x * 0.65, this.roll.x);
    if (def.ears) {
      const [l, r] = def.ears;
      // perked forward when happy; out to the side and then flat as the mood sinks
      const earBase = (joyful ? -0.25 : low * 0.45 + dn * 0.35) + sleepy * 0.3;
      b[l].rotation.set(-this.ears[0].x * 0.05 - earBase * 0.5, 0, this.ears[0].x * 0.03 + earBase * 0.6);
      b[r].rotation.set(-this.ears[1].x * 0.05 - earBase * 0.5, 0, -this.ears[1].x * 0.03 - earBase * 0.6);
    }
    if (def.tail) this.moveTail(def.tail, t, joyful, low, sleepy, A === Action.celebrate);

    if (def.animate) {
      const c: AnimCtx = { t, dt, bones: b, joyful, gloomy, asleep, still: this.still, action: A, at, sleepy, close: 0, mem: this.mem, fast: false, mood: m, crouch: cr, down: dn };
      def.animate(c);
      this.ownClose = c.close;
      this.ownFast = c.fast;
    }
    // the mood poses go on top of everything else
    if (def.poses)
      for (const [p, w] of [[def.poses.crouch, cr], [def.poses.down, dn]] as [Pose, number][]) {
        if (w < 0.001) continue;
        b[def.hips].position.y -= p.drop * w;
        b[def.hips].position.z += (p.fwd ?? 0) * w;
        for (const [i, r] of Object.entries(p.rot)) b[+i].rotation.set(b[+i].rotation.x + r![0] * w, b[+i].rotation.y + r![1] * w, b[+i].rotation.z + r![2] * w);
        for (const [i, sc] of Object.entries(p.scale ?? {})) b[+i].scale.multiply(new THREE.Vector3(1 + (sc![0] - 1) * w, 1 + (sc![1] - 1) * w, 1 + (sc![2] - 1) * w));
      }
    if (def.tapLeg !== undefined) b[def.tapLeg].rotation.x -= tapping * 0.75;

    // the mouth: smiling, flat or frowning, opening to meow, eat or cheer; the tongue licks round it
    if (this.mouth) {
      const { lines, hole, tongue } = this.mouth;
      const c = this.curve.x, o = Math.max(0, this.open.x), lk = Math.max(0, this.lick.x);
      lines.morphTargetInfluences![0] = Math.max(0, c);
      lines.morphTargetInfluences![1] = Math.max(0, -c);
      hole.visible = o > 0.03;
      hole.scale.set(0.7 + o * 0.3, Math.max(0.01, o), 1);
      tongue.visible = lk > 0.03 || o > 0.3;
      tongue.position.set(0, -0.006 - o * 0.006 - lk * 0.006, lk * 0.012);
      tongue.scale.setScalar(Math.max(0.01, Math.max(lk, o * 0.6)));
    }

    b[def.head].getWorldPosition(this.headPos);
    this.propsFast = this.props.update({
      t,
      dt,
      bowl: A === Action.eat ? "eating" : !asleep && (m === "peckish" || m === "hungry") ? "empty" : "none",
      eatAt: A === Action.eat ? at : 0,
      rain: !asleep && m === "gloomy",
      asleep: sleepy,
      confetti: A === Action.celebrate ? at : -1,
      postcard: A === Action.postcard ? at : -1,
      heart: heartAt < 1.6 && !this.still ? heartAt : -1,
      head: this.headPos,
    });

    let close = Math.max(low * 0.3, sleepy, purr * 0.85, this.ownClose);

    // eyelids: a blink closes both lids, a slow blink holds them half-shut
    let shut = 0;
    if (this.blink.start >= 0) {
      const p = (t - this.blink.start) / this.blink.len;
      if (p >= 1) this.blink.start = -1;
      else shut = Math.sin(p * Math.PI) ** (this.blink.len > 0.5 ? 0.6 : 1);
    }
    for (const e of this.eyes) {
      e.u.uOpen.value = 1 - Math.max(shut, close);
      e.u.uPupil.value = this.pupil.x;
      e.u.uSmile.value = this.eyeSmile.x;
      e.u.uSad.value = this.eyeSad.x;
    }
  }

  /** curl: a slow wave down the tail, the tip lifting off the floor; wag: side to side, faster when happy;
   *  bob: tail feathers rocking with the breath. */
  private moveTail(tail: NonNullable<SpeciesDef["tail"]>, t: number, joyful: boolean, low: number, sleepy: number, celebrating: boolean) {
    const b = this.bones;
    const n = tail.bones.length;
    if (tail.style === "wag") {
      const amp = this.still ? 0 : (joyful ? 0.55 : 0.22 - low * 0.16) * (1 - sleepy);
      const speed = joyful ? 24 : 9;
      this.wagging = amp > 0.3;
      tail.bones.forEach((bone, i) => b[bone].rotation.set(0, amp * Math.sin(t * speed - i * 0.5) * (0.5 + i / n), 0));
      return;
    }
    if (tail.style === "bob") {
      const bob = this.still ? 0 : 0.05 * Math.sin((t * Math.PI * 2) / 3.4) + this.flick.x * 0.03 + (celebrating ? 0.15 * Math.sin(t * 12) : 0);
      tail.bones.forEach((bone) => b[bone].rotation.set(bob, 0, 0));
      return;
    }
    const speed = this.still ? 0 : (1 - low * 0.5) * (1 - sleepy * 0.8);
    const tipUp = (joyful ? 1.6 : 1 - low * 0.6) * (1 - sleepy * 0.7) + (celebrating ? 1.2 : 0);
    for (let i = 0; i < n; i++) {
      const along = i / (n - 1);
      const sway = (0.08 * Math.sin(t * 1.1 * speed - i * 0.55) * Math.min(1, speed * 2) + this.flick.x * 0.05) * (0.25 + 0.75 * along);
      const lift = i >= n - 3 ? (i - (n - 4)) * (0.1 + 0.08 * Math.sin(t * 0.6 * speed)) * tipUp : 0;
      this.q.setFromAxisAngle(this.up, sway);
      this.q2.setFromAxisAngle(this.tailAxes[i], lift + Math.abs(this.flick.x) * 0.04 * along);
      b[tail.bones[i]].quaternion.copy(this.q).multiply(this.q2);
    }
  }

  /** True while something quick is moving (a blink, a twitch, a flick, a wag, the head turning), so the
   *  page can draw at a high frame rate then and a lower one for breathing and slow sway. */
  get fast() {
    const turning = Math.abs(this.yaw.v) + Math.abs(this.pitch.v) + Math.abs(this.roll.v) > 0.35;
    const twitching = this.ears.some((e) => Math.abs(e.v) > 1.5) || Math.abs(this.flick.v) > 0.6;
    return turning || twitching || this.wagging || this.ownFast || this.propsFast || this.blink.start >= 0;
  }

  render() {
    this.renderer.render(this.scene, this.camera);
  }

  dispose() {
    this.worn.parts.forEach((d) => d.dispose());
    this.disposables.forEach((d) => d.dispose());
    this.renderer.dispose();
    // give the context back now: WebKit only frees it on garbage collection, and caps how many live
    this.renderer.forceContextLoss();
  }
}
