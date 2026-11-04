// Adoption flow (§8.2): project, species, colour (with a live preview), name, goal, pace.
import { Rive, RuntimeLoader } from "@rive-app/canvas";
import riveWasmUrl from "@rive-app/canvas/rive.wasm?url";
import { artFor } from "../art";
import { api, Pace, Species, UiState } from "./api";
import { PACE_LABEL, SPECIES_ICON, ago, el } from "./dom";
import { deriveLook, hexToArgb, looksFor } from "./look";
import { Pet3DRig, SPECIES_3D } from "../pet3d/rig";
import { meshReady } from "../pet3d/meshes";

RuntimeLoader.setWasmUrl(riveWasmUrl);

const NAMES = ["Miso", "Biscuit", "Pixel", "Nori", "Mochi", "Pepper", "Tofu", "Juniper", "Waffle", "Echo", "Clover", "Sprout", "Fig", "Bean"];
const SPECIES: [Species, string][] = [["cat", "Cat (Persian)"], ["dog", "Dog (Corgi)"], ["hamster", "Hamster (Syrian)"], ["parrot", "Parrot (macaw)"]];

export function renderAdopt(root: HTMLElement, state: UiState, query: URLSearchParams, notice: (msg: string, error?: boolean) => void) {
  let repoId: number | null = query.get("repo") ? Number(query.get("repo")) : null;
  let folder: string | null = query.get("folder");
  let species: Species = "cat";
  const randomLook = () => { const looks = looksFor(species); return { ...looks[Math.floor(Math.random() * looks.length)] }; };
  let colors = randomLook();
  let pace: Pace = state.settings.defaultPace;

  // project
  const untracked = state.repos.filter((r) => !r.tracked);
  const repoSel = el(
    "select",
    { "aria-label": "Repository" },
    el("option", { value: "" }, untracked.length ? "Choose a GitHub repo…" : "No GitHub repos (connect GitHub in Settings)"),
    ...untracked.map((r) => el("option", { value: r.id, selected: r.id === repoId }, `${r.fullName}${r.pushedAt ? ` · ${ago(r.pushedAt)}` : ""}`)),
  );
  const folderText = el("span", { class: "muted" }, folder ?? "");
  const localChoices = state.localRepos.filter(([, , tracked]) => !tracked);
  const folderSel = el(
    "select",
    { "aria-label": "Folder" },
    el("option", { value: "" }, "Choose a folder from your watched folders…"),
    ...localChoices.map(([path, shown]) => el("option", { value: path, selected: path === folder }, shown)),
  );
  folderSel.addEventListener("change", () => {
    folder = folderSel.value || null;
    folderText.textContent = "";
    notice("");
  });
  const name = el("input", { type: "text", "aria-label": "Name", value: NAMES[Math.floor(Math.random() * NAMES.length)] });
  repoSel.addEventListener("change", () => {
    repoId = repoSel.value ? Number(repoSel.value) : null;
    notice("");
  });
  const pickFolder = el("button", {
    onclick: async () => {
      const f = await api.pickFolder();
      if (f) {
        folder = f;
        folderText.textContent = f;
        notice("");
      }
    },
  }, "Or a folder on this Mac…");

  // species
  const speciesRow = el("div", { class: "choices", role: "group", "aria-label": "Species" });
  const drawSpecies = () =>
    speciesRow.replaceChildren(
      ...SPECIES.map(([sp, t]) => el("button", { class: "choice", "aria-pressed": sp === species ? "true" : "false", onclick: () => { const swap = (sp === "parrot") !== (species === "parrot"); species = sp; if (swap) { colors = randomLook(); drawSwatches(); } drawSpecies(); loadPreview(); } }, `${SPECIES_ICON[sp]} ${t}`)),
    );
  drawSpecies();

  // colour + preview: the pet as it will look on screen, in 3D (in 2D if this Mac can't draw 3D)
  const stage = el("div", { class: "preview", role: "img" });
  const report = (key: string, text: string) => api.reportProblem(key, text).catch((e) => notice(`${text} (and it couldn't be recorded: ${e})`, true));
  let shown: { cleanup(): void } | null = null;
  let rive: Rive | null = null;
  let asked = 0;
  function loadPreview() {
    const mine = ++asked;
    stage.setAttribute("aria-label", `Preview of your ${species}`);
    void meshReady(species, report).then(() => {
      if (mine !== asked) return; // a newer choice came in meanwhile
      shown?.cleanup();
      rive = null;
      const canvas = el("canvas", { class: "preview" });
      stage.replaceChildren(canvas);
      try {
        const rig = new Pet3DRig(canvas, SPECIES_3D[species], colors, report, () => drawIn2d());
        rig.setLiveliness("gentle", matchMedia("(prefers-reduced-motion: reduce)").matches);
        rig.resize(160);
        rig.play();
        shown = rig;
      } catch (e) {
        void report("preview-3d", `The adoption preview can't draw in 3D (${e}), so it's drawn in 2D`);
        drawIn2d();
      }
    });
  }
  function drawIn2d() {
    shown?.cleanup();
    const canvas = el("canvas", { class: "preview", width: 320, height: 320 });
    stage.replaceChildren(canvas);
    rive = new Rive({ src: artFor(species), canvas, autoplay: true, stateMachine: "Idle", autoBind: true, onLoad: () => paint() });
    shown = rive;
  }
  // a new colour rebuilds the 3D pet (its coat is baked into its materials); dragging the picker
  // only rebuilds once it settles
  let repaint = 0;
  const paint = () => {
    if (!rive) {
      window.clearTimeout(repaint);
      repaint = window.setTimeout(loadPreview, 120);
      return;
    }
    const vmi = rive.viewModelInstance;
    for (const [k, v] of Object.entries(colors)) {
      const prop = vmi?.color(k);
      if (prop) prop.value = hexToArgb(v);
    }
  };
  const swatches = el("div", { class: "choices", role: "group", "aria-label": "Colour" });
  const picker = el("input", { type: "color", value: colors.coatBase.toLowerCase(), "aria-label": "Any colour" });
  const drawSwatches = () => {
    picker.value = colors.coatBase.toLowerCase();
    swatches.replaceChildren(
      ...looksFor(species).map((look, i) => el("button", { class: "swatch", style: `background:${look.coatBase}`, "aria-label": `Colour ${i + 1}`, "aria-pressed": look.coatBase === colors.coatBase ? "true" : "false", onclick: () => { colors = { ...look }; drawSwatches(); paint(); } })),
      picker,
    );
  };
  picker.addEventListener("input", () => {
    colors = deriveLook(picker.value);
    drawSwatches();
    paint();
  });
  drawSwatches();
  loadPreview();

  const goal = el("input", { type: "text", placeholder: "What does done look like? (optional)", style: "width:100%", "aria-label": "Goal" });
  const paceSel = el("select", { "aria-label": "Pace" }, ...Object.entries(PACE_LABEL).map(([v, t]) => el("option", { value: v, selected: v === pace }, t)));
  paceSel.addEventListener("change", () => (pace = paceSel.value as Pace));

  const adopt = el("button", { class: "primary" }, "Adopt");
  adopt.addEventListener("click", async () => {
    if (repoId == null && !folder) return notice("Pick a GitHub repo or a folder first.", true);
    adopt.disabled = true;
    try {
      notice(await api.adopt({ repoId, folder, species, colors, name: name.value, goal: goal.value, pace }));
      void api.closeWindow("adopt");
    } catch (e) {
      notice(String(e), true);
      adopt.disabled = false;
    }
  });

  const slotsFull = state.slotsUsed >= state.settings.slots;
  root.replaceChildren(
    el(
      "div",
      { class: "page" },
      el("h1", {}, "Adopt a pet"),
      slotsFull ? el("p", { class: "muted" }, `All ${state.settings.slots} slots are in use, so this pet will wait in the shelter until one frees up.`) : null,
      el("h2", {}, "1. Project"),
      repoSel,
      localChoices.length ? folderSel : null,
      el("div", { class: "row" }, pickFolder, folderText),
      el("h2", {}, "2. Species"),
      speciesRow,
      el("h2", {}, "3. Colour"),
      el("div", { class: "row" }, stage, swatches),
      el("h2", {}, "4. Name"),
      name,
      el("h2", {}, "5. Goal"),
      goal,
      el("h2", {}, "6. Pace"),
      el("div", { class: "row" }, paceSel, el("span", { class: "muted small" }, "Sprint gets hungry twice as fast; Slow-burn half as fast.")),
      el("div", { class: "buttons" }, adopt, el("button", { onclick: () => void api.closeWindow("adopt") }, "Cancel")),
      el("p", { id: "notice", class: "small", role: "status" }),
    ),
  );
}
