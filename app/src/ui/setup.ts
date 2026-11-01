// GitHub setup (§6.2): guided steps, paste (never read from the clipboard), check, show, then save.
import { api } from "./api";
import { date, el } from "./dom";

export function renderSetup(root: HTMLElement) {
  const token = el("input", { type: "password", placeholder: "github_pat_…", style: "width:100%", autocomplete: "off", spellcheck: "false", "aria-label": "Token" });
  const result = el("div", { role: "status" });
  const check = el("button", { class: "primary" }, "Check token");

  check.addEventListener("click", async () => {
    result.replaceChildren(el("p", { class: "muted" }, "Checking with GitHub…"));
    check.disabled = true;
    try {
      const v = await api.tokenCheck(token.value);
      const save = el("button", { class: "primary" }, "Save");
      save.addEventListener("click", async () => {
        save.disabled = true;
        try {
          await api.tokenSave(token.value);
          token.value = "";
          result.replaceChildren(el("p", { class: "ok" }, "Saved to the Keychain. The first sync has started."), el("div", { class: "buttons" }, el("button", { class: "primary", onclick: () => void api.openWindow("adopt") }, "Adopt a pet…"), el("button", { onclick: () => void api.closeWindow("setup") }, "Done")));
        } catch (e) {
          result.append(el("p", { class: "error" }, String(e)));
          save.disabled = false;
        }
      });
      result.replaceChildren(
        el(
          "div",
          { class: "card" },
          el("p", {}, "Connected as ", el("b", {}, v.login), `. Sees ${v.repoCount} repos. Expires ${date(v.expires)}.`),
          el("p", { class: "muted" }, "It can't read code in private repos. Public repos are readable by anyone."),
          el("div", { class: "buttons" }, save, el("button", { onclick: () => result.replaceChildren() }, "Cancel")),
        ),
      );
    } catch (e) {
      result.replaceChildren(el("p", { class: "error" }, String(e)));
    } finally {
      check.disabled = false;
    }
  });

  root.replaceChildren(
    el(
      "div",
      { class: "page" },
      el("h1", {}, "Connect GitHub"),
      el("p", { class: "muted" }, "Strays only needs to know when your repos were last pushed to. Create a token that can see exactly that and nothing more."),
      el(
        "ol",
        {},
        el("li", {}, "Open GitHub's new fine-grained token page.", " ", el("button", { onclick: () => void api.openLink("token-page") }, "Open in browser")),
        el("li", {}, "Token name: ", el("b", {}, "Strays"), ". Expiration: your choice; Strays reminds you before it runs out."),
        el("li", {}, "Repository access: ", el("b", {}, "All repositories"), " lets Strays notice new projects. ", el("span", { class: "muted" }, "\"Only select repositories\" works too, without new-project detection.")),
        el("li", {}, "Permissions → Repository permissions → ", el("b", {}, "Metadata: Read-only"), ". Nothing else."),
        el("li", {}, "Generate the token, copy it, and paste it here."),
      ),
      token,
      el("div", { class: "buttons" }, check),
      result,
    ),
  );
  token.focus();
}
