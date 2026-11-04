// Typed wrappers over the app's commands (src-tauri/src/commands.rs).
import { invoke } from "@tauri-apps/api/core";

export type Species = "cat" | "dog" | "hamster" | "parrot";
export type Pace = "sprint" | "normal" | "slowBurn";
export type Liveliness = "calm" | "gentle" | "lively";
export type StateKind = "shelter" | "active" | "snoozed" | "paused" | "holiday" | "graduated" | "letGo";
export type Goal = { text: string; checklist: { text: string; done: boolean }[] };

export type Pet = {
  id: string;
  name: string;
  species: Species;
  colors: Record<string, string>;
  state: StateKind;
  until: string | null;
  since: string;
  mood: string | null;
  idleDays: number;
  nextMoodDays: number | null;
  repo: string | null;
  folder: string | null;
  sources: string[];
  lastActivity: string;
  lastSource: string;
  goal: Goal;
  pace: Pace;
  /** its own liveliness, or null to follow the app's */
  liveliness: Liveliness | null;
  /** its own mood days (Joyful, Content, Peckish, Hungry up to), if not Settings' scaled by its pace */
  customThresholds: [number, number, number, number] | null;
  onScreen: boolean;
  releases: number;
  missing: boolean;
};

export type Repo = { id: number; fullName: string; pushedAt: string | null; private: boolean; tracked: boolean };

export type Settings = {
  thresholds: [number, number, number, number];
  defaultPace: Pace;
  skipWeekends: boolean;
  slots: number;
  holidayDays: number;
  releaseCooldownDays: number;
  letGoUndoDays: number;
  syncInterval: number | null;
  syncOnWake: boolean;
  syncOnLaunch: boolean;
  syncOnlyOnWifi: boolean;
  syncPauseOnLowPower: boolean;
  maxVisible: number;
  minVisible: number;
  rotationHours: number;
  liveliness: Liveliness;
  ignore: string[];
  petSize: "small" | "medium" | "large";
  modifier: "option" | "control" | "shift" | "optionShift";
  hideInFullScreen: boolean;
  nudgeStyle: "gentle" | "playful" | "off";
  nudgesPerPetDaily: number;
  nudgesDaily: number;
  quietHours: boolean;
  quietStart: number;
  quietEnd: number;
  wakeStagger: number;
  watchedFolders: string[];
  offline: boolean;
  sound: boolean;
  volume: number;
  mutedSpecies: Species[];
  checkUpdates: boolean;
  errorReports: boolean;
};

export type UiState = {
  pets: Pet[];
  settings: Settings;
  account: { login: string; expires: string | null; repoCount: number } | null;
  hasToken: boolean;
  lastSync: string | null;
  syncError: string | null;
  rateRemaining: number | null;
  requestsToday: number;
  repos: Repo[];
  newProjects: Repo[];
  localRepos: [string, string, boolean][];
  newFolders: string[];
  slotsUsed: number;
  presentation: boolean;
  vacation: boolean;
  hideAll: boolean;
  dataDir: string;
  version: string;
  loginItem: "on" | "off" | "needsApproval";
  update: { tag_name: string; html_url: string } | null;
  updateChecked: string | null;
  issues: string[];
  reportsAvailable: boolean;
  firstRun: boolean;
};

export type NetEntry = { at: string; method: string; host: string; path: string; status: number; reason: string };
export type TokenCheck = { login: string; repoCount: number; expires: string | null };
export type AdoptRequest = {
  repoId: number | null;
  folder: string | null;
  species: Species;
  colors: Record<string, string>;
  name: string;
  goal: string;
  pace: Pace;
};

/** Saves only the settings that differ from `base` (what this page started from), on top of the
 *  latest saved ones, so two open windows can't undo each other's changes. Updates `base` after. */
export async function saveChanged(s: Settings, base: Settings): Promise<void> {
  const latest = (await api.state()).settings as Record<string, unknown>;
  const mine = s as Record<string, unknown>;
  const before = base as Record<string, unknown>;
  for (const k of Object.keys(mine)) if (JSON.stringify(mine[k]) !== JSON.stringify(before[k])) latest[k] = structuredClone(mine[k]);
  await api.saveSettings(latest as Settings);
  Object.assign(base, structuredClone(s));
}

export const api = {
  state: () => invoke<UiState>("ui_state"),
  /** a problem this window hit, for the menu, Settings and the log */
  reportProblem: (key: string, text: string) => invoke<void>("report_problem", { key, text }),
  /** `until` (ISO time) for a picked snooze or pause end, `label` for a release's version */
  petAction: (id: string, action: string, extra: { until?: string; label?: string } = {}) => invoke<string>("pet_action", { id, action, until: extra.until ?? null, label: extra.label ?? null }),
  setGoal: (id: string, goal: Goal) => invoke<void>("set_goal", { id, goal }),
  setPet: (id: string, name: string, pace: Pace, liveliness: Liveliness | null, thresholds: [number, number, number, number] | null) => invoke<void>("set_pet", { id, name, pace, liveliness, thresholds }),
  adopt: (request: AdoptRequest) => invoke<string>("adopt", { request }),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  setFlag: (name: "presentation" | "hideAll" | "vacation", on: boolean) => invoke<void>("set_flag", { name, on }),
  syncNow: () => invoke<void>("sync_now"),
  dismissIssues: () => invoke<void>("dismiss_issues"),
  testSound: () => invoke<void>("test_sound"),
  tokenCheck: (token: string) => invoke<TokenCheck>("token_check", { token }),
  tokenSave: (token: string) => invoke<void>("token_save", { token }),
  tokenDisconnect: () => invoke<void>("token_disconnect"),
  pickFolder: () => invoke<string | null>("pick_folder"),
  setLoginItem: (on: boolean) => invoke<UiState["loginItem"]>("set_login_item", { on }),
  openGithub: (id: string) => invoke<void>("open_github", { id }),
  reveal: (id: string) => invoke<void>("reveal", { id }),
  openLink: (which: "token-page" | "release" | "new-issue") => invoke<void>("open_link", { which }),
  netlog: () => invoke<NetEntry[]>("netlog"),
  openDataFolder: () => invoke<void>("open_data_folder"),
  revealLog: () => invoke<void>("reveal_log"),
  saveLog: () => invoke<string>("save_log"),
  logPreview: () => invoke<string>("log_preview"),
  sendLog: () => invoke<void>("send_log"),
  exportData: () => invoke<string>("export_data"),
  deleteEverything: (confirm: string) => invoke<void>("delete_everything", { confirm }),
  openWindow: (kind: "settings" | "setup" | "adopt" | "welcome", arg?: string) => invoke<void>("open_window", { kind, arg }),
  closeWindow: (label: string) => invoke<void>("close_window", { label }),
};
