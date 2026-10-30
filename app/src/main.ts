// Gate 0 overlay prototype renderer: plays the cat .riv, recolours it, reacts to ⌥-hover/click.
import "./throttle";
import { Rive, RuntimeLoader } from "@rive-app/canvas";
import riveWasmUrl from "@rive-app/canvas/rive.wasm?url";
import catUrl from "../../art/cat/build/cat.riv?url";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const log = (msg: string) => void invoke("pet_log", { msg });
addEventListener("error", (e) => log(`ERROR ${e.message}`));
addEventListener("unhandledrejection", (e) => log(`REJECT ${String(e.reason)}`));
log(`BOOT ${location.href}`);

// Bundled locally: the runtime must never fetch its WASM from a CDN.
RuntimeLoader.setWasmUrl(riveWasmUrl);

// One test colour per pet so recolouring is visible: coatBase, coatShade, coatHighlight, outline.
const LOOKS: Array<[number, number, number, number]> = [
  [0xffefe1cc, 0xffd9c4a6, 0xfffff8ee, 0xff7a5c46],
  [0xff34343a, 0xff26262b, 0xff55555e, 0xffa9a9b6],
  [0xffe8a15c, 0xffcb8240, 0xfffbe3c6, 0xff7a4520],
];

const params = new URLSearchParams(location.search);
const mode = params.get("mode") ?? "A";

type Slot = { pet: number; x: number; y: number; w: number; h: number };
const slots: Slot[] =
  mode === "B"
    ? (params.get("pets") ?? "").split(";").map((s, pet) => {
        const [x, y, w, h] = s.split(",").map(Number);
        return { pet, x, y, w, h };
      })
    : [{ pet: Number(params.get("pet") ?? 0), x: 0, y: 0, w: innerWidth, h: innerHeight }];

const canvases = new Map<number, HTMLCanvasElement>();

for (const slot of slots) {
  const canvas = document.createElement("canvas");
  Object.assign(canvas.style, {
    position: "absolute",
    left: `${slot.x}px`,
    top: `${slot.y}px`,
    width: `${slot.w}px`,
    height: `${slot.h}px`,
  });
  canvas.width = slot.w * devicePixelRatio;
  canvas.height = slot.h * devicePixelRatio;
  document.body.appendChild(canvas);
  canvases.set(slot.pet, canvas);

  const rive: Rive = new Rive({
    src: catUrl,
    canvas,
    autoplay: true,
    stateMachines: "Idle",
    autoBind: true,
    onLoadError: (e) => log(`LOADERROR pet=${slot.pet} ${JSON.stringify(e.data)}`),
    onLoad: () => {
      rive.resizeDrawingSurfaceToCanvas();
      const vmi = rive.viewModelInstance;
      const look = LOOKS[slot.pet % LOOKS.length];
      const names = ["coatBase", "coatShade", "coatHighlight", "outline"];
      const bound = names.filter((name, i) => {
        const prop = vmi?.color(name);
        if (prop) prop.value = look[i];
        return prop != null;
      });
      void invoke("pet_log", { msg: `LOADED pet=${slot.pet} colours=${bound.length}/4` });
      rive.enableFPSCounter(() => {});
      setTimeout(() => log(`FPS pet=${slot.pet} ${rive.fps}`), 6000);
    },
  });

  canvas.addEventListener("mousedown", (e) => {
    if (!e.altKey) return;
    void invoke("pet_clicked", { pet: slot.pet });
    showBubble(slot, "Hi! I'm tracking strays-test 🐾");
  });
}

void listen<[number, boolean]>("pet-hover", ({ payload: [pet, on] }) => {
  canvases.get(pet)?.classList.toggle("hover", on);
});

function showBubble(slot: Slot, text: string) {
  const bubble = document.createElement("div");
  bubble.className = "bubble";
  bubble.textContent = text;
  bubble.style.left = `${slot.x + slot.w / 2}px`;
  bubble.style.top = `${slot.y + 4}px`;
  document.body.appendChild(bubble);
  setTimeout(() => bubble.remove(), 2500);
}
