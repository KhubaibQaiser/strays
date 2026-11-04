// The props that go with moods and moments (art/STYLE.md colours): a food bowl (empty when peckish
// or hungry, full while eating), a rain cloud over a gloomy pet, a bed and drifting Z's while it
// sleeps, confetti when it celebrates, and the postcard it brings back from holiday. They stand in
// the scene beside the pet, not on its bones, and fade in and out.
import * as THREE from "three";
import type { SpeciesDef } from "./species";

const BOWL = 0x5b8def, FOOD = 0xc98a4b, BED = 0xb9a2e0, CUSHION = 0xe9dff8, CLOUD = 0x9aa4b5, DROP = 0x6fa8dc;
const CARD = 0xfff6e0, STAMP = 0xe2725b, ZZZ = 0x8e8ea0;
const CONFETTI = [0xf2c14e, 0xe2725b, 0x5b8def, 0x7bc47f];

/** What the props show this frame. */
export type PropState = { t: number; dt: number; bowl: "none" | "empty" | "eating"; eatAt: number; rain: boolean; asleep: number; confetti: number; postcard: number };

type Fade = { obj: THREE.Object3D; mats: THREE.Material[]; o: number };

export class Props {
  private bowl: Fade;
  private food: THREE.Mesh;
  private cloud: Fade;
  private drops: THREE.Mesh[] = [];
  private bed: Fade;
  private zzz: Fade;
  private zs: THREE.Mesh[] = [];
  private confetti: Fade;
  private bits: { m: THREE.Mesh; v: THREE.Vector3; spin: THREE.Vector3 }[] = [];
  private card: Fade;
  private top: number;
  private hasBed: boolean;

  constructor(scene: THREE.Scene, def: SpeciesDef, keep: (d: { dispose(): void }) => void) {
    const mats: THREE.Material[] = [];
    const mat = (color: number, roughness = 0.7) => {
      const m = new THREE.MeshStandardMaterial({ color, roughness, transparent: true, opacity: 0 });
      keep(m);
      mats.push(m);
      return m;
    };
    const geo = <G extends THREE.BufferGeometry>(g: G) => (keep(g), g);
    const fade = (obj: THREE.Object3D, from: number): Fade => {
      scene.add(obj);
      obj.visible = false;
      return { obj, mats: mats.splice(from), o: 0 };
    };
    const r = def.bounds.r;
    this.top = def.wear.top[1];

    // a blue bowl on the floor beside the pet, with a heap of food that shrinks while it eats
    let from = mats.length;
    const bowlProfile = [[0.0, 0], [0.07, 0], [0.085, 0.005], [0.1, 0.045], [0.093, 0.05], [0.08, 0.012], [0, 0.012]].map(([x, y]) => new THREE.Vector2(x * r * 1.6, y * r * 1.6));
    const bowl = new THREE.Mesh(geo(new THREE.LatheGeometry(bowlProfile, 32)), mat(BOWL, 0.4));
    this.food = new THREE.Mesh(geo(new THREE.SphereGeometry(r * 0.13, 20, 10, 0, Math.PI * 2, 0, Math.PI / 2)), mat(FOOD, 0.9));
    this.food.scale.y = 0.45;
    this.food.position.y = r * 0.02;
    const bowlG = new THREE.Group().add(bowl, this.food);
    bowlG.position.set(r * 0.62, 0, r * 0.38);
    this.bowl = fade(bowlG, from);

    // a small grey cloud up beside the head, dripping slowly
    from = mats.length;
    const cloudG = new THREE.Group();
    const puff = mat(CLOUD, 0.95);
    for (const [x, y, s] of [[0, 0, 1], [-0.7, -0.15, 0.75], [0.7, -0.12, 0.8], [0.25, 0.35, 0.7], [-0.3, 0.3, 0.6]]) {
      const p = new THREE.Mesh(geo(new THREE.SphereGeometry(r * 0.095 * s, 16, 12)), puff);
      p.position.set(x * r * 0.11, y * r * 0.11, 0);
      cloudG.add(p);
    }
    const drop = mat(DROP, 0.3);
    for (let i = 0; i < 4; i++) {
      const d = new THREE.Mesh(geo(new THREE.SphereGeometry(r * 0.012, 8, 6).scale(1, 2, 1)), drop);
      d.position.x = (i - 1.5) * r * 0.045;
      this.drops.push(d);
      cloudG.add(d);
    }
    cloudG.position.set(r * 0.3, this.top + r * 0.02, 0.1);
    this.cloud = fade(cloudG, from);

    // a round lilac bed with a pale cushion, under the pet while it sleeps
    from = mats.length;
    const bedR = def.shadow.size[0] * 0.36;
    const rim = new THREE.Mesh(geo(new THREE.TorusGeometry(bedR, bedR * 0.11, 12, 40)), mat(BED, 0.9));
    rim.rotation.x = Math.PI / 2;
    rim.position.y = bedR * 0.1;
    const cushion = new THREE.Mesh(geo(new THREE.CylinderGeometry(bedR * 0.95, bedR, bedR * 0.08, 40)), mat(CUSHION, 0.95));
    cushion.position.y = bedR * 0.03;
    const bedG = new THREE.Group().add(rim, cushion);
    bedG.position.set(def.shadow.at[0], -bedR * 0.02, def.shadow.at[2]);
    this.bed = fade(bedG, from);
    this.hasBed = def.bed !== false;

    // three Z's drifting up from the head
    from = mats.length;
    const zShape = new THREE.Shape([new THREE.Vector2(-0.5, 0.5), new THREE.Vector2(0.5, 0.5), new THREE.Vector2(0.5, 0.3), new THREE.Vector2(-0.15, -0.3), new THREE.Vector2(0.5, -0.3), new THREE.Vector2(0.5, -0.5), new THREE.Vector2(-0.5, -0.5), new THREE.Vector2(-0.5, -0.3), new THREE.Vector2(0.15, 0.3), new THREE.Vector2(-0.5, 0.3)]);
    const zGeo = geo(new THREE.ShapeGeometry(zShape));
    const zMat = mat(ZZZ, 0.8);
    const zzzG = new THREE.Group();
    for (let i = 0; i < 3; i++) {
      const z = new THREE.Mesh(zGeo, zMat);
      this.zs.push(z);
      zzzG.add(z);
    }
    zzzG.position.set(r * 0.2, this.top - r * 0.05, 0.15);
    this.zzz = fade(zzzG, from);

    // confetti falling from above
    from = mats.length;
    const confG = new THREE.Group();
    const bitGeo = geo(new THREE.PlaneGeometry(r * 0.06, r * 0.035));
    const bitMats = CONFETTI.map((c) => {
      const m = mat(c, 0.6);
      m.side = THREE.DoubleSide;
      return m;
    });
    for (let i = 0; i < 36; i++) {
      const m = new THREE.Mesh(bitGeo, bitMats[i % bitMats.length]);
      this.bits.push({ m, v: new THREE.Vector3(), spin: new THREE.Vector3(Math.random() * 8, Math.random() * 8, Math.random() * 8) });
      confG.add(m);
    }
    this.confetti = fade(confG, from);

    // a postcard with a red stamp, held up in front of the pet
    from = mats.length;
    const card = new THREE.Mesh(geo(new THREE.BoxGeometry(r * 0.38, r * 0.26, r * 0.008)), mat(CARD, 0.8));
    const stamp = new THREE.Mesh(geo(new THREE.BoxGeometry(r * 0.07, r * 0.085, r * 0.01)), mat(STAMP, 0.6));
    stamp.position.set(r * 0.12, r * 0.07, r * 0.002);
    const cardG = new THREE.Group().add(card, stamp);
    cardG.position.set(r * 0.05, (def.wear.chest[1] + def.wear.top[1]) / 2 - r * 0.15, def.wear.chest[2] + r * 0.12);
    cardG.rotation.set(-0.15, 0.25, 0.08);
    this.card = fade(cardG, from);
  }

  /** Moves the props one frame; returns true while any of them is moving quickly. */
  update(s: PropState): boolean {
    const show = (f: Fade, on: boolean) => {
      f.o += ((on ? 1 : 0) - f.o) * Math.min(1, s.dt * 5);
      if (!on && f.o < 0.01) f.o = 0;
      f.obj.visible = f.o > 0;
      for (const m of f.mats) m.opacity = f.o;
    };
    show(this.bowl, s.bowl !== "none");
    // full when eating starts, gone 3 s later
    const left = s.bowl === "eating" ? Math.max(0, 1 - Math.max(0, s.eatAt - 0.5) / 3) : 0;
    this.food.visible = left > 0.02;
    this.food.scale.set(0.4 + left * 0.6, 0.15 + left * 0.35, 0.4 + left * 0.6);

    show(this.cloud, s.rain);
    if (this.cloud.obj.visible)
      this.drops.forEach((d, i) => {
        const p = (s.t * 0.45 + i * 0.27) % 1;
        d.position.y = -p * this.top * 0.18 - 0.01;
        d.scale.setScalar(p < 0.85 ? 1 : (1 - p) / 0.15);
      });

    show(this.bed, this.hasBed && s.asleep > 0.3);
    show(this.zzz, s.asleep > 0.9);
    if (this.zzz.obj.visible)
      this.zs.forEach((z, i) => {
        const p = (s.t * 0.25 + i / 3) % 1;
        const size = this.top * (0.05 + p * 0.04);
        z.scale.set(size, size, 1);
        z.position.set(p * this.top * 0.12 + Math.sin(p * 6) * this.top * 0.01, p * this.top * 0.2, 0);
        (z.material as THREE.MeshStandardMaterial).opacity = this.zzz.o * Math.min(1, (1 - p) * 3);
      });

    // a burst at the start of a celebration, falling and spinning for ~2.5 s
    const burst = s.confetti >= 0 && s.confetti < 2.6;
    if (burst && this.confetti.o === 0) {
      const spread = this.top * 0.9;
      this.bits.forEach((b) => {
        b.m.position.set((Math.random() - 0.5) * spread, this.top * (0.85 + Math.random() * 0.25), (Math.random() - 0.5) * spread * 0.5 + 0.1);
        b.v.set((Math.random() - 0.5) * 0.15, Math.random() * 0.15, (Math.random() - 0.5) * 0.08);
      });
    }
    show(this.confetti, burst);
    if (this.confetti.obj.visible)
      this.bits.forEach((b) => {
        b.v.y -= 0.35 * s.dt;
        b.v.multiplyScalar(1 - 1.5 * s.dt);
        b.m.position.addScaledVector(b.v, s.dt);
        b.m.rotation.x += b.spin.x * s.dt;
        b.m.rotation.y += b.spin.y * s.dt;
        b.m.rotation.z += b.spin.z * s.dt;
      });

    show(this.card, s.postcard >= 0 && s.postcard < 3.6);
    return burst || this.bowl.o % 1 > 0.01 || this.card.o % 1 > 0.01;
  }
}
