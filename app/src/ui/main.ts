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
  const n = document.getElementById("notice");
  if (!n) return;
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

async function refresh() {
  state = await api.state();
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
