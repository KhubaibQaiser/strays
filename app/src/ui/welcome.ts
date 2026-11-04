// First run (§8.1): three skippable screens, then adopting existing projects, several at once (§5.9).
import { api, saveChanged, Settings, Species, UiState } from "./api";
import { ago, el, row } from "./dom";
import { looksFor } from "./look";
import { REPORTS_TEXT } from "./settings";

const NAMES = ["Miso", "Biscuit", "Pixel", "Nori", "Mochi", "Pepper", "Tofu", "Juniper", "Waffle", "Echo", "Clover", "Sprout", "Fig", "Bean"];
const SPECIES: Species[] = ["cat", "dog", "hamster", "parrot"];

/** Updates the open welcome window's project list from new state (repos arrive with the first sync). */
export let fillProjects: (state: UiState) => void = () => {};

export function renderWelcome(root: HTMLElement, state: UiState, notice: (msg: string, error?: boolean) => void) {
  let step = 0;
  const s: Settings = structuredClone(state.settings);
  const base: Settings = structuredClone(state.settings);

  const next = () => {
    step++;
    draw();
  };
  const finish = async () => {
    try {
      await saveChanged(s, base);
      await api.openWindow("adopt");
      void api.closeWindow("welcome");
    } catch (e) {
      notice(String(e), true);
    }
  };
  const footer = (...b: Node[]) => el("div", { class: "buttons" }, ...b, el("button", { onclick: () => void finish() }, "Skip"));

  const screens = [
    () => [
      el("h1", {}, "Welcome to Strays 🐾"),
      el("p", {}, "Every project you haven't finished is a stray looking for a home. Give each one a pet. Work on the project and its pet stays happy; leave it too long and the pet gets hungry. Pets never die."),
      el("p", { class: "muted" }, "Strays works offline. Nothing is on until you choose."),
      footer(el("button", { class: "primary", onclick: next }, "Continue")),
    ],
    () => [
      el("h1", {}, "Where are your projects?"),
      el("div", { class: "card" }, el("b", {}, "GitHub"), el("p", { class: "muted" }, "Recommended if you work on more than one Mac: pushes from anywhere count. Needs a read-only token."), el("button", { onclick: () => void api.openWindow("setup") }, "Connect GitHub…")),
      el(
        "div",
        { class: "card" },
        el("b", {}, "Folders on this Mac"),
        el("p", { class: "muted" }, "No network at all. Only commits made or fetched on this Mac count."),
        el("button", {
          onclick: async () => {
            const f = await api.pickFolder();
            if (f && !s.watchedFolders.includes(f)) {
              s.watchedFolders.push(f);
              notice(`Watching ${f}`);
            }
          },
        }, "Add a folder…"),
      ),
      el("p", { class: "muted small" }, "You can use both, and change this later in Settings."),
      footer(el("button", { class: "primary", onclick: next }, "Continue")),
    ],
    () => {
      const num = (key: "nudgesDaily" | "maxVisible", min: number, max: number) => {
        const i = el("input", { type: "number", min, max, value: s[key] });
        i.addEventListener("change", () => {
          const v = Math.round(i.valueAsNumber);
          if (Number.isFinite(v) && v >= min && v <= max) s[key] = v;
          else {
            i.value = String(s[key]);
            notice(`Enter a number from ${min} to ${max}`, true);
          }
        });
        return i;
      };
      const quiet = el("input", { type: "checkbox", checked: s.quietHours, id: "quiet" });
      quiet.addEventListener("change", () => (s.quietHours = quiet.checked));
      // asked, never assumed: the box starts unticked
      const reports = el("input", { type: "checkbox", checked: s.errorReports, id: "reports" });
      reports.addEventListener("change", () => (s.errorReports = reports.checked));
      return [
        el("h1", {}, "Comfort"),
        row("Nudges per day, at most", num("nudgesDaily", 0, 20)),
        el("div", { class: "check" }, quiet, el("label", { for: "quiet" }, "Quiet hours, 22:00–08:00")),
        row("Most pets on screen", num("maxVisible", 1, 8)),
        state.reportsAvailable ? el("div", { class: "check" }, reports, el("label", { for: "reports" }, "Help fix bugs: send anonymous error and crash reports")) : null,
        state.reportsAvailable ? el("p", { class: "muted small" }, REPORTS_TEXT) : null,
        el("p", {}, "Clicks go straight through your pets. Hold ⌥ (Option) and click a pet to open its card, or hold ⌥ and drag it to move it. You can pick another key in Settings → Interaction."),
        el("p", { class: "muted small" }, "Sound and launch at login are in Settings, both off until you turn them on."),
        footer(el("button", { class: "primary", onclick: () => void saveChanged(s, base).then(next, (e) => notice(String(e), true)) }, "Continue")),
      ];
    },
    // existing projects, newest work first: tick any number, each gets a pet (or a shelter place)
    () => {
      const list = el("div", { class: "projects" });
      const picked = new Set<string>();
      const adoptAll = el("button", { class: "primary", disabled: true }, "Adopt selected");
      fillProjects = (st: UiState) => {
        const repos = st.repos.filter((r) => !r.tracked).sort((a, b) => (b.pushedAt ?? "").localeCompare(a.pushedAt ?? ""));
        const folders = st.localRepos.filter(([, , tracked]) => !tracked);
        const item = (key: string, label: string, detail: string) => {
          const box = el("input", { type: "checkbox", id: key, checked: picked.has(key) });
          box.addEventListener("change", () => {
            if (box.checked) picked.add(key);
            else picked.delete(key);
            adoptAll.disabled = picked.size === 0;
            adoptAll.textContent = picked.size ? `Adopt ${picked.size}` : "Adopt selected";
          });
          return el("div", { class: "check" }, box, el("label", { for: key }, label, el("span", { class: "muted small" }, detail)));
        };
        list.replaceChildren(
          ...repos.map((r) => item(`repo:${r.id}`, r.fullName, r.pushedAt ? ` · ${ago(r.pushedAt)}` : "")),
          ...folders.map(([path, shown]) => item(`folder:${path}`, shown, " · this Mac")),
        );
        if (!repos.length && !folders.length) list.replaceChildren(el("p", { class: "muted" }, st.account && !st.lastSync ? "Fetching your repos…" : "No projects yet: connect GitHub or add a folder (step 2), or skip this."));
      };
      fillProjects(state);
      adoptAll.addEventListener("click", async () => {
        adoptAll.disabled = true;
        const keys = [...picked];
        const names = [...NAMES].sort(() => Math.random() - 0.5);
        try {
          for (const [i, key] of keys.entries()) {
            const species = SPECIES[i % SPECIES.length];
            const looks = looksFor(species);
            const [kind, id] = [key.slice(0, key.indexOf(":")), key.slice(key.indexOf(":") + 1)];
            const msg = await api.adopt({ repoId: kind === "repo" ? Number(id) : null, folder: kind === "folder" ? id : null, species, colors: { ...looks[Math.floor(Math.random() * looks.length)] }, name: names[i % names.length], goal: "", pace: kind === "folder" ? "slowBurn" : s.defaultPace });
            notice(msg);
          }
          void api.closeWindow("welcome");
        } catch (e) {
          notice(String(e), true);
          adoptAll.disabled = false;
        }
      });
      return [
        el("h1", {}, "Adopt your projects"),
        el("p", { class: "muted" }, "Tick the ones you want to finish. Each gets a pet; past your adoption slots they wait in the shelter. Species, colours and names are picked for you; you can rename a pet in its card."),
        list,
        el("div", { class: "buttons" }, adoptAll, el("button", { onclick: () => void finish() }, "Choose one by hand…"), el("button", { onclick: () => void api.closeWindow("welcome") }, "Skip")),
      ];
    },
  ];

  const draw = () => root.replaceChildren(el("div", { class: "page" }, el("p", { class: "muted small" }, `Step ${step + 1} of ${screens.length}`), ...screens[step](), el("p", { id: "notice", class: "small", role: "status" })));
  draw();
}
