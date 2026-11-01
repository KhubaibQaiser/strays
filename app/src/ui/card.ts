// Info card (§8.3): a small panel beside the pet. Closes on Escape or when it loses focus.
import { api, Pet, UiState } from "./api";
import { MOOD_ICON, PACE_LABEL, SPECIES_ICON, ago, confirmButton, el } from "./dom";

const STATE_TEXT: Record<string, string> = {
  shelter: "In the shelter",
  snoozed: "Asleep",
  paused: "Paused",
  holiday: "On holiday",
  graduated: "Graduated",
  letGo: "Let go",
};

export function cardChrome() {
  addEventListener("keydown", (e) => {
    if (e.key === "Escape") void api.closeWindow("card");
  });
  addEventListener("blur", () => void api.closeWindow("card"));
}

function days(n: number): string {
  const d = Math.floor(n);
  return d === 1 ? "1 day" : `${d} days`;
}

export function renderCard(root: HTMLElement, state: UiState, id: string, notice: (msg: string, error?: boolean) => void) {
  const p = state.pets.find((x) => x.id === id);
  if (!p) {
    root.replaceChildren(el("div", { class: "infocard" }, el("p", { class: "muted" }, "This pet is gone.")));
    return;
  }
  const act = (action: string) => api.petAction(p.id, action).then((m) => notice(m)).catch((e) => notice(String(e), true));

  const status =
    p.state === "active"
      ? el("p", {}, `${MOOD_ICON[p.mood ?? ""] ?? ""} ${p.mood} · ${days(p.idleDays)} since last activity`, p.nextMoodDays != null ? el("span", { class: "muted" }, ` · next mood in ${days(Math.ceil(p.nextMoodDays))}`) : null)
      : el("p", {}, STATE_TEXT[p.state] ?? p.state, p.until ? el("span", { class: "muted" }, ` until ${new Date(p.until).toLocaleString(undefined, { weekday: "short", hour: "2-digit", minute: "2-digit" })}`) : null);

  const tracking = el(
    "div",
    { class: "small" },
    el("div", {}, p.repo ?? p.folder ?? "", " ", ...p.sources.map((s) => el("span", { class: "badge" }, s))),
    el("div", { class: "muted" }, `Last activity ${ago(p.lastActivity)} (${p.lastSource.replace(/([A-Z])/g, " $1").toLowerCase().trim()})`),
  );

  // goal and checklist, saved as they change
  const goal = structuredClone(p.goal);
  const save = () => api.setGoal(p.id, goal).catch((e) => notice(String(e), true));
  const goalInput = el("input", { type: "text", value: goal.text, placeholder: "What does done look like?", style: "width:100%" });
  goalInput.addEventListener("change", () => {
    goal.text = goalInput.value;
    void save();
  });
  const list = el("div");
  const drawList = () =>
    list.replaceChildren(
      ...goal.checklist.map((c, i) => {
        const box = el("input", { type: "checkbox", checked: c.done, "aria-label": c.text });
        box.addEventListener("change", () => {
          c.done = box.checked;
          void save();
        });
        const remove = el("button", { class: "small", "aria-label": `Remove ${c.text}` }, "×");
        remove.addEventListener("click", () => {
          goal.checklist.splice(i, 1);
          drawList();
          void save();
        });
        return el("div", { class: "check" }, box, el("span", { style: c.done ? "text-decoration:line-through" : "" }, c.text), remove);
      }),
    );
  drawList();
  const add = el("input", { type: "text", placeholder: "Add a step and press Return", style: "width:100%" });
  add.addEventListener("keydown", (e) => {
    if ((e as KeyboardEvent).key === "Enter" && add.value.trim()) {
      goal.checklist.push({ text: add.value.trim(), done: false });
      add.value = "";
      drawList();
      void save();
    }
  });

  const snooze = el(
    "select",
    { "aria-label": "Snooze" },
    el("option", { value: "" }, "Snooze…"),
    el("option", { value: "snooze-1h" }, "For 1 hour"),
    el("option", { value: "snooze-4h" }, "For 4 hours"),
    el("option", { value: "snooze-tomorrow" }, "Until tomorrow"),
    el("option", { value: "snooze-3d" }, "For 3 days"),
    el("option", { value: "snooze-wake" }, "Until I wake it"),
  );
  snooze.addEventListener("change", () => snooze.value && void act(snooze.value));

  const buttons: (HTMLElement | null)[] = [
    el("button", { onclick: () => void act("sync") }, "Sync now"),
    p.state === "active" ? snooze : null,
    p.state === "active" ? el("button", { onclick: () => void act("release") }, "Mark release") : null,
    p.state === "active" ? el("button", { onclick: () => void act("pause") }, "Pause") : null,
    p.state === "paused" ? el("button", { onclick: () => void act("resume") }, "Resume") : null,
    p.state === "snoozed" ? el("button", { onclick: () => void act("wake") }, "Wake now") : null,
    p.state === "shelter" ? el("button", { onclick: () => void act("adopt") }, "Adopt") : null,
    p.state === "letGo" ? el("button", { onclick: () => void act("undo") }, "Bring back") : null,
    p.repo ? el("button", { onclick: () => void api.openGithub(p.id) }, "Open on GitHub") : null,
    p.folder ? el("button", { onclick: () => void api.reveal(p.id) }, "Reveal in Finder") : null,
    ["active", "snoozed", "paused", "holiday", "shelter"].includes(p.state) ? confirmButton("Graduate…", "Finished? Click again", () => void act("graduate")) : null,
  ];

  root.replaceChildren(
    el(
      "div",
      { class: "infocard" },
      el("div", { class: "title" }, el("h1", {}, `${SPECIES_ICON[p.species]} ${p.name}`), el("span", { class: "muted small" }, PACE_LABEL[p.pace])),
      status,
      tracking,
      el("h2", {}, "Goal"),
      goalInput,
      list,
      add,
      el("div", { class: "buttons" }, ...buttons),
      petSettings(p, notice),
      el("p", { id: "notice", class: "small", role: "status" }),
    ),
  );
}

function petSettings(p: Pet, notice: (msg: string, error?: boolean) => void): HTMLElement {
  const name = el("input", { type: "text", value: p.name, "aria-label": "Name" });
  const pace = el("select", { "aria-label": "Pace" }, ...Object.entries(PACE_LABEL).map(([v, t]) => el("option", { value: v, selected: v === p.pace }, t)));
  const save = () => api.setPet(p.id, name.value, pace.value as Pet["pace"]).then(() => notice("Saved")).catch((e) => notice(String(e), true));
  name.addEventListener("change", save);
  pace.addEventListener("change", save);
  return el("details", {}, el("summary", {}, "Pet settings"), el("div", { class: "row" }, name, pace));
}

