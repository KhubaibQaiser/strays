// First run (§8.1): three skippable screens, then adoption.
import { api, Settings, UiState } from "./api";
import { el, row } from "./dom";
import { REPORTS_TEXT } from "./settings";

export function renderWelcome(root: HTMLElement, state: UiState, notice: (msg: string, error?: boolean) => void) {
  let step = 0;
  const s: Settings = structuredClone(state.settings);

  const next = () => {
    step++;
    draw();
  };
  const finish = async () => {
    try {
      await api.saveSettings(s);
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
        i.addEventListener("change", () => (s[key] = Math.round(Number(i.value))));
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
        el("p", { class: "muted small" }, "Sound and launch at login arrive later; both will start switched off."),
        el("div", { class: "buttons" }, el("button", { class: "primary", onclick: () => void finish() }, "Adopt your first pet…")),
      ];
    },
  ];

  const draw = () => root.replaceChildren(el("div", { class: "page" }, el("p", { class: "muted small" }, `Step ${step + 1} of 3`), ...screens[step](), el("p", { id: "notice", class: "small", role: "status" })));
  draw();
}
