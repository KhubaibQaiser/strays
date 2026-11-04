// Router for the app's windows: the URL fragment picks the view; state comes from the core.
import { listen } from "@tauri-apps/api/event";
import { api, UiState } from "./api";
import { cardChrome, renderCard } from "./card";
import { renderSettings } from "./settings";
import { renderSetup } from "./setup";
import { renderAdopt } from "./adopt";
import { renderWelcome } from "./welcome";

const root = document.getElementById("app")!;
let route = location.hash.slice(1);
let state: UiState | null = null;

function notice(msg: string, error = false) {
  // views without a notice line (setup, an error page) still get one
  const n = document.getElementById("notice") ?? root.appendChild(Object.assign(document.createElement("p"), { id: "notice", role: "status" }));
  n.textContent = msg;
  n.className = `small ${error ? "error" : "ok"}`;
}

function render() {
  if (!state) return;
  const [path, query = ""] = route.split("?");
  const [view, arg = ""] = path.split("/");
  switch (view) {
    case "card":
      return renderCard(root, state, arg, notice);
    case "settings":
      return renderSettings(root, state, arg || "pets", (s) => go(`settings/${s}`), notice);
    case "setup":
      return renderSetup(root);
    case "adopt":
      return renderAdopt(root, state, new URLSearchParams(query), notice);
    case "welcome":
      return renderWelcome(root, state, notice);
  }
}

function go(next: string) {
  route = next;
  history.replaceState(null, "", `#${next}`);
  render();
}

const SCROLLERS = "main, .infocard";
// a text field the user is in; fields save on change, so skipping redraws there loses nothing
const typing = () => root.contains(document.activeElement) && !!document.activeElement?.matches("textarea, input:not([type=checkbox], [type=radio], [type=range], [type=button])");

// anything that fails without its own handler still reaches the user, here and in the menu and log
const unexpected = (text: string) => {
  notice(text, true);
  api.reportProblem("script", `Something went wrong in this window: ${text}`).catch(() => notice(`…and Strays couldn't record it`, true));
};
addEventListener("unhandledrejection", (e) => unexpected(String(e.reason)));
addEventListener("error", (e) => unexpected(e.message));

async function refresh() {
  try {
    state = await api.state();
  } catch (e) {
    root.replaceChildren(Object.assign(document.createElement("p"), { className: "error", textContent: `Strays couldn't load its state: ${e}` }));
    // shown above either way; if even reporting fails, the page is the only place left to say it
    api.reportProblem("state", `A window couldn't load Strays' state: ${e}`).catch((r) => root.append(` (and it couldn't be recorded: ${r})`));
    return;
  }
  // forms keep what the user is typing; only views that just display state redraw on changes
  const live = route.startsWith("card") || route.startsWith("settings");
  if (!root.firstChild) return render();
  if (!live || typing()) return;
  const tops = [...root.querySelectorAll(SCROLLERS)].map((e) => e.scrollTop);
  render();
  root.querySelectorAll(SCROLLERS).forEach((e, i) => (e.scrollTop = tops[i] ?? 0));
}

if (route.startsWith("card")) cardChrome();
void listen("state-changed", () => void refresh());
void listen<string>("route", (e) => {
  route = e.payload;
  render();
});
void refresh();
