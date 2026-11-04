// Tiny DOM helpers: the windows are small enough not to need a framework.
type Attrs = Record<string, string | number | boolean | ((e: Event) => void) | undefined | null>;
type Child = Node | string | number | false | null | undefined;

/** `el("button", { class: "primary", onclick: … }, "Save")`. `false`/`null` attrs and children are skipped. */
export function el<K extends keyof HTMLElementTagNameMap>(tag: K, attrs: Attrs = {}, ...children: Child[]): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v === false || v == null) continue;
    if (typeof v === "function") node.addEventListener(k.replace(/^on/, ""), v);
    else if (k === "value" && "value" in node) (node as HTMLInputElement).value = String(v);
    else if (k === "checked" && "checked" in node) (node as HTMLInputElement).checked = Boolean(v);
    else node.setAttribute(k, v === true ? "" : String(v));
  }
  for (const c of children) if (c !== false && c != null) node.append(c instanceof Node ? c : String(c));
  return node;
}

export const SPECIES_ICON: Record<string, string> = { cat: "🐈", dog: "🐕", hamster: "🐹", parrot: "🦜" };
export const MOOD_ICON: Record<string, string> = { Joyful: "😺", Content: "🙂", Peckish: "🥣", Hungry: "😿", Gloomy: "🌧" };
export const PACE_LABEL: Record<string, string> = { sprint: "Sprint", normal: "Normal", slowBurn: "Slow-burn" };

export function ago(iso: string): string {
  const mins = Math.max(0, Math.round((Date.now() - Date.parse(iso)) / 60000));
  if (mins < 60) return `${mins} min ago`;
  if (mins < 48 * 60) return `${Math.round(mins / 60)} h ago`;
  return `${Math.round(mins / (24 * 60))} days ago`;
}

export function date(iso: string | null): string {
  return iso ? new Date(iso).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" }) : "never";
}

/** A labelled settings row. */
export function row(label: string, ...controls: Child[]): HTMLElement {
  return el("div", { class: "row" }, el("label", {}, label), ...controls);
}

/** A button that asks "Sure?" on the first click and acts on the second, so no modal dialog steals focus. */
export function confirmButton(text: string, sure: string, act: () => void, cls = ""): HTMLButtonElement {
  let armed = false;
  // the "click again" text is announced, so screen-reader users know a second click is needed
  const b = el("button", { class: cls, "aria-live": "polite" }, text);
  b.addEventListener("click", () => {
    if (!armed) {
      armed = true;
      b.textContent = sure;
      setTimeout(() => {
        armed = false;
        b.textContent = text;
      }, 4000);
      return;
    }
    act();
  });
  return b;
}
