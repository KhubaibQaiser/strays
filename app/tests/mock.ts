// Stands in for Tauri's IPC in the browser: answers the app's commands from a fixture and records calls.
import type { Page } from "@playwright/test";

export const PET_ID = "11111111-1111-4111-8111-111111111111";

export const STATE = {
  pets: [
    {
      id: PET_ID,
      name: "Miso",
      species: "cat",
      colors: { coatBase: "#EFE1CC", coatShade: "#D9C4A6", coatHighlight: "#FFF8EE", outline: "#7A5C46" },
      state: "active",
      until: null,
      since: "2026-10-20T10:00:00Z",
      mood: "Peckish",
      idleDays: 6.4,
      nextMoodDays: 3.6,
      repo: "me/strays",
      folder: null,
      sources: ["GitHub"],
      lastActivity: "2026-10-26T10:00:00Z",
      lastSource: "Github",
      goal: { text: "Ship v1", checklist: [{ text: "Gate 2", done: true }] },
      pace: "normal",
      onScreen: true,
      releases: 0,
    },
  ],
  settings: {
    thresholds: [2, 5, 10, 20],
    defaultPace: "normal",
    skipWeekends: false,
    slots: 3,
    holidayDays: 3,
    releaseCooldownDays: 14,
    letGoUndoDays: 30,
    syncInterval: 30,
    syncOnWake: true,
    syncOnLaunch: true,
    syncOnlyOnWifi: false,
    syncPauseOnLowPower: true,
    maxVisible: 3,
    minVisible: 1,
    rotationHours: 3,
    liveliness: "calm",
    ignore: [],
    petSize: "medium",
    modifier: "option",
    hideInFullScreen: true,
    nudgeStyle: "gentle",
    nudgesPerPetDaily: 1,
    nudgesDaily: 3,
    quietHours: true,
    quietStart: 1320,
    quietEnd: 480,
    wakeStagger: 3,
    watchedFolders: [],
    offline: false,
    sound: false,
    volume: 50,
    mutedSpecies: [],
    checkUpdates: false,
  },
  account: { login: "me", expires: "2026-11-29T13:28:15Z", repoCount: 96 },
  hasToken: true,
  lastSync: "2026-11-01T10:00:00Z",
  syncError: null,
  rateRemaining: 4990,
  requestsToday: 12,
  repos: [
    { id: 1, fullName: "me/strays", pushedAt: "2026-10-26T10:00:00Z", private: true, tracked: true },
    { id: 2, fullName: "me/side-project", pushedAt: "2026-10-01T10:00:00Z", private: false, tracked: false },
  ],
  newProjects: [],
  localRepos: [["/Users/me/Code/notes", "~/Code/notes", false]],
  newFolders: [],
  slotsUsed: 1,
  presentation: false,
  vacation: false,
  hideAll: false,
  dataDir: "/Users/me/Library/Application Support/com.khubaibqaiser.strays",
  version: "0.1.0",
  loginItem: "off",
  update: null as { tag_name: string; html_url: string } | null,
  updateChecked: null as string | null,
  firstRun: false,
};

export type Call = { cmd: string; args: Record<string, unknown> };

/** Installs the mock before the page loads. `fail` makes those commands reject with the given text; `patch` changes the state. */
export async function mockApp(page: Page, fail: Record<string, string> = {}, patch: Partial<typeof STATE> = {}) {
  await page.addInitScript(
    ({ state, fail }) => {
      const calls: { cmd: string; args: unknown }[] = [];
      (window as unknown as { __calls: unknown }).__calls = calls;
      let next = 1;
      const answers: Record<string, unknown> = {
        ui_state: state,
        token_check: { login: "me", repoCount: 96, expires: "2026-11-29T13:28:15Z" },
        pet_action: "Done",
        adopt: "Biscuit moved in",
        netlog: [{ at: "2026-11-01T10:00:00Z", method: "GET", host: "api.github.com", path: "/user/repos?sort=pushed", status: 200, reason: "scheduled" }],
        pick_folder: "/Users/me/Code",
        export_data: "/tmp/strays-export.json",
      };
      // event handlers by name, so tests can emit events the way the app does
      const callbacks: Record<number, (e: unknown) => void> = {};
      const handlers: Record<string, number[]> = {};
      (window as unknown as { __emit: unknown }).__emit = (event: string, payload: unknown) =>
        (handlers[event] ?? []).forEach((h) => callbacks[h]?.({ event, id: 0, payload }));
      (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
        transformCallback: (cb: (e: unknown) => void) => {
          callbacks[next] = cb;
          return next++;
        },
        unregisterCallback: () => {},
        invoke: async (cmd: string, args: Record<string, unknown>) => {
          if (cmd === "plugin:event|listen") (handlers[args.event as string] ??= []).push(args.handler as number);
          if (cmd.startsWith("plugin:event|")) return next++;
          calls.push({ cmd, args });
          if (cmd in fail) throw fail[cmd];
          return structuredClone(answers[cmd] ?? null);
        },
        metadata: { currentWindow: { label: "test" }, currentWebview: { label: "test" } },
      };
      (window as unknown as { __TAURI_EVENT_PLUGIN_INTERNALS__: unknown }).__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    },
    { state: { ...STATE, ...patch }, fail },
  );
}

export async function calls(page: Page, cmd: string): Promise<Call[]> {
  const all = (await page.evaluate(() => (window as unknown as { __calls: Call[] }).__calls)) as Call[];
  return all.filter((c) => c.cmd === cmd);
}
