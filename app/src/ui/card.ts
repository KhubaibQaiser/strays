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
  // the window is see-through; only the rounded card is drawn
  document.documentElement.classList.add("card-window");
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
  const act = (action: string, extra: { until?: string; label?: string } = {}) => api.petAction(p.id, action, extra).then((m) => notice(m)).catch((e) => notice(String(e), true));

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
    el("option", { value: "custom" }, "Until a time I pick…"),
  );
  // a picked time: a date-and-time field and a button, shown only when asked for
  const snoozeAt = el("input", { type: "datetime-local", "aria-label": "Snooze until" });
  const snoozeGo = el("button", { onclick: () => snoozeAt.value && void act("snooze-until", { until: new Date(snoozeAt.value).toISOString() }) }, "Snooze");
  const custom = el("div", { class: "row" }, snoozeAt, snoozeGo);
  custom.hidden = true;
  snooze.addEventListener("change", () => {
    custom.hidden = snooze.value !== "custom";
    if (snooze.value && snooze.value !== "custom") void act(snooze.value);
  });
  const resumeOn = el("input", { type: "date", "aria-label": "Pause until (optional)", title: "Pause until (optional)" });
  // resumes at 09:00 local on the picked day
  const pauseExtra = () => (resumeOn.value ? { until: new Date(`${resumeOn.value}T09:00`).toISOString() } : {});
  const version = el("input", { type: "text", placeholder: "Version (optional)", "aria-label": "Release version (optional)", size: 12, maxlength: 40 });

  const buttons: (HTMLElement | null)[] = [
    el("button", { onclick: () => void act("sync") }, "Sync now"),
    p.state === "active" ? snooze : null,
    p.state === "active" ? custom : null,
    // a release sends the pet away for days, so a slip of the mouse mustn't do it
    p.state === "active" ? el("div", { class: "row" }, version, confirmButton("Mark release…", "Shipped it? Click again", () => void act("release", version.value.trim() ? { label: version.value } : {}))) : null,
    p.state === "active" ? el("div", { class: "row" }, el("button", { onclick: () => void act("pause", pauseExtra()) }, "Pause"), resumeOn) : null,
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
      p.missing && p.state !== "letGo" && p.state !== "graduated" ? missing(p, act) : null,
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

/** GitHub answers 404 both for a deleted repo and for one the token can't see, so the pet asks. */
function missing(p: Pet, act: (action: string) => Promise<unknown>): HTMLElement {
  return el(
    "div",
    { class: "issues", role: "alert" },
    el("p", {}, el("b", {}, "❓ Where did my repo go?")),
    el("p", { class: "small" }, `GitHub can't find ${p.repo ?? "this pet's repo"} any more. It may have been deleted, or your token can't see it now (for example a new token limited to fewer repos). ${p.name}'s mood is on hold until you decide.`),
    el(
      "div",
      { class: "buttons" },
      confirmButton("It was deleted: let go", "Let go? Click again", () => void act("letgo")),
      el("button", { onclick: () => void api.openWindow("setup") }, "Check the token…"),
      el("button", { onclick: () => void act("sync") }, "Look again"),
    ),
  );
}

function petSettings(p: Pet, notice: (msg: string, error?: boolean) => void): HTMLElement {
  const name = el("input", { type: "text", value: p.name, "aria-label": "Name" });
  const custom = !!p.customThresholds;
  const pace = el(
    "select",
    { "aria-label": "Pace" },
    ...Object.entries(PACE_LABEL).map(([v, t]) => el("option", { value: v, selected: !custom && v === p.pace }, t)),
    el("option", { value: "custom", selected: custom }, "Custom days…"),
  );
  // this pet's own mood days, shown when "Custom days…" is picked
  const names = ["Joyful", "Content", "Peckish", "Hungry"];
  const start = p.customThresholds ?? [2, 5, 10, 20];
  const days = start.map((d, i) => el("input", { type: "number", min: 0.5, max: 365, step: 0.5, value: d, size: 4, "aria-label": `${names[i]} up to (days), this pet` }));
  const daysRow = el("div", { class: "row small" }, ...days.flatMap((d, i) => [el("span", { class: "muted" }, `${names[i]} ≤`), d]), el("span", { class: "muted" }, "days, then Gloomy"));
  daysRow.hidden = !custom;
  const motion = el(
    "select",
    { "aria-label": "Liveliness" },
    ...[["", "Liveliness: as in Settings"], ["calm", "Calm"], ["gentle", "Gentle"], ["lively", "Lively"]].map(([v, t]) => el("option", { value: v, selected: v === (p.liveliness ?? "") }, t)),
  );
  const save = () => {
    const own = (motion.value || null) as Pet["liveliness"];
    const isCustom = pace.value === "custom";
    daysRow.hidden = !isCustom;
    const th = isCustom ? (days.map((d) => Number(d.value)) as [number, number, number, number]) : null;
    if (th && !th.every((v, i) => v > 0 && (i === 0 || v > th[i - 1]))) return notice("Each level must last longer than the one before it.", true);
    // a custom pet keeps its last pace underneath, so switching back restores it
    const keptPace = (isCustom ? p.pace : pace.value) as Pet["pace"];
    return api.setPet(p.id, name.value, keptPace, own, th).then(() => notice("Saved")).catch((e) => notice(String(e), true));
  };
  for (const c of [name, pace, motion, ...days]) c.addEventListener("change", save);
  return el("details", {}, el("summary", {}, "Pet settings"), el("div", { class: "row" }, name, pace, motion), daysRow);
}

