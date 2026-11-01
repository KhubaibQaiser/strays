// Settings window (§8.4) including Privacy & Permissions (§8.5). Every change saves straight away.
import { api, Settings, UiState } from "./api";
import { PACE_LABEL, confirmButton, date, el, row } from "./dom";

const SECTIONS: [string, string][] = [
  ["pets", "Pets"],
  ["moods", "Moods"],
  ["display", "Display"],
  ["nudges", "Nudges"],
  ["interaction", "Interaction"],
  ["syncing", "Syncing"],
  ["privacy", "Privacy & Permissions"],
  ["about", "About"],
];

const DEFAULT_THRESHOLDS: Settings["thresholds"] = [2, 5, 10, 20];

type Notice = (msg: string, error?: boolean) => void;

export function renderSettings(root: HTMLElement, state: UiState, section: string, go: (section: string) => void, notice: Notice) {
  const s = structuredClone(state.settings);
  const save = () => api.saveSettings(s).then(() => notice("Saved")).catch((e) => notice(String(e), true));

  const num = (key: keyof Settings, min: number, max: number) => {
    const i = el("input", { type: "number", min, max, value: s[key] as number, "aria-label": String(key) });
    i.addEventListener("change", () => {
      (s[key] as number) = Math.round(Number(i.value));
      void save();
    });
    return i;
  };
  const toggle = (key: keyof Settings, label: string) => {
    const i = el("input", { type: "checkbox", checked: s[key] as boolean, id: String(key) });
    i.addEventListener("change", () => {
      (s[key] as boolean) = i.checked;
      void save();
    });
    return el("div", { class: "check" }, i, el("label", { for: String(key) }, label));
  };
  const select = <K extends keyof Settings>(key: K, options: [Settings[K], string][]) => {
    const sel = el("select", { "aria-label": String(key) }, ...options.map(([v, t], i) => el("option", { value: i, selected: v === s[key] }, t)));
    sel.addEventListener("change", () => {
      s[key] = options[Number(sel.value)][0];
      void save();
    });
    return sel;
  };
  const time = (key: "quietStart" | "quietEnd") => {
    const m = s[key];
    const i = el("input", { type: "time", value: `${String(Math.floor(m / 60)).padStart(2, "0")}:${String(m % 60).padStart(2, "0")}`, "aria-label": key });
    i.addEventListener("change", () => {
      const [hh, mm] = i.value.split(":").map(Number);
      s[key] = hh * 60 + mm;
      void save();
    });
    return i;
  };
  const flag = (name: "presentation" | "hideAll" | "vacation", on: boolean, label: string) => {
    const i = el("input", { type: "checkbox", checked: on, id: name });
    i.addEventListener("change", () => void api.setFlag(name, i.checked).catch((e) => notice(String(e), true)));
    return el("div", { class: "check" }, i, el("label", { for: name }, label));
  };

  const pages: Record<string, () => (Node | null)[]> = {
    pets: () => [
      el("h1", {}, "Pets"),
      row("Adoption slots", num("slots", 1, 20), el("span", { class: "muted" }, `${state.slotsUsed} in use`)),
      row("Pace for new pets", select("defaultPace", Object.entries(PACE_LABEL) as [Settings["defaultPace"], string][])),
      row("Holiday after a release", num("holidayDays", 1, 14), "days"),
      row("Days between holidays", num("releaseCooldownDays", 0, 90), "days"),
      row("Pet size", select("petSize", [["small", "Small"], ["medium", "Medium"], ["large", "Large"]])),
      el("h2", {}, "Never offer these repos as new projects"),
      ignoreList(s, save),
      el("div", { class: "buttons" }, el("button", { class: "primary", onclick: () => void api.openWindow("adopt") }, "Adopt a pet…")),
    ],
    moods: () => [
      el("h1", {}, "Moods"),
      el("p", { class: "muted" }, "A pet's mood drops as days pass without activity. Each level lasts up to the number of days shown. Holidays and paused time don't count."),
      thresholdRows(s, save),
      el(
        "div",
        { class: "buttons" },
        ...([["Sprint", 0.5], ["Normal", 1], ["Slow-burn", 2]] as const).map(([t, f]) =>
          el("button", { onclick: () => { s.thresholds = DEFAULT_THRESHOLDS.map((d) => d * f) as Settings["thresholds"]; void save().then(() => go("moods")); } }, `${t} preset`),
        ),
      ),
      toggle("skipWeekends", "Don't count weekends"),
    ],
    display: () => [
      el("h1", {}, "Display"),
      row(
        "Liveliness",
        select("liveliness", [
          ["calm", "Calm: still, with an occasional blink (about 1% CPU)"],
          ["gentle", "Gentle: small moments every few seconds (about 3–6% CPU)"],
          ["lively", "Lively: always moving (about 11% CPU, noticeably more battery)"],
        ]),
      ),
      el("p", { class: "muted small" }, "CPU figures were measured with 3 pets on an Apple Silicon Mac (Gate 2a). Reduce Motion in macOS settings always keeps pets calm."),
      row("Most pets on screen", num("maxVisible", 1, 8)),
      row("Fewest pets on screen", num("minVisible", 0, 8)),
      row("Change who's on screen every", num("rotationHours", 1, 48), "hours"),
      toggle("hideInFullScreen", "Hide pets when an app is full screen"),
      flag("presentation", state.presentation, "Presentation mode (hide repo names)"),
      flag("hideAll", state.hideAll, "Hide all pets"),
    ],
    nudges: () => [
      el("h1", {}, "Nudges"),
      el("p", { class: "muted" }, "A hungry pet may say something, now and then. Never during quiet hours, Presentation mode or a full-screen app."),
      row("Style", select("nudgeStyle", [["gentle", "Gentle"], ["playful", "Playful"], ["off", "Off"]])),
      row("Per pet, at most", num("nudgesPerPetDaily", 0, 10), "a day"),
      row("All pets together, at most", num("nudgesDaily", 0, 20), "a day"),
      toggle("quietHours", "Quiet hours"),
      row("Quiet from", time("quietStart"), "to", time("quietEnd")),
      row("Pets waking together come back", num("wakeStagger", 0, 60), "minutes apart"),
    ],
    interaction: () => [
      el("h1", {}, "Interaction"),
      el("p", { class: "muted" }, "Clicks pass straight through pets. Hold this key to hover, click or sync a pet."),
      row("Key", select("modifier", [["option", "⌥ Option"], ["control", "⌃ Control"], ["shift", "⇧ Shift"], ["optionShift", "⌥⇧ Option-Shift"]])),
    ],
    syncing: () => [
      el("h1", {}, "Syncing"),
      el("p", {}, state.lastSync ? `Last synced ${new Date(state.lastSync).toLocaleTimeString()}` : "Not synced yet", " · ", `${state.requestsToday} requests today`, state.rateRemaining != null ? ` · ${state.rateRemaining} left this hour` : "", state.syncError ? el("span", { class: "error" }, ` · ⚠ ${state.syncError}`) : null),
      row("Check GitHub", select("syncInterval", [[5, "Every 5 minutes"], [15, "Every 15 minutes"], [30, "Every 30 minutes"], [60, "Every hour"], [180, "Every 3 hours"], [720, "Every 12 hours"], [null, "Only when I ask"]])),
      toggle("syncOnWake", "Also when the Mac wakes"),
      toggle("syncOnLaunch", "Also when Strays starts"),
      toggle("syncPauseOnLowPower", "Pause in Low Power Mode"),
      el("div", { class: "check muted" }, el("input", { type: "checkbox", disabled: true }), "Only on Wi-Fi (not available yet)"),
      el("div", { class: "buttons" }, el("button", { onclick: () => void api.syncNow().then(() => notice("Syncing…")) }, "Sync now")),
    ],
    privacy: () => privacy(state, s, save, notice),
    about: () => [
      el("h1", {}, "About Strays"),
      el("p", {}, `Version ${state.version} (pre-release)`),
      el("p", { class: "muted" }, "A pet for every unfinished project. Local-first: nothing leaves this Mac except read-only requests to GitHub, and only once you connect it."),
      el("p", { class: "muted small" }, `Data folder: ${state.dataDir}`),
      el("p", { class: "muted small" }, "Not yet licensed. All rights reserved."),
    ],
  };

  const page = pages[section] ?? pages.pets;
  root.replaceChildren(
    el(
      "div",
      { class: "settings" },
      el("nav", { "aria-label": "Settings sections" }, ...SECTIONS.map(([id, t]) => el("button", { "aria-current": id === section ? "page" : false, onclick: () => go(id) }, t))),
      el("main", {}, ...page(), el("p", { id: "notice", class: "small", role: "status" })),
    ),
  );
}

function thresholdRows(s: Settings, save: () => Promise<unknown>): HTMLElement {
  const names = ["Joyful", "Content", "Peckish", "Hungry"];
  const err = el("p", { class: "error small", role: "alert" });
  const inputs = s.thresholds.map((d, i) => el("input", { type: "number", min: 0.5, max: 365, step: 0.5, value: d, "aria-label": `${names[i]} up to (days)` }));
  inputs.forEach((input) =>
    input.addEventListener("change", () => {
      const next = inputs.map((x) => Number(x.value)) as Settings["thresholds"];
      // the core rejects these too; say why here
      if (!next.every((v, i) => v > 0 && (i === 0 || v > next[i - 1]))) {
        err.textContent = "Each level must last longer than the one before it.";
        return;
      }
      err.textContent = "";
      s.thresholds = next;
      void save();
    }),
  );
  return el("div", {}, ...inputs.map((input, i) => row(`${names[i]} up to`, input, "days")), row("Gloomy", el("span", { class: "muted" }, "after that")), err);
}

function ignoreList(s: Settings, save: () => Promise<unknown>): HTMLElement {
  const t = el("textarea", { rows: 4, cols: 40, placeholder: "One per line, * as a wildcard, e.g. test-*", "aria-label": "Ignored repos" });
  t.value = s.ignore.join("\n");
  t.addEventListener("change", () => {
    s.ignore = t.value.split("\n").map((l) => l.trim()).filter(Boolean);
    void save();
  });
  return t;
}

function privacy(state: UiState, s: Settings, save: () => Promise<unknown>, notice: Notice): (Node | null)[] {
  const card = (title: string, status: string, good: boolean, ...body: (Node | string | null)[]) =>
    el("section", { class: "card", "aria-label": title }, el("header", {}, el("span", {}, title), el("span", { class: good ? "ok" : "muted" }, `${good ? "●" : "○"} ${status}`)), ...body);

  const gh = state.account;
  const github = card(
    "GitHub access",
    gh ? "Connected" : "Not connected",
    Boolean(gh),
    el("p", {}, el("b", {}, "Can: "), "see repo names and when they were last pushed to."),
    el("p", {}, el("b", {}, "Can't: "), "read code in private repos, change anything, or see issues and pull requests. Public repos are readable by any token."),
    gh ? el("p", {}, `Account: ${gh.login} · sees ${gh.repoCount} repos · token expires ${date(gh.expires)}`) : null,
    el(
      "div",
      { class: "buttons" },
      gh ? el("button", { onclick: () => void api.syncNow().then(() => notice("Testing with a sync…")) }, "Test") : null,
      el("button", { class: gh ? "" : "primary", onclick: () => void api.openWindow("setup") }, gh ? "Replace token" : "Connect GitHub…"),
      gh ? confirmButton("Disconnect", "Delete the token? Click again", () => void api.tokenDisconnect().then(() => notice("Disconnected; the token was deleted from the Keychain")), "danger") : null,
    ),
  );

  const folderList = el("div");
  const drawFolders = () =>
    folderList.replaceChildren(
      ...s.watchedFolders.map((f, i) =>
        el("div", { class: "row" }, el("span", {}, f), el("button", { onclick: () => { s.watchedFolders.splice(i, 1); void save(); drawFolders(); } }, "Remove")),
      ),
    );
  drawFolders();
  const folders = card(
    "Project folders",
    s.watchedFolders.length ? `${s.watchedFolders.length} watched` : "None",
    s.watchedFolders.length > 0,
    el("p", {}, "Reads commit dates and remote URLs only. Never file contents. New git repos inside these folders are offered as projects."),
    folderList,
    el(
      "div",
      { class: "buttons" },
      el("button", {
        onclick: async () => {
          const f = await api.pickFolder();
          if (f && !s.watchedFolders.includes(f)) {
            s.watchedFolders.push(f);
            await save();
            drawFolders();
          }
        },
      }, "Add folder…"),
    ),
  );

  const logBox = el("div");
  const network = card(
    "Network",
    s.offline ? "Fully offline" : "api.github.com only",
    !s.offline,
    el("p", {}, `${state.requestsToday} requests today.`),
    el(
      "div",
      { class: "buttons" },
      el("button", {
        onclick: async () => {
          const log = await api.netlog();
          logBox.replaceChildren(
            log.length
              ? el("table", { class: "log" }, el("tr", {}, el("th", {}, "Time"), el("th", {}, "Request"), el("th", {}, "Status"), el("th", {}, "Why")), ...log.slice(0, 200).map((e) => el("tr", {}, el("td", {}, new Date(e.at).toLocaleString()), el("td", {}, `${e.method} ${e.host}${e.path}`), el("td", {}, e.status || "no answer"), el("td", {}, e.reason))))
              : el("p", { class: "muted" }, "No requests yet."),
          );
        },
      }, "View activity log"),
    ),
    (() => {
      const i = el("input", { type: "checkbox", checked: s.offline, id: "offline" });
      i.addEventListener("change", () => {
        s.offline = i.checked;
        void save();
      });
      return el("div", { class: "check" }, i, el("label", { for: "offline" }, "Go fully offline"));
    })(),
    logBox,
  );

  const capture = card("Screen capture", "Pets hidden when sharing", true, el("p", {}, "Pets never appear in screenshots, recordings or screen sharing."));
  const notYet = card("Launch at login · Focus status", "Not available yet", false, el("p", { class: "muted" }, "Arrives with the signed release (Gate 4)."));

  const confirmBox = el("input", { type: "text", placeholder: "Type DELETE", "aria-label": "Type DELETE to confirm" });
  const data = card(
    "Your data",
    "On this Mac",
    true,
    el("p", {}, "Pets, settings and logs are JSON files in the data folder. The token is in the Keychain."),
    el(
      "div",
      { class: "buttons" },
      el("button", { onclick: () => void api.exportData().then((p) => notice(`Exported to ${p}`)).catch((e) => notice(String(e), true)) }, "Export JSON"),
      el("button", { onclick: () => void api.openDataFolder() }, "Open data folder"),
    ),
    el("h2", {}, "Delete everything"),
    el("p", { class: "muted small" }, "Removes the token from the Keychain and every data file and log, then quits. This can't be undone."),
    el("div", { class: "row" }, confirmBox, el("button", { class: "danger", onclick: () => void api.deleteEverything(confirmBox.value).catch((e) => notice(String(e), true)) }, "Delete everything")),
  );

  return [el("h1", {}, "Privacy & Permissions"), github, folders, network, capture, notYet, data];
}
