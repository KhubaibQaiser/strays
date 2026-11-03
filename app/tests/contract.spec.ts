// The Rust ↔ TypeScript contract. src-tauri's `contract` test writes contract.json from the real
// payloads; this checks the mock the UI tests run on has exactly that shape, and tsc (tsconfig in this
// folder) checks the mock against the app's types. So a field renamed on one side fails here, not in the app.
import { expect, test } from "@playwright/test";
import contract from "./contract.json" with { type: "json" };
import { STATE, placed } from "./mock";
import type { NetEntry, UiState } from "../src/ui/api";
import type { Feedback, Frame, Hover } from "../src/main";

// compile-time half: the mock samples are what the app's types say they are
const state: UiState = STATE;
const frame: Frame = { pets: [{ ...placed("a", "content", 0), confused: true }], liveliness: "calm", reduceMotion: false };
const hover: Hover = { id: null, held: true };
const feedback: Feedback = { id: "a", ok: true, text: "✓" };
const entry: NetEntry = { at: "2026-11-01T10:00:00Z", method: "GET", host: "api.github.com", path: "/user/repos", status: 200, reason: "scheduled" };

type Shape = string | Shape[] | { [k: string]: Shape };

/** Where `value` doesn't fit `shape` (see `shape` in src-tauri/src/commands.rs). */
function mismatches(shape: Shape, value: unknown, at: string): string[] {
  if (typeof shape === "string") {
    const kind = value === null ? "null" : typeof value;
    return shape === "never" || shape.split("|").includes(kind) ? [] : [`${at}: Rust sends ${shape}, the mock has ${kind}`];
  }
  if (Array.isArray(shape)) {
    if (!Array.isArray(value)) return [`${at}: Rust sends an array`];
    return value.flatMap((v, i) => mismatches(shape[0], v, `${at}[${i}]`));
  }
  if ("nullable" in shape) return value === null ? [] : mismatches(shape.nullable, value, at);
  if (typeof value !== "object" || value === null || Array.isArray(value)) return [`${at}: Rust sends an object`];
  const keys = Object.keys(shape);
  // no keys: a map with any keys (pet colours)
  if (keys.length === 0) return [];
  const have = Object.keys(value);
  return [
    ...keys.filter((k) => !have.includes(k)).map((k) => `${at}.${k}: Rust sends it, the mock lacks it`),
    ...have.filter((k) => !keys.includes(k)).map((k) => `${at}.${k}: in the mock, but Rust never sends it`),
    ...keys.filter((k) => have.includes(k)).flatMap((k) => mismatches(shape[k], (value as Record<string, unknown>)[k], `${at}.${k}`)),
  ];
}

test("the mock UI state, overlay frame and events have the shape Rust sends", () => {
  expect(mismatches(contract.uiState, state, "uiState")).toEqual([]);
  expect(mismatches(contract.overlay, frame, "overlay")).toEqual([]);
  expect(mismatches(contract.hover, hover, "hover")).toEqual([]);
  expect(mismatches(contract.feedback, feedback, "feedback")).toEqual([]);
  expect(mismatches(contract.netEntry, entry, "netEntry")).toEqual([]);
});

test("the checker itself catches renames, missing fields and wrong kinds", () => {
  expect(mismatches({ repoCount: "number" }, { repo_count: 1 }, "a")).toEqual(["a.repoCount: Rust sends it, the mock lacks it", "a.repo_count: in the mock, but Rust never sends it"]);
  expect(mismatches({ n: "null|number" }, { n: "1" }, "a")).toEqual(["a.n: Rust sends null|number, the mock has string"]);
  expect(mismatches({ nullable: { x: "string" } }, null, "a")).toEqual([]);
});

/** `#[tauri::command]` functions and their webview-supplied arguments (State and AppHandle are Tauri's). */
function rustCommands(sources: string[]): Map<string, string[]> {
  const out = new Map<string, string[]>();
  for (const src of sources) {
    for (const m of src.matchAll(/#\[tauri::command\]\s*(?:pub )?(?:async )?fn (\w+)\(([^)]*)\)/g)) {
      const args = [...m[2].matchAll(/(\w+):\s*([\w:]+)/g)].filter((a) => !/^(State|AppHandle|tauri::)/.test(a[2])).map((a) => a[1].replace(/_(\w)/g, (_, c: string) => c.toUpperCase()));
      out.set(m[1], args);
    }
  }
  return out;
}

test("every command the webviews call exists, is registered, and gets the arguments Rust expects", async ({ request }) => {
  const text = async (path: string) => (await request.get(path)).text();
  // the dev server compiles .ts; `?raw` gives the source as written, as a JS string export
  const source = async (path: string) => JSON.parse((await text(`${path}?raw`)).replace(/^export default /, "").replace(/;\s*$/, "")) as string;
  const rust = await Promise.all(["/src-tauri/src/lib.rs", "/src-tauri/src/commands.rs"].map(text));
  const commands = rustCommands(rust);
  const registered = new Set([...rust[0].matchAll(/^\s+(?:commands::)?(\w+),$/gm)].map((m) => m[1]));
  const ts = await Promise.all(["/src/ui/api.ts", "/src/main.ts"].map(source));
  const calls = ts.flatMap((src) => [...src.matchAll(/invoke(?:<[^(]*>)?\("(\w+)"(?:, \{([^}]*)\})?\)/g)]);
  expect(calls.length).toBeGreaterThan(20);
  const problems = calls.flatMap(([, name, args = ""]) => {
    if (!commands.has(name)) return [`${name}: no such Rust command`];
    if (!registered.has(name)) return [`${name}: not in generate_handler!`];
    const sent = args.split(",").map((a) => a.split(":")[0].trim()).filter(Boolean).sort();
    const wanted = [...commands.get(name)!].sort();
    return JSON.stringify(sent) === JSON.stringify(wanted) ? [] : [`${name}: sends {${sent}}, Rust takes {${wanted}}`];
  });
  expect(problems).toEqual([]);
  // a registered command no page calls is attack surface for nothing
  const called = new Set(calls.map((c) => c[1]));
  expect([...registered].filter((r) => commands.has(r) && !called.has(r))).toEqual([]);
});

test("each window may call exactly the commands its pages use, and no wrapper is left unused", async ({ request }) => {
  const source = async (path: string) => JSON.parse((await (await request.get(`${path}?raw`)).text()).replace(/^export default /, "").replace(/;\s*$/, "")) as string;
  const api = await source("/src/ui/api.ts");
  // api.ts wrapper name → command
  const wrappers = new Map([...api.matchAll(/^\s+(\w+): \([^)]*\) => invoke(?:<[^(]*>)?\("(\w+)"/gm)].map((m) => [m[1], m[2]]));
  const views: Record<string, string> = { card: "card", settings: "settings", setup: "setup", adopt: "adopt", welcome: "welcome" };
  const shared = await source("/src/ui/main.ts"); // every window's router
  const used: Record<string, Set<string>> = {};
  for (const [file, window] of Object.entries(views)) {
    const src = (await source(`/src/ui/${file}.ts`)) + shared;
    used[window] = new Set([...src.matchAll(/api\.(\w+)\(/g)].map((m) => wrappers.get(m[1])).filter((c): c is string => !!c));
    // saveChanged (api.ts) reads the latest settings, then saves
    if (src.includes("saveChanged(")) ["ui_state", "save_settings"].forEach((c) => used[window].add(c));
  }
  used.overlay = new Set([...(await source("/src/main.ts")).matchAll(/invoke\("(\w+)"/g)].map((m) => m[1]));
  const problems: string[] = [];
  for (const [window, cmds] of Object.entries(used)) {
    const cap = (await (await request.get(`/src-tauri/capabilities/${window}.json`)).json()) as { windows: string[]; permissions: string[] };
    const allowed = new Set(cap.permissions.filter((p) => p.startsWith("allow-")).map((p) => p.slice(6).replace(/-/g, "_")));
    expect(cap.windows).toEqual([window]);
    problems.push(...[...cmds].filter((c) => !allowed.has(c)).map((c) => `${window} calls ${c} without permission`));
    problems.push(...[...allowed].filter((c) => !cmds.has(c)).map((c) => `${window} may call ${c} but never does`));
  }
  const everyUse = new Set(Object.values(used).flatMap((s) => [...s]));
  problems.push(...[...wrappers].filter(([, c]) => !everyUse.has(c)).map(([w]) => `api.${w} is never called`));
  expect(problems).toEqual([]);
});
