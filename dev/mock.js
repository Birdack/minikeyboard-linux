// Browser-only emulation of the Tauri backend, for UI work. Not shipped.
// Like Rust it sends no labels (the UI builds them) and errors as {code, args}.
// Query: ?layout=15x3  ?state=connected|disconnected|permission|error  ?model=none (not identified)
//        ?lang=en|it|de|fr|es|pt (stored as if picked in the menu; default: browser languages)
//        ?scenario=step,step,...   steps (language independent unless noted):
//        read, layer:N, sel:N, knob:K:P (K 1-based, P 0-2), tab:keys|media|mouse|led (or its label),
//        vk:name (syntax name), mod:ctrl (syntax name), text:Label (click button by its text),
//        opt:N (LED mode 0-5, or its label), swatch:N, rec, modal:clearlayer, modal:perm, wait:ms,
//        lang:xx (switch from the menu), syntax:text (type into the syntax field, no commas)
(() => {
  const q = new URLSearchParams(location.search);
  if (q.get("lang")) try { localStorage.setItem("minikeyboard.lang", q.get("lang")); } catch {}
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  let cat = null;
  const loadCat = async () => cat || (cat = await (await fetch(new URL("catalog.json", document.currentScript?.src || location.href))).json());
  const listeners = {};
  const emit = (ev, payload) => (listeners[ev] || []).forEach((cb) => cb({ payload }));

  const keyByCode = (c) => cat.keys.find((k) => k.code === c);
  const modNames = (m) => cat.modifiers.filter(([b]) => m & b).map(([, n]) => n);
  const chordText = (m, c) => [...modNames(m), ...(c ? [keyByCode(c)?.name ?? `<${c}>`] : [])].join("-");
  const fail = (code, ...args) => { throw { code, args: args.map(String) }; };

  // the action plus its textual syntax, like Action::view()
  function describe(a) {
    let text = "";
    if (a.kind === "keys") text = a.chords.map(([m, c]) => chordText(m, c)).join(",");
    else if (a.kind === "media") text = cat.media.find((m) => m.code === a.media)?.name ?? `media<${a.media}>`;
    else if (a.kind === "mouse") {
      text = [...modNames(a.mouseMod), ...cat.mouseButtons.filter(([b]) => a.buttons & b).map(([, n]) => n),
        ...(a.wheel === 1 ? ["wheelup"] : a.wheel === 255 ? ["wheeldown"] : [])].join("-");
    }
    return { ...a, text };
  }

  function parse(text, delay) {
    const chords = [];
    for (const part of text.split(",").map((s) => s.trim()).filter(Boolean)) {
      let mods = 0, code = 0;
      const toks = part.toLowerCase().split("-");
      toks.forEach((t, i) => {
        const m = cat.modifiers.find(([, n]) => n === t);
        if (m && (i < toks.length - 1 || toks.length === 1)) mods |= m[0];
        else { const k = cat.keys.find((k) => k.name === t); if (!k) fail("unknown_key", t); code = k.code; }
      });
      chords.push([mods, code]);
    }
    if (chords.length > cat.maxChords) fail("too_many_chords", cat.maxChords);
    return describe({ kind: chords.length ? "keys" : "none", chords, delay: delay || 0, media: 0, buttons: 0, wheel: 0, mouseMod: 0 });
  }

  function pickLayout() {
    const spec = q.get("layout") || "12x2";
    if (spec === "0x1") return cat.layouts.find((l) => l.keys === 0 && l.knobs === 1);
    const [k, n] = spec.split("x").map(Number);
    return cat.layouts.find((l) => l.keys === k && l.knobs === n) || cat.layouts.find((l) => l.keys === 12 && l.knobs === 2);
  }

  const sample = [
    [["ctrl", "c"]], [["ctrl", "v"]], [["ctrl", "z"]], [["ctrl", "shift", "t"]], [["alt", "tab"]], [["win", "d"]],
    [["f5"]], [["ctrl", "s"]], [["ctrl", "a"], ["ctrl", "c"]], [["escape"]], [["enter"]], [["ctrl", "alt", "delete"]],
  ];
  function seedAction(layer, id) {
    if (id > 24 || (id + layer) % 5 === 0) return describe({ kind: "none", chords: [], delay: 0, media: 0, buttons: 0, wheel: 0, mouseMod: 0 });
    const base = { chords: [], delay: 0, media: 0, buttons: 0, wheel: 0, mouseMod: 0 };
    if (id >= 16) { // knobs
      const p = (id - 16) % 3;
      if (p === 0) return describe({ ...base, kind: "media", media: 0xea });
      if (p === 2) return describe({ ...base, kind: "media", media: 0xe9 });
      return describe({ ...base, kind: "media", media: 0xe2 });
    }
    if (id % 7 === 0) return describe({ ...base, kind: "mouse", buttons: 1 });
    const s = sample[(id + layer * 3) % sample.length];
    const chords = s.map((toks) => {
      let mods = 0, code = 0;
      toks.forEach((t) => { const m = cat.modifiers.find(([, n]) => n === t); if (m) mods |= m[0]; else code = cat.keys.find((k) => k.name === t).code; });
      return [mods, code];
    });
    return describe({ ...base, kind: "keys", chords, delay: id === 9 ? 200 : 0 });
  }

  const invoke = async (cmd, args = {}) => {
    await loadCat();
    switch (cmd) {
      case "catalog": return cat;
      case "describe": return describe(args.action);
      case "parse_action": return parse(args.text, args.delay);
      case "poll": {
        const st = q.get("state") || "connected";
        if (st === "connected") return { state: "connected", node: { name: "Mini Keyboard", vid: 0x1189, pid: 0x8890, path: "/dev/hidraw3" }, info: null, layout: q.get("model") === "none" ? null : pickLayout(), error: null, demo: q.get("demo") !== "0" };
        if (st === "permission") return { state: "permission", node: { name: "Mini Keyboard", vid: 0x1189, pid: 0x8890, path: "/dev/hidraw3" }, info: null, layout: null, error: null, demo: false };
        if (st === "error") return { state: "error", node: null, info: null, layout: null, error: { code: "io", args: ["write: Broken pipe (os error 32)"] }, demo: false };
        return { state: "disconnected", node: null, info: null, layout: null, error: null, demo: false };
      }
      case "read_config": {
        const ids = [];
        for (let i = 1; i <= args.layout.keys; i++) ids.push(i);
        args.layout.groups.forEach((g) => g.forEach((id) => id != null && ids.push(id)));
        const total = ids.length * 3, out = [];
        let done = 0;
        for (let layer = 1; layer <= 3; layer++) for (const id of ids) {
          out.push({ layer, keyId: id, action: seedAction(layer, id) });
          if (++done % 6 === 0) { emit("progress", [done, total]); await sleep(15); }
        }
        emit("progress", [total, total]);
        return out;
      }
      case "write_changes": {
        const total = (args.keys?.length || 0) + (args.leds?.length || 0);
        for (let i = 1; i <= total; i++) { emit("progress", [i, total]); await sleep(20); }
        return null;
      }
      case "open_profile": case "save_profile": case "reconnect": return null;
      default: return fail("internal", `unknown command ${cmd}`);
    }
  };

  window.__TAURI__ = {
    core: { invoke },
    event: { listen: async (ev, cb) => { (listeners[ev] ||= []).push(cb); return () => {}; } },
  };

  // ---- scenarios -----------------------------------------------------------
  const $$ = (s) => [...document.querySelectorAll(s)];
  const click = (el) => { if (!el) throw new Error("scenario: element not found"); el.click(); };
  const byText = (sel, text) => $$(sel).find((b) => b.textContent.trim().toLowerCase().startsWith(text.toLowerCase()));
  const one = (sel) => document.querySelector(sel);
  const steps = {
    read: async () => { click(one("#btn-read")); await sleep(900); },
    layer: (n) => click(one(`.stage-head [data-layer="${n}"]`)),
    sel: (n) => click(one(`.cap[data-id="${n}"]`)),
    knob: (k, p) => click($$(".knob")[k - 1]?.querySelector(`[data-part="${p}"]`)),
    tab: (name) => click(one(`.insp-tabs [data-tab="${name}"]`) || byText(".insp-tabs button", name)),
    vk: (name) => click($$(".vk button").find((b) => b.title.startsWith(`${name} (`))),
    mod: (name) => click(one(`.toggle[data-mod="${name}"]`) || byText(".toggle", name)),
    text: (t) => click(byText("button", t)),
    opt: (t) => click(one(`.option[data-mode="${t}"]`) || byText(".option", t)),
    swatch: (n) => click($$(".swatch")[Number(n)]),
    rec: () => click(one(".rec")),
    modal: (w) => click(one(w === "perm" ? '[data-action="perm"]' : ".insp-foot .danger")),
    wait: (ms) => sleep(Number(ms)),
    lang: (code) => { const el = one("#lang"); el.value = code; el.dispatchEvent(new Event("change")); },
    syntax: (t) => { // type into the textual syntax field and keep focus there
      const el = one(".inspector .text-input");
      el.closest("details").open = true;
      el.focus();
      el.value = t;
      el.dispatchEvent(new Event("input"));
    },
  };
  const sc = q.get("scenario");
  if (sc) {
    window.addEventListener("load", async () => {
      try {
        await sleep(500);
        for (const s of sc.split(",")) {
          const [name, ...args] = s.split(":");
          await steps[name](...args);
          await sleep(120);
        }
      } catch (e) { console.error(e); document.title = "SCENARIO FAILED: " + e.message; }
    });
  }
})();
