// Settings window (§8.4) including Privacy & Permissions (§8.5). Every change saves straight away.
import { api, saveChanged, Settings, Species, UiState } from "./api";
import { PACE_LABEL, confirmButton, date, el, row } from "./dom";

const SECTIONS: [string, string][] = [
  ["pets", "Pets"],
  ["moods", "Moods"],
  ["display", "Display"],
  ["nudges", "Nudges"],
  ["sound", "Sound"],
  ["interaction", "Interaction"],
  ["syncing", "Syncing"],
  ["privacy", "Privacy & Permissions"],
  ["about", "About"],
];

const SPECIES: [Species, string][] = [["cat", "Cats"], ["dog", "Dogs"], ["hamster", "Hamsters"], ["parrot", "Parrots"]];

const DEFAULT_THRESHOLDS: Settings["thresholds"] = [2, 5, 10, 20];

type Notice = (msg: string, error?: boolean) => void;

export function renderSettings(root: HTMLElement, state: UiState, section: string, go: (section: string) => void, notice: Notice) {
  const s = structuredClone(state.settings);
  const base = structuredClone(state.settings);
  const save = () => saveChanged(s, base).then(() => notice("Saved")).catch((e) => notice(String(e), true));

  const num = (key: keyof Settings, min: number, max: number) => {
    const i = el("input", { type: "number", min, max, value: s[key] as number, "aria-label": String(key) });
    i.addEventListener("change", () => {
      const v = Math.round(i.valueAsNumber);
      // an emptied or out-of-range field is put back rather than sent
      if (!Number.isFinite(v) || v < min || v > max) {
        i.value = String(s[key]);
        return notice(`${String(key)} must be a number from ${min} to ${max}`, true);
      }
      (s[key] as number) = v;
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
          ["calm", "Calm: slower, lighter motion (least battery)"],
          ["gentle", "Gentle: smooth motion"],
          ["lively", "Lively: the smoothest, always moving (most battery)"],
        ]),
      ),
      el(
        "p",
        { class: "muted small" },
        "One pet at Gentle measured about 8–26% of one CPU core on an Apple Silicon Mac, depending on what else the Mac was doing; each extra pet adds less. A pet can have its own liveliness in its card. Reduce Motion in macOS settings always keeps pets calm.",
      ),
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
    sound: () => [
      el("h1", {}, "Sound"),
      el("p", { class: "muted" }, "Pets make a small sound when they greet you, eat, celebrate, feel low or wake up. Never during quiet hours, Presentation mode or a full-screen app, and never for snoozed or paused pets."),
      toggle("sound", "Play sounds"),
      row("Volume", volumeSlider(s, save), el("button", { onclick: () => void api.testSound().catch((e) => notice(String(e), true)) }, "Test")),
      el("h2", {}, "Species"),
      ...SPECIES.map(([sp, label]) => {
        const i = el("input", { type: "checkbox", checked: !s.mutedSpecies.includes(sp), id: `sound-${sp}` });
        i.addEventListener("change", () => {
          s.mutedSpecies = i.checked ? s.mutedSpecies.filter((m) => m !== sp) : [...s.mutedSpecies, sp];
          void save();
        });
        return el("div", { class: "check" }, i, el("label", { for: `sound-${sp}` }, label));
      }),
    ],
    interaction: () => [
      el("h1", {}, "Interaction"),
      el("p", { class: "muted" }, "Clicks pass straight through pets. Hold this key to hover, click or sync a pet."),
      row("Key", select("modifier", [["option", "⌥ Option"], ["control", "⌃ Control"], ["shift", "⇧ Shift"], ["optionShift", "⌥⇧ Option-Shift"]])),
      shortcutRow(s, save, notice),
    ],
    syncing: () => [
      el("h1", {}, "Syncing"),
      el("p", {}, state.lastSync ? `Last synced ${new Date(state.lastSync).toLocaleTimeString()}` : "Not synced yet", " · ", `${state.requestsToday} requests today`, state.rateRemaining != null ? ` · ${state.rateRemaining} left this hour` : "", state.syncError ? (/^Offline/.test(state.syncError) ? ` · ${state.syncError}` : el("span", { class: "error" }, ` · ⚠ ${state.syncError}`)) : null),
      row("Check GitHub", select("syncInterval", [[5, "Every 5 minutes"], [15, "Every 15 minutes"], [30, "Every 30 minutes"], [60, "Every hour"], [180, "Every 3 hours"], [720, "Every 12 hours"], [null, "Only when I ask"]])),
      toggle("syncOnWake", "Also when the Mac wakes"),
      toggle("syncOnLaunch", "Also when Strays starts"),
      toggle("syncPauseOnLowPower", "Pause in Low Power Mode"),
      toggle("syncOnlyOnWifi", "Only on Wi-Fi (scheduled syncs wait; Sync now always works)"),
      el("div", { class: "buttons" }, el("button", { onclick: () => void api.syncNow().then(() => notice("Syncing…")).catch((e) => notice(String(e), true)) }, "Sync now")),
    ],
    privacy: () => privacy(state, s, save, notice),
    about: () => [
      el("h1", {}, "About Strays"),
      el("p", {}, `Version ${state.version} (pre-release)`),
      toggle("checkUpdates", "Tell me when a new version is out"),
      el("p", { class: "muted small" }, "Off by default. When on, Strays asks GitHub once a week for the latest release, without your token. Shown in the activity log. Updates are never installed for you."),
      state.update
        ? el("div", { class: "buttons" }, el("span", {}, `Strays ${state.update.tag_name} is out.`), el("button", { class: "primary", onclick: () => void api.openLink("release").catch((e) => notice(String(e), true)) }, "Download…"))
        : s.checkUpdates && state.updateChecked
          ? el("p", { class: "muted small" }, `Up to date · checked ${new Date(state.updateChecked).toLocaleDateString()}`)
          : null,
      el("p", { class: "muted" }, "A pet for every unfinished project. Local-first: nothing leaves this Mac except read-only requests to GitHub, and only once you connect it."),
      el("p", { class: "muted small" }, `Data folder: ${state.dataDir}`),
      el("p", { class: "muted small" }, "MIT licence. Sounds are CC0 recordings from Freesound."),
    ],
  };

  const page = pages[section] ?? pages.pets;
  root.replaceChildren(
    el(
      "div",
      { class: "settings" },
      el("nav", { "aria-label": "Settings sections" }, ...SECTIONS.map(([id, t]) => el("button", { "aria-current": id === section ? "page" : false, onclick: () => go(id) }, t))),
      el("main", {}, issues(state, notice), ...page(), el("p", { id: "notice", class: "small", role: "status" })),
    ),
  );
}

/** Problems the app hit (unreadable files, failed saves, lost folder access…), until fixed or dismissed. */
function issues(state: UiState, notice: Notice): HTMLElement | null {
  if (!state.issues.length) return null;
  return el(
    "div",
    { class: "issues", role: "alert" },
    ...state.issues.map((t) => el("p", { class: "error small" }, `⚠ ${t}`)),
    el("button", { onclick: () => void api.dismissIssues().catch((e) => notice(String(e), true)) }, "Dismiss"),
  );
}

/** What an error report holds, word for word the same in Settings and the welcome screen. */
export const REPORTS_TEXT =
  "Sent to PostHog: what kind of error it was and a short code (never its message, so no names or paths), the Strays and macOS versions, and a random id, with location lookup turned off. At most 20 a day; each one shows in the activity log. Nothing is sent while this is off or Strays is offline.";

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
      gh ? el("button", { onclick: () => void api.syncNow().then(() => notice("Testing with a sync…")).catch((e) => notice(String(e), true)) }, "Test") : null,
      el("button", { class: gh ? "" : "primary", onclick: () => void api.openWindow("setup") }, gh ? "Replace token" : "Connect GitHub…"),
      gh ? confirmButton("Disconnect", "Delete the token? Click again", () => void api.tokenDisconnect().then(() => notice("Disconnected; the token was deleted from the Keychain")).catch((e) => notice(String(e), true)), "danger") : null,
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
    s.offline ? "Fully offline" : s.errorReports && state.reportsAvailable ? "api.github.com and PostHog" : "api.github.com only",
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
  const login = (() => {
    const i = el("input", { type: "checkbox", checked: state.loginItem !== "off", id: "login-item" });
    i.addEventListener("change", () => void api.setLoginItem(i.checked).then(() => notice("Saved")).catch((e) => { i.checked = !i.checked; notice(String(e), true); }));
    return card(
      "Launch at login",
      { on: "On", off: "Off", needsApproval: "Turned off in System Settings" }[state.loginItem],
      state.loginItem === "on",
      el("div", { class: "check" }, i, el("label", { for: "login-item" }, "Open Strays when you log in")),
      state.loginItem === "needsApproval" ? el("p", { class: "muted small" }, "Allow it in System Settings → General → Login Items.") : el("p", { class: "muted small" }, "Listed in System Settings → General → Login Items."),
    );
  })();
  const reports = (() => {
    const i = el("input", { type: "checkbox", checked: s.errorReports, id: "error-reports", disabled: !state.reportsAvailable });
    i.addEventListener("change", () => {
      s.errorReports = i.checked;
      void save();
    });
    return card(
      "Error reports",
      !state.reportsAvailable ? "Not in this build" : s.errorReports ? "On" : "Off",
      s.errorReports && state.reportsAvailable,
      el("div", { class: "check" }, i, el("label", { for: "error-reports" }, "Send anonymous error and crash reports")),
      el("p", { class: "muted small" }, REPORTS_TEXT),
      logButtons(notice, state.reportsAvailable),
    );
  })();
  const notYet = card("Focus status", "Not available yet", false, el("p", { class: "muted" }, "Needs the signed release."));

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
    el("p", { class: "muted small" }, "Removes the token from the Keychain, every data file and log, folder access and launch at login, then quits. This can't be undone."),
    el("div", { class: "row" }, confirmBox, el("button", { class: "danger", onclick: () => void api.deleteEverything(confirmBox.value).catch((e) => notice(String(e), true)) }, "Delete everything")),
  );

  return [el("h1", {}, "Privacy & Permissions"), github, folders, network, reports, capture, login, notYet, data];
}

/** Show the log, save a copy safe to attach to a bug report, or send one after reading exactly what goes. */
function logButtons(notice: (msg: string, error?: boolean) => void, canSend: boolean): HTMLElement {
  const fail = (e: unknown) => notice(String(e), true);
  const preview = el("pre", { class: "logpreview", tabindex: 0, "aria-label": "The log that would be sent" });
  const confirm = el("div", { class: "buttons" });
  const box = el("div", { class: "sendlog" }, el("p", { class: "small" }, "This is everything that would be sent. Names, repos, folders and paths are already blanked out."), preview, confirm);
  box.hidden = true;
  const close = () => (box.hidden = true);
  confirm.append(
    el("button", {
      class: "primary",
      onclick: () => void api.sendLog().then(() => (close(), notice("Sent. Thank you!"))).catch(fail),
    }, "Send this"),
    el("button", { onclick: close }, "Cancel"),
  );
  const save = () =>
    void api.saveLog().then((p) => {
        notice(`Saved ${p}, with names and paths blanked out. Attach it to a new issue on GitHub.`);
        return api.openLink("new-issue");
      }).catch(fail);
  const send = () =>
    void api.logPreview().then((text) => {
        preview.textContent = text;
        box.hidden = false;
        preview.focus();
      }).catch(fail);
  return el(
    "div",
    {},
    el(
      "div",
      { class: "buttons" },
      el("button", { onclick: () => void api.revealLog().catch(fail) }, "Show log"),
      el("button", { onclick: save }, "Save log for a bug report…"),
      canSend ? el("button", { onclick: send }, "Send log to the developer…") : null,
    ),
    box,
  );
}

const SUGGESTED_SHORTCUT = "Control+Alt+S";
/** "Control+Alt+S" as macOS writes it: ⌃⌥S */
const keysLabel = (keys: string) =>
  keys.split("+").map((k) => ({ Control: "⌃", Alt: "⌥", Shift: "⇧", Super: "⌘" })[k] ?? k).join("");

/** Hide or show every pet from anywhere: off by default; a recorder takes the next key combination. */
function shortcutRow(s: Settings, save: () => Promise<unknown>, notice: Notice): HTMLElement {
  const on = el("input", { type: "checkbox", id: "hide-shortcut", checked: !!s.hideShortcut });
  const keys = el("button", { "aria-label": "Shortcut keys, click to record new ones" }, keysLabel(s.hideShortcut ?? SUGGESTED_SHORTCUT));
  keys.disabled = !s.hideShortcut;
  const set = (value: string | null) => {
    s.hideShortcut = value;
    keys.textContent = keysLabel(value ?? SUGGESTED_SHORTCUT);
    keys.disabled = !value;
    void save();
  };
  on.addEventListener("change", () => set(on.checked ? SUGGESTED_SHORTCUT : null));
  keys.addEventListener("click", () => {
    keys.textContent = "Press the keys…";
    const take = (e: KeyboardEvent) => {
      e.preventDefault();
      if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return; // still holding modifiers
      removeEventListener("keydown", take, true);
      if (e.key === "Escape") return set(s.hideShortcut);
      const mods = [e.ctrlKey && "Control", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"].filter(Boolean) as string[];
      const key = e.code.replace(/^Key|^Digit/, "");
      if (!mods.length || !/^[A-Z0-9]$|^F\d{1,2}$/.test(key)) {
        notice("Use at least one of ⌃ ⌥ ⇧ ⌘ with a letter, number or F-key.", true);
        return set(s.hideShortcut);
      }
      set([...mods, key].join("+"));
    };
    addEventListener("keydown", take, true);
  });
  return el(
    "div",
    {},
    el("div", { class: "check" }, on, el("label", { for: "hide-shortcut" }, "Hide or show all pets with a shortcut"), keys),
    el("p", { class: "muted small" }, "Works from any app. Click the keys to record others; Esc keeps the old ones."),
  );
}

function volumeSlider(s: Settings, save: () => Promise<void>) {
  const i = el("input", { type: "range", min: 0, max: 100, step: 5, value: s.volume, "aria-label": "Volume" });
  i.addEventListener("change", () => {
    s.volume = Number(i.value);
    void save();
  });
  return i;
}
