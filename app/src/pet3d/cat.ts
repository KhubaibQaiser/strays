// A realistic 3D Persian cat for the prototype: an original sitting cat modelled from distance-field primitives,
// rigged with 16 bones, furred with shells (one instanced draw), glossy eyes with slit pupils, eyelids and
// whiskers. All motion is procedural and spring-driven, so it never snaps.
import * as THREE from "three";
import { RoomEnvironment } from "three/examples/jsm/environments/RoomEnvironment.js";
import { catShape, EAR_L, EAR_R, EYE, EYE_R, HEAD, NOSE_AT, HIPS, NECK, PARENT, PIVOT, SPINE, TAIL, TAIL_PTS, whiskerRoot } from "./shape";

export { catShape };

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
float fbm(vec3 p) { return 0.5 * vnoise(p) + 0.25 * vnoise(p * 2.03) + 0.125 * vnoise(p * 4.01); }
`;

/** `stripes` 0..1 is how strong the tabby bands are; `tipping` 0..1 darkens the guard hairs' tips (chinchilla, smoke). */
export type Look = { base: string; shade: string; light: string; pink: string; iris: string; stripes: number; tipping: number };
export type Quality = { shells: number; furLen: number; density: number };

function furMaterial(look: Look, q: Quality) {
  const u = {
    uFurLen: { value: q.furLen },
    uGravity: { value: new THREE.Vector3(0, -0.9, -0.2) },
    uDensity: { value: q.density },
    uBase: { value: new THREE.Color(look.base) },
    uShade: { value: new THREE.Color(look.shade) },
    uLight: { value: new THREE.Color(look.light) },
    uPink: { value: new THREE.Color(look.pink) },
    uEyeL: { value: new THREE.Vector3(...EYE) },
    uEyeR: { value: new THREE.Vector3(-EYE[0], EYE[1], EYE[2]) },
    uNose: { value: new THREE.Vector3(...NOSE_AT) },
    uStripes: { value: look.stripes },
    uTipping: { value: look.tipping },
  };
  const m = new THREE.MeshStandardMaterial({ roughness: 0.88, metalness: 0 });
  m.onBeforeCompile = (sh) => {
    Object.assign(sh.uniforms, u);
    sh.vertexShader = sh.vertexShader
      .replace(
        "#include <common>",
        `#include <common>
        attribute float aShell;
        attribute vec4 aMat;
        attribute vec4 aNoise;
        varying vec4 vNoise;
        uniform float uFurLen;
        uniform vec3 uGravity, uEyeL, uEyeR, uNose;
        varying vec3 vRest, vRestN;
        varying float vShell;
        varying vec4 vMat;`,
      )
      .replace(
        "#include <skinning_vertex>",
        `#include <skinning_vertex>
        vRest = position;
        vRestN = normal;
        vNoise = aNoise;
        vShell = aShell;
        vMat = aMat;
        // fur thins out round the eyes and nose, so the face stays open
        float clear = smoothstep(${(EYE_R * 1.05).toFixed(4)}, ${(EYE_R * 2.4).toFixed(4)}, min(distance(position, uEyeL), distance(position, uEyeR)));
        clear *= smoothstep(0.035, 0.08, distance(position, uNose));
        // the tail's fur shortens towards the tip, so it ends in a plume rather than a brush
        clear *= aMat.w > 0.0 ? mix(1.0, 0.5, aMat.w * aMat.w) : 1.0;
        float hgt = aShell * uFurLen * aMat.x * clear;
        transformed += normalize(objectNormal) * hgt + uGravity * hgt * aShell;`,
      );
    sh.fragmentShader = sh.fragmentShader
      .replace(
        "#include <common>",
        `#include <common>
        uniform vec3 uBase, uShade, uLight, uPink;
        uniform float uDensity, uStripes, uTipping;
        varying vec3 vRest, vRestN;
        varying float vShell;
        varying vec4 vMat, vNoise;
        ${NOISE}`,
      )
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
        // a triple coat: a dense lattice of short soft undercoat, and on a coarser lattice mid-length
        // awn hairs plus sparse long guard hairs; short fur (face, paws) gets finer strands
        vec3 q = vRest * uDensity * mix(2.2, 1.0, clamp(vMat.x, 0.0, 1.0));
        // neighbouring hairs bend the same way, so long fur falls in soft clumps instead of standing straight
        q += vNoise.xyz * 1.2 * vShell * vShell;
        vec3 rnd = hash3(floor(q));
        bool guard = rnd.y > 0.8;
        float tall = guard ? 0.9 + 0.1 * rnd.z : 0.55 + 0.25 * rnd.z;
        float r = length(fract(q) - (0.25 + 0.5 * rnd));
        bool hair = vShell <= tall && r <= (guard ? 0.32 : 0.45) * (1.0 - 0.7 * vShell / tall);
        // the undercoat never grows past 0.45 of the fur length, so higher layers skip it
        vec3 q2 = q * 2.0 + 17.0;
        vec3 rnd2 = vShell < 0.45 ? hash3(floor(q2)) : vec3(0.0);
        float tall2 = 0.25 + 0.2 * rnd2.z;
        bool under = vShell < 0.45 && vShell <= tall2 && length(fract(q2) - (0.25 + 0.5 * rnd2)) <= 0.55 * (1.0 - 0.5 * vShell / tall2);
        if (vShell > 0.0 && (vMat.x < 0.06 || !(hair || under))) discard;
        // mackerel tabby: bands across the body, rings on the tail, fading out on light fur
        float warp = vNoise.w;
        // bands run across the spine on the body, down the forehead on the head, round the tail
        float coord = mix(dot(vRest.yz, vec2(0.55, 0.83)) * 46.0, vRest.x * 95.0, smoothstep(0.66, 0.72, vRest.y));
        coord = vMat.w > 0.0 ? vMat.w * 17.0 : coord;
        float stripe = smoothstep(0.3, 0.7, sin(coord + warp * 4.0) * 0.5 + 0.5) * (1.0 - vMat.y) * uStripes;
        vec3 coat = mix(uBase, uShade, stripe);
        // darker along the back and top, as real coats are
        coat *= mix(1.0, 0.75, smoothstep(0.3, 0.95, vRestN.y) * (1.0 - vMat.y));
        coat = mix(coat, uLight, vMat.y * (0.75 + 0.25 * warp));
        coat = mix(coat, uPink, smoothstep(0.3, 0.65, vMat.z));
        coat *= 0.88 + 0.24 * (hair ? rnd.x : rnd2.x);
        // the undercoat is paler; guard hairs can carry dark tips
        if (!hair) coat = mix(coat, uLight, 0.25);
        else if (guard) coat = mix(coat, uShade, smoothstep(0.55, 1.0, vShell / tall) * uTipping);
        // darker near the skin, lighter at the tips; bare skin stays as it is
        float ao = mix(1.0, mix(0.4, 1.08, pow(vShell, 0.7)), smoothstep(0.0, 0.2, vMat.x));
        diffuseColor.rgb = coat * ao;`,
      );
  };
  return { m, u };
}

/** An eye with its lids painted on: an almond opening that slants up towards the outside, dark rims,
 *  and lids that meet in the middle as `uOpen` goes to 0. `side` is +1 for the pet's left eye. */
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

function eyeMaterial(iris: string, lid: THREE.Color, side: number) {
  const u = { uIris: { value: new THREE.Color(iris) }, uPupil: { value: 0.3 }, uOpen: { value: 1 }, uLid: { value: lid }, uSide: { value: side } };
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
        uniform vec3 uIris, uLid;
        uniform float uPupil, uOpen, uSide;
        varying vec3 vEyeN;
        ${NOISE}`,
      )
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
        vec3 n = normalize(vEyeN);
        float ang = acos(clamp(n.z, -1.0, 1.0));
        // iris: warm gold at the pupil, greener and darker outwards, radial fibres and a dark outer ring
        float fibres = 0.75 + 0.35 * vnoise(vec3(atan(n.y, n.x) * 14.0, ang * 9.0, 0.0));
        vec3 col = mix(uIris * 1.35, mix(uIris, vec3(0.35, 0.42, 0.14), 0.45) * 0.7, smoothstep(0.15, 1.0, ang)) * fibres;
        col *= 1.0 - smoothstep(0.9, 1.1, ang) * 0.8;
        float slit = length(vec2(n.x / uPupil, n.y / (uPupil * 1.35)));
        col *= smoothstep(1.0, 1.25, slit) * 0.92 + 0.08 * smoothstep(0.9, 1.0, slit); // soft dark rim round the pupil
        col = mix(vec3(0.012), col, smoothstep(0.92, 1.0, slit));
        // lids: almond opening, slanting up to the outer corner
        float x = n.x * uSide;
        float upper = mix(-0.05, 0.95, uOpen) - 0.25 * x * x + 0.04 * x;
        float lower = mix(-0.05, -0.85, uOpen) + 0.2 * x * x + 0.03 * x;
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
class Spring {
  x = 0;
  v = 0;
  target = 0;
  constructor(private w: number, private z: number) {}
  step(dt: number) {
    this.v += (this.w * this.w * (this.target - this.x) - 2 * this.z * this.w * this.v) * dt;
    this.x += this.v * dt;
  }
}

const rand = (lo: number, hi: number) => lo + Math.random() * (hi - lo);

/** The app's action numbers (the Rive rigs' `action` view-model value, art/STYLE.md). */
export const Action = { none: 0, eat: 1, petted: 2, celebrate: 3, exit: 4, enter: 5, fallAsleep: 6, sleep: 7, wake: 8, postcard: 9 } as const;
export type Action = (typeof Action)[keyof typeof Action];

export type Mood = "joyful" | "content" | "peckish" | "hungry" | "gloomy";

// ---- the pet ---------------------------------------------------------------------------------------

export class Cat3D {
  readonly renderer: THREE.WebGLRenderer;
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
  private pupil = Object.assign(new Spring(3, 1), { x: 0.32 });
  private blink = { start: -1, len: 0.16, next: rand(1, 4) };
  private next = { glance: rand(1, 3), ear: rand(3, 8), flick: rand(4, 10), tilt: rand(6, 14) };
  private sleepy = new Spring(1.6, 1);
  private scale = Object.assign(new Spring(7, 0.55), { x: 1, target: 1 });
  private turn = new Spring(2.5, 0.9);
  private root!: THREE.Group;
  private furGeo!: THREE.InstancedBufferGeometry;
  private action: Action = Action.none;
  private actionT0 = 0;
  mood: Mood = "content";
  /** Reduce Motion: only blinks and breathing, no glances, twitches or tail */
  still = false;
  /** where the user's cursor is, in this pet's view (-1..1), or null to let the cat look around */
  cursor: { x: number; y: number } | null = null;

  constructor(canvas: HTMLCanvasElement, look: Look, quality: Quality) {
    this.renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true, powerPreference: "low-power" });
    this.renderer.setClearColor(0, 0);
    // Neutral keeps the coat colours people pick; AgX washed light ginger out to cream
    this.renderer.toneMapping = THREE.NeutralToneMapping;
    this.renderer.toneMappingExposure = 1;

    const pmrem = new THREE.PMREMGenerator(this.renderer);
    const env = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
    pmrem.dispose();
    this.scene.environment = env;
    this.scene.environmentIntensity = 0.55;
    const key = new THREE.DirectionalLight(0xfff3e6, 2.4);
    key.position.set(-1.2, 2, 2.2);
    const rim = new THREE.DirectionalLight(0xd8e4ff, 1.6);
    rim.position.set(1.5, 1.2, -2);
    this.scene.add(key, rim);

    this.camera.position.set(0, 0.6, 2.6);
    this.camera.lookAt(0, 0.5, 0);

    const root = (this.root = new THREE.Group());
    root.rotation.y = -0.45;
    this.scene.add(root);

    // skeleton: bones sit at their pivots with no rest rotation, so bind space is mesh space
    this.bones = PIVOT.map(() => new THREE.Bone());
    this.bones.forEach((b, i) => {
      const p = PIVOT[i], parent = PARENT[i] >= 0 ? PIVOT[PARENT[i]] : [0, 0, 0];
      b.position.set(p[0] - parent[0], p[1] - parent[1], p[2] - parent[2]);
      if (PARENT[i] >= 0) this.bones[PARENT[i]].add(b);
    });
    for (let i = 0; i < 8; i++) {
      const a = new THREE.Vector3(...TAIL_PTS[i]), b = new THREE.Vector3(...TAIL_PTS[i + 1]);
      const dir = b.sub(a).setY(0).normalize();
      this.tailAxes.push(new THREE.Vector3().crossVectors(dir, this.up).normalize());
    }

    // the fur: the skin plus `shells` layers, drawn as instances of one skinned mesh
    const s = catShape();
    const geo = (this.furGeo = new THREE.InstancedBufferGeometry());
    geo.setAttribute("position", new THREE.BufferAttribute(s.position, 3));
    geo.setAttribute("normal", new THREE.BufferAttribute(s.normal, 3));
    geo.setAttribute("skinIndex", new THREE.Uint16BufferAttribute(s.skinIndex, 4));
    geo.setAttribute("skinWeight", new THREE.BufferAttribute(s.skinWeight, 4));
    geo.setAttribute("aMat", new THREE.BufferAttribute(s.mat, 4));
    geo.setAttribute("aNoise", new THREE.BufferAttribute(furNoise(s.position), 4));
    geo.setIndex(new THREE.BufferAttribute(s.index, 1));
    const layers = new Float32Array(quality.shells + 1).map((_, i) => i / quality.shells);
    geo.setAttribute("aShell", new THREE.InstancedBufferAttribute(layers, 1));
    geo.instanceCount = layers.length;
    geo.boundingSphere = new THREE.Sphere(new THREE.Vector3(0, 0.45, -0.1), 0.75);
    const fur = furMaterial(look, quality);
    const mesh = new THREE.SkinnedMesh(geo, fur.m);
    mesh.frustumCulled = false;
    mesh.add(this.bones[HIPS]);
    root.add(mesh);
    mesh.updateMatrixWorld(true);
    mesh.bind(new THREE.Skeleton(this.bones));
    this.disposables.push(geo, fur.m, env);

    // eyes ride on the head bone, set a little into their sockets
    const eyeGeo = new THREE.SphereGeometry(EYE_R, 32, 24);
    this.disposables.push(eyeGeo);
    const headBone = this.bones[HEAD];
    for (const side of [1, -1]) {
      const mat = eyeMaterial(look.iris, new THREE.Color(look.base).multiplyScalar(0.85), side);
      const eye = new THREE.Mesh(eyeGeo, mat.m);
      eye.position.set(EYE[0] * side - PIVOT[HEAD][0], EYE[1] - PIVOT[HEAD][1], EYE[2] - PIVOT[HEAD][2]);
      eye.rotation.y = 0.08 * side;
      headBone.add(eye);
      this.eyes.push(mat);
      this.disposables.push(mat.m);
    }

    // whiskers: gently drooping curves from the muzzle
    const pts: number[] = [];
    for (const side of [1, -1])
      for (let j = 0; j < 5; j++) {
        const root = whiskerRoot(j);
        const start = new THREE.Vector3(root[0] * side, root[1], root[2]);
        const len = 0.22 - Math.abs(j - 2) * 0.018;
        const end = start.clone().add(new THREE.Vector3(side * len, 0.03 - j * 0.022, -len * 0.3));
        const mid = start.clone().lerp(end, 0.5).add(new THREE.Vector3(0, 0.02, 0));
        const curve = new THREE.QuadraticBezierCurve3(start, mid, end).getPoints(8);
        for (let k = 0; k < 8; k++) {
          for (const p of [curve[k], curve[k + 1]]) pts.push(p.x - PIVOT[HEAD][0], p.y - PIVOT[HEAD][1], p.z - PIVOT[HEAD][2]);
        }
      }
    const wGeo = new THREE.BufferGeometry().setAttribute("position", new THREE.Float32BufferAttribute(pts, 3));
    const wMat = new THREE.LineBasicMaterial({ color: 0xf4efe6, transparent: true, opacity: 0.7 });
    headBone.add(new THREE.LineSegments(wGeo, wMat));
    this.disposables.push(wGeo, wMat);

    // a soft contact shadow under the cat
    const sTex = shadowTexture();
    const sGeo = new THREE.PlaneGeometry(1.0, 0.9);
    const sMat = new THREE.MeshBasicMaterial({ map: sTex, transparent: true, depthWrite: false });
    const shadow = new THREE.Mesh(sGeo, sMat);
    shadow.rotation.x = -Math.PI / 2;
    shadow.position.set(0.05, 0.001, -0.08);
    root.add(shadow);
    this.disposables.push(sTex, sGeo, sMat);
  }

  /** Changes how many fur layers are drawn (fewer is cheaper; the fur keeps its length). */
  setShells(shells: number) {
    const layers = new Float32Array(shells + 1).map((_, i) => i / shells);
    this.furGeo.setAttribute("aShell", new THREE.InstancedBufferAttribute(layers, 1));
    this.furGeo.instanceCount = layers.length;
  }

  setSize(px: number, dpr: number) {
    this.renderer.setPixelRatio(dpr);
    this.renderer.setSize(px, px, false);
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

  /** Advances the motion by `dt` seconds. */
  update(dt: number) {
    dt = Math.min(dt, 1 / 20);
    const t = (this.t += dt);
    const at = t - this.actionT0;
    const A = this.action;
    const gloomy = this.mood === "gloomy" || this.mood === "hungry";
    const joyful = this.mood === "joyful" || A === Action.celebrate || A === Action.postcard;
    const asleep = A === Action.fallAsleep || A === Action.sleep;
    const lively = !this.still && !asleep;

    // decisions: glances, ear twitches, tail flicks, head tilts, blinks
    if (lively && t > this.next.glance) {
      this.next.glance = t + rand(1.8, 5.5);
      this.yaw.target = rand(-0.55, 0.55);
      this.pitch.target = rand(-0.12, 0.18) - (gloomy ? 0.22 : 0);
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
      this.next.flick = t + rand(4, 12);
      this.flick.v += rand(4, 8) * (Math.random() < 0.5 ? -1 : 1);
    }
    if (lively && t > this.next.tilt) {
      this.next.tilt = t + rand(5, 13);
      this.roll.target = Math.random() < 0.4 ? rand(-0.22, 0.22) : 0;
    }
    if (!asleep && t > this.blink.next) {
      const slow = Math.random() < 0.15;
      this.blink = { start: t, len: slow ? 0.9 : 0.17, next: t + rand(2.5, 7) };
    }

    // actions steer the same springs, so every pose blends in and out
    let nod = 0, hop = 0, purr = 0;
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
    this.pupil.target = joyful ? 0.45 : gloomy ? 0.2 : 0.32;
    for (const s of [this.yaw, this.pitch, this.roll, ...this.ears, this.flick, this.pupil, this.sleepy, this.scale, this.turn]) s.step(dt);

    const b = this.bones;
    const sleepy = this.sleepy.x;
    this.root.scale.setScalar(Math.max(0.001, this.scale.x));
    this.root.rotation.y = -0.45 + this.turn.x;
    b[HIPS].position.y = PIVOT[HIPS][1] + hop;
    // breathing: slower and deeper asleep; the chest swells and the neck undoes the scale for the head
    const breath = Math.sin((t * Math.PI * 2) / (3.4 + sleepy * 1.6)) * (1 + sleepy * 0.6) + purr * 0.25 * Math.sin(t * 50);
    b[SPINE].scale.set(1 + 0.012 * breath, 1 + 0.006 * breath, 1 + 0.02 * breath);
    b[NECK].scale.set(1 / b[SPINE].scale.x, 1 / b[SPINE].scale.y, 1 / b[SPINE].scale.z);
    b[HIPS].rotation.z = this.still ? 0 : 0.012 * Math.sin(t * 0.55);
    b[NECK].rotation.set(-this.pitch.x * 0.35, this.yaw.x * 0.35, 0);
    b[HEAD].rotation.set(-this.pitch.x * 0.6 + nod, this.yaw.x * 0.65, this.roll.x);
    const earBase = (gloomy ? 0.45 : joyful ? -0.1 : 0) + sleepy * 0.3;
    b[EAR_L].rotation.set(-this.ears[0].x * 0.05 - earBase * 0.5, 0, this.ears[0].x * 0.03 + earBase * 0.6);
    b[EAR_R].rotation.set(-this.ears[1].x * 0.05 - earBase * 0.5, 0, -this.ears[1].x * 0.03 - earBase * 0.6);

    // tail: a slow wave down its length, a flick now and then, the tip lifting (high when happy)
    const speed = this.still ? 0 : (gloomy ? 0.5 : 1) * (1 - sleepy * 0.8);
    const tipUp = (joyful ? 1.6 : 1) * (1 - sleepy * 0.7) + (A === Action.celebrate ? 1.2 : 0);
    for (let i = 0; i < 8; i++) {
      const along = i / 7;
      const sway = (0.08 * Math.sin(t * 1.1 * speed - i * 0.55) * Math.min(1, speed * 2) + this.flick.x * 0.05) * (0.25 + 0.75 * along);
      const lift = i >= 5 ? (i - 4) * (0.1 + 0.08 * Math.sin(t * 0.6 * speed)) * tipUp : 0;
      this.q.setFromAxisAngle(this.up, sway);
      this.q2.setFromAxisAngle(this.tailAxes[i], lift + Math.abs(this.flick.x) * 0.04 * along);
      b[TAIL + i].quaternion.copy(this.q).multiply(this.q2);
    }

    // eyelids: a blink closes both lids, a slow blink holds them half-shut; sleep and petting close them
    let shut = 0;
    if (this.blink.start >= 0) {
      const p = (t - this.blink.start) / this.blink.len;
      if (p >= 1) this.blink.start = -1;
      else shut = Math.sin(p * Math.PI) ** (this.blink.len > 0.5 ? 0.6 : 1);
    }
    const close = Math.max(shut, gloomy ? 0.3 : 0, sleepy, purr * 0.85);
    for (const e of this.eyes) {
      e.u.uOpen.value = 1 - close;
      e.u.uPupil.value = this.pupil.x;
    }
  }

  /** True while something quick is moving (a blink, a twitch, a flick, the head turning), so the page
   *  can draw at a high frame rate then and a lower one for breathing and slow sway. */
  get fast() {
    const turning = Math.abs(this.yaw.v) + Math.abs(this.pitch.v) + Math.abs(this.roll.v) > 0.35;
    const twitching = this.ears.some((e) => Math.abs(e.v) > 1.5) || Math.abs(this.flick.v) > 0.6;
    return turning || twitching || this.blink.start >= 0;
  }

  render() {
    this.renderer.render(this.scene, this.camera);
  }

  dispose() {
    this.disposables.forEach((d) => d.dispose());
    this.renderer.dispose();
  }
}
