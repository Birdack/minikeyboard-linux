// MINI KEYBOARD – user interface. Protocol, files and device access live in Rust;
// every text shown comes from i18n.js.

import { LANGS, detectLang, getLang, hasKey, setLang, t } from "./i18n.js";

const T = window.__TAURI__;
const invoke = (cmd, args) => T.core.invoke(cmd, args);

// ---------------------------------------------------------------------------
// tiny DOM helpers
// ---------------------------------------------------------------------------
const $ = (sel) => document.querySelector(sel);

function h(tag, attrs = {}, ...children) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v == null || v === false) continue;
    if (k === "class") el.className = v;
    else if (k.startsWith("on")) el.addEventListener(k.slice(2), v);
    else if (k === "style") el.style.cssText = v;
    else el.setAttribute(k, v === true ? "" : v);
  }
  for (const c of children.flat(Infinity)) {
    if (c == null || c === false) continue;
    el.append(c instanceof Node ? c : document.createTextNode(String(c)));
  }
  return el;
}

const ICONS = {
  new: '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5M12 11v6M9 14h6"/>',
  open: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
  save: '<path d="M5 3h11l3 3v13a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z"/><path d="M7 3v5h8V3M7 21v-7h10v7"/>',
  read: '<path d="M12 4v11M7 10l5 5 5-5M5 20h14"/>',
  write: '<path d="M12 20V9M7 14l5-5 5 5M5 4h14"/>',
  keys: '<rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M7 14h10"/>',
  media: '<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>',
  mouse: '<rect x="6" y="3" width="12" height="18" rx="6"/><path d="M12 7v4"/>',
  led: '<path d="M9 18h6M10 22h4M12 2a7 7 0 0 0-4 12.7V17h8v-2.3A7 7 0 0 0 12 2z"/>',
  trash: '<path d="M3 6h18M8 6V4h8v2M6 6l1 15h10l1-15"/>',
  x: '<path d="M18 6 6 18M6 6l12 12"/>',
  check: '<path d="M20 6 9 17l-5-5"/>',
  alert: '<path d="M12 9v4M12 17h.01"/><path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"/>',
  info: '<circle cx="12" cy="12" r="10"/><path d="M12 16v-4M12 8h.01"/>',
  ccw: '<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/>',
  cw: '<path d="M21 12a9 9 0 1 1-3-6.7L21 8"/><path d="M21 3v5h-5"/>',
  plug: '<path d="M9 2v6M15 2v6M6 8h12v4a6 6 0 0 1-12 0zM12 18v4"/>',
  clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
  play: '<path d="M7 4v16l13-8z"/>',
  stop: '<rect x="6" y="6" width="12" height="12" rx="1"/>',
  previous: '<path d="M19 20 9 12l10-8zM5 19V5"/>',
  next: '<path d="m5 4 10 8-10 8zM19 5v14"/>',
  mute: '<path d="M11 5 6 9H2v6h4l5 4zM22 9l-6 6M16 9l6 6"/>',
  volumedown: '<path d="M11 5 6 9H2v6h4l5 4zM15.5 8.5a5 5 0 0 1 0 7"/>',
  volumeup: '<path d="M11 5 6 9H2v6h4l5 4zM15.5 8.5a5 5 0 0 1 0 7M19 5a9 9 0 0 1 0 14"/>',
  screenbrightnessdown: '<circle cx="12" cy="12" r="4"/><path d="M12 3v1M12 20v1M3 12h1M20 12h1"/>',
  screenbrightnessup: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
  calculator: '<rect x="5" y="2" width="14" height="20" rx="2"/><path d="M8 6h8M8 11h.01M12 11h.01M16 11h.01M8 15h.01M12 15h.01M16 15h.01M8 19h8"/>',
  mail: '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="m3 7 9 6 9-6"/>',
  explorer: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
  screenlock: '<rect x="4" y="11" width="16" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>',
  favorites: '<path d="m12 3 2.8 5.7 6.2.9-4.5 4.4 1 6.2L12 17.3 6.5 20.2l1-6.2L3 9.6l6.2-.9z"/>',
  search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
  webpagehome: '<path d="M3 11 12 3l9 8v10h-6v-6H9v6H3z"/>',
  webpageback: '<path d="M19 12H5M11 18l-6-6 6-6"/>',
  webpageforward: '<path d="M5 12h14M13 6l6 6-6 6"/>',
  webpagerefresh: '<path d="M21 12a9 9 0 1 1-2.6-6.4L21 8M21 3v5h-5"/>',
};

function icon(name) {
  const tpl = document.createElement("template");
  tpl.innerHTML = `<svg class="i" viewBox="0 0 24 24" aria-hidden="true">${ICONS[name] || ICONS.info}</svg>`;
  return tpl.content.firstChild;
}

// ---------------------------------------------------------------------------
// state
// ---------------------------------------------------------------------------
const LAYERS = 3;
const KNOB_PARTS = [{ icon: "ccw", key: "ccw" }, { icon: null, key: "press" }, { icon: "cw", key: "cw" }];
const partName = (p) => t(`knob.${KNOB_PARTS[p].key}`);
const LED_MODES = [0, 1, 2, 3, 4, 5]; // names led.modeN, descriptions led.descN (mode 0 has none)
const LED_COLORS = [
  [1, "red", "#f2555a"], [2, "orange", "#f7893b"], [3, "yellow", "#f5c84c"], [4, "green", "#3ecf8e"],
  [5, "cyan", "#2ec5d3"], [6, "blue", "#4b8bff"], [7, "purple", "#a970ff"],
];
const WHEEL_UP = 0x01;
const WHEEL_DOWN = 0xff;
const NONE = { kind: "none", chords: [], delay: 0, media: 0, buttons: 0, wheel: 0, mouseMod: 0, text: "" };

const S = {
  cat: null,
  layout: null,
  detected: null,
  modelChoice: "auto",
  layer: 1,
  sel: null,
  tab: "keys",
  layers: [new Map(), new Map(), new Map()], // key id -> action view
  leds: [null, null, null], // {mode, color}
  dirty: new Set(), // "layer:id", id 0 = LED
  status: { state: "disconnected" },
  busy: false,
  recording: false,
  mods: 0,
  held: 0,
  syntaxError: null, // {code, args} from Rust, translated when shown
  permissionShown: false,
  pollSig: "", // last device status shown
  inspStale: false, // inspector not rebuilt because the user is typing in it
  open: { more: false, syntax: false }, // expanded <details> in the inspector
};

// edit counters per "layer:id": syntax results and finished writes only apply to an unchanged key
let editSeq = 0;
const edits = new Map();
const bump = (k) => { edits.set(k, ++editSeq); return editSeq; };

const keyOf = (layer, id) => `${layer}:${id}`;
const get = (layer, id) => S.layers[layer - 1].get(id) || NONE;
const strip = (a) => ({ kind: a.kind, chords: a.chords, delay: a.delay, media: a.media, buttons: a.buttons,
  wheel: a.wheel, mouseMod: a.mouseMod });

function keyIds(layout) {
  if (!layout) return [];
  const ids = [];
  for (let i = 1; i <= layout.keys; i++) ids.push(i);
  for (const g of layout.groups) for (const id of g) if (id != null) ids.push(id);
  return ids;
}

function knobPart(layout, id) {
  for (let k = 0; k < layout.groups.length; k++) {
    const p = layout.groups[k].indexOf(id);
    if (p >= 0) return [k, p];
  }
  return null;
}

function keyTitle(id) {
  const kp = S.layout && knobPart(S.layout, id);
  return kp ? `${t("stage.knob", { n: kp[0] + 1 })} · ${partName(kp[1])}` : t("stage.key", { n: id });
}

// ---------------------------------------------------------------------------
// labels: built here from names and codes, Rust only sends the syntax
// ---------------------------------------------------------------------------
// keys whose cap is a symbol; letters, digits and F-keys show their name in capitals
const SYMBOLS = { minus: "-", equal: "=", leftbracket: "[", rightbracket: "]", backslash: "\\", nonushash: "#",
  semicolon: ";", quote: "'", grave: "`", comma: ",", dot: ".", slash: "/", nonusbackslash: "<>", right: "→",
  left: "←", down: "↓", up: "↑", numpadslash: "/", numpadasterisk: "*", numpadminus: "-", numpadplus: "+",
  numpaddot: ".", numpadequal: "=" };

function keyNameLabel(name) {
  if (hasKey(`cap.${name}`)) return t(`cap.${name}`);
  const pad = name.match(/^numpad(.+)$/);
  if (pad) return t("cap.numpad", { key: pad[1] === "enter" ? t("cap.enter") : SYMBOLS[name] ?? pad[1] });
  return SYMBOLS[name] ?? name.toUpperCase();
}

function keyLabel(code) {
  const k = S.cat.keys.find((k) => k.code === code);
  return k ? keyNameLabel(k.name) : `#${code}`;
}

const modLabels = (mods) => S.cat.modifiers.filter(([bit]) => mods & bit).map(([, name]) => t(`mod.${name}`));

function mediaLabel(code) {
  const m = S.cat.media.find((m) => m.code === code);
  return m ? t(`media.${m.name}`) : t("media.unknown", { code: code.toString(16).toUpperCase().padStart(3, "0") });
}

/** Short description in the current language: "Ctrl+C, Ctrl+V", "Volume +", "Ctrl+Wheel ↑". */
function actionLabel(a) {
  if (a.kind === "keys") return a.chords.map(([m, c]) => [...modLabels(m), ...(c ? [keyLabel(c)] : [])].join("+")).join(", ");
  if (a.kind === "media") return mediaLabel(a.media);
  if (a.kind === "mouse") {
    const parts = [...modLabels(a.mouseMod), ...S.cat.mouseButtons.filter(([bit]) => a.buttons & bit).map(([, name]) => t(`mouse.${name}`))];
    if (a.wheel === WHEEL_UP) parts.push(t("mouse.wheelup"));
    if (a.wheel === WHEEL_DOWN) parts.push(t("mouse.wheeldown"));
    return parts.join("+");
  }
  return "";
}

function layoutLabel(l) {
  if (!l) return "";
  if (l.kind === "knob_wheel") return t("layout.knob_wheel");
  const keys = t("layout.keys", { n: l.keys });
  const knobs = t("layout.knobs", { n: l.knobs });
  if (!l.keys) return t("layout.knobs_only", { knobs });
  return l.knobs ? t("layout.keys_knobs", { keys, knobs }) : keys;
}

/** A Rust error {code, args, key?} in the current language; unknown codes show code and args. */
function errText(e) {
  if (!e || typeof e !== "object" || !e.code) return String(e ?? "");
  const args = e.args || [];
  const msg = hasKey(`err.${e.code}`) ? t(`err.${e.code}`, { ...args }) : `${e.code}${args.length ? ` (${args.join(", ")})` : ""}`;
  return e.key != null ? t("err.in_key", { key: e.key, msg }) : msg;
}

const sameLayout = (a, b) => !!a && !!b && a.keys === b.keys && a.knobs === b.knobs &&
  JSON.stringify(a.groups) === JSON.stringify(b.groups);

function pending() {
  const ids = new Set([0, ...keyIds(S.layout)]);
  let n = 0;
  for (const k of S.dirty) if (ids.has(Number(k.split(":")[1]))) n++;
  return n;
}

function layerDirty(layer) {
  const ids = new Set([0, ...keyIds(S.layout)]);
  for (const k of S.dirty) {
    const [l, id] = k.split(":").map(Number);
    if (l === layer && ids.has(id)) return true;
  }
  return false;
}

/** Move to another model keeping keys by number and knobs by (knob, part). */
function relayout(layout) {
  const old = S.layout;
  const target = (id) => {
    if (id === 0 || !old) return id;
    const kp = knobPart(old, id);
    if (!kp) return id <= layout.keys ? id : null;
    const g = layout.groups[kp[0]];
    return g ? g[kp[1]] : null;
  };
  S.layers = S.layers.map((m) => {
    const out = new Map();
    for (const [id, a] of m) {
      const to = target(id);
      if (to != null) out.set(to, a);
    }
    return out;
  });
  const dirty = new Set();
  for (const k of S.dirty) {
    const [l, id] = k.split(":").map(Number);
    const to = target(id);
    if (to != null) dirty.add(keyOf(l, to));
  }
  S.dirty = dirty;
  S.layout = layout;
  edits.clear();
  fixSel();
}

function fixSel() {
  if (S.sel != null && !keyIds(S.layout).includes(S.sel)) S.sel = keyIds(S.layout)[0] ?? null;
}

function markAllDirty() {
  for (let l = 1; l <= LAYERS; l++) {
    for (const id of keyIds(S.layout)) S.dirty.add(keyOf(l, id));
    if (S.leds[l - 1]) S.dirty.add(keyOf(l, 0));
  }
}

// ---------------------------------------------------------------------------
// toasts, modals, progress
// ---------------------------------------------------------------------------
function toast(text, kind = "ok") {
  const el = h("div", { class: `toast ${kind}` }, icon(kind === "ok" ? "check" : kind === "err" ? "alert" : "info"), text);
  $("#toasts").append(el);
  setTimeout(() => el.remove(), kind === "err" ? 6000 : 3200);
}

// one dialog at a time: later ones wait for the open one to close
let modalQueue = Promise.resolve();
function modal(opts) {
  const shown = modalQueue.then(() => showModal(opts));
  modalQueue = shown.catch(() => {});
  return shown;
}

function showModal({ title, body, actions }) {
  return new Promise((resolve) => {
    const root = $("#modal-root");
    const close = (v) => {
      root.replaceChildren();
      document.removeEventListener("keydown", onKey, true);
      resolve(v);
    };
    const onKey = (e) => {
      if (e.key === "Escape") { e.preventDefault(); close(null); }
    };
    document.addEventListener("keydown", onKey, true);
    const back = h("div", { class: "modal-back", onmousedown: (e) => e.target === back && close(null) },
      h("div", { class: "modal", role: "dialog" },
        h("h3", {}, title),
        ...(Array.isArray(body) ? body : [body]),
        h("div", { class: "actions" },
          actions.map((a) => h("button", { class: `btn ${a.kind || ""}`, onclick: () => close(a.value) }, a.label)))));
    root.replaceChildren(back);
    back.querySelector(".btn.primary, .btn.danger")?.focus();
  });
}

const confirm = (title, text, label = t("dlg.continue"), kind = "primary") =>
  modal({ title, body: h("p", {}, text), actions: [{ label: t("dlg.cancel"), kind: "ghost", value: false }, { label, kind, value: true }] });

function showPermissionHelp() {
  return modal({
    title: t("dlg.perm_title"),
    body: [
      h("p", {}, t("dlg.perm_body")),
      h("pre", {}, "sudo install -m 644 data/70-minikeyboard.rules /etc/udev/rules.d/\nsudo udevadm control --reload\nsudo udevadm trigger"),
      h("p", { class: "hint" }, t("dlg.perm_pkg")),
    ],
    actions: [{ label: t("dlg.got_it"), kind: "primary", value: true }],
  });
}

function setProgress(done, total) {
  const bar = $("#progress");
  bar.classList.add("on");
  bar.style.width = `${Math.round((100 * done) / Math.max(total, 1))}%`;
}

async function busy(fn) {
  S.busy = true;
  stopRecording();
  setProgress(0, 1);
  render();
  try {
    return await fn();
  } finally {
    S.busy = false;
    const bar = $("#progress");
    bar.style.width = "100%";
    setTimeout(() => { bar.classList.remove("on"); bar.style.width = "0"; }, 350);
    render();
  }
}

// ---------------------------------------------------------------------------
// editing
// ---------------------------------------------------------------------------
/** Store a structured action now; its text comes from Rust and only replaces this very placeholder. */
async function setAction(action, layer = S.layer, id = S.sel) {
  if (id == null) return;
  const k = keyOf(layer, id);
  bump(k);
  const a = strip({ ...NONE, ...action });
  const placeholder = a.kind === "none" ? NONE : { ...a, text: "" };
  S.layers[layer - 1].set(id, placeholder);
  S.dirty.add(k);
  S.syntaxError = null;
  render();
  if (placeholder === NONE) return;
  let view;
  try {
    view = await invoke("describe", { action: a });
  } catch (e) {
    toast(errText(e), "err");
    return;
  }
  // gone if a newer edit, a read or a profile replaced it; relayout may have moved it to another id
  for (const m of S.layers) for (const [i, v] of m) if (v === placeholder) m.set(i, view);
  softRender();
}

function currentChords() {
  const a = get(S.layer, S.sel);
  return a.kind === "keys" ? a.chords : [];
}

function addChord(mods, code) {
  const chords = currentChords();
  if (chords.length >= S.cat.maxChords) {
    toast(t("keys.max", { n: S.cat.maxChords }), "err");
    return;
  }
  const a = get(S.layer, S.sel);
  S.mods = 0; // the toggles apply to one chord
  setAction({ kind: "keys", chords: [...chords, [mods, code]], delay: a.kind === "keys" ? a.delay : 0 });
}

function removeChord(i) {
  const a = get(S.layer, S.sel);
  const chords = a.chords.filter((_, j) => j !== i);
  setAction(chords.length ? { kind: "keys", chords, delay: a.delay } : NONE);
}

function clickKey(code) {
  addChord(S.mods, code);
}

/** Another layer, same key; the LED tab stays (it is per layer). */
function switchLayer(l) {
  S.layer = l;
  if (S.tab !== "led") return select(S.sel);
  stopRecording();
  S.syntaxError = null;
  render();
}

function toggleMod(bit) {
  S.mods ^= bit;
  render();
}

function select(id) {
  stopRecording();
  S.sel = id;
  S.syntaxError = null;
  const kind = get(S.layer, id).kind;
  if (kind === "media") S.tab = "media";
  else if (kind === "mouse") S.tab = "mouse";
  else if (kind === "keys" || S.tab === "led") S.tab = "keys";
  render();
}

// recording from the physical keyboard (layout independent: KeyboardEvent.code)
const MOD_CODES = { ControlLeft: 0x01, ShiftLeft: 0x02, AltLeft: 0x04, MetaLeft: 0x08,
  ControlRight: 0x10, ShiftRight: 0x20, AltRight: 0x40, MetaRight: 0x80 };
const MEDIA_CODES = { AudioVolumeUp: 0xe9, AudioVolumeDown: 0xea, AudioVolumeMute: 0xe2, MediaPlayPause: 0xcd,
  MediaTrackNext: 0xb5, MediaTrackPrevious: 0xb6, MediaStop: 0xb7 };

function onRecordKey(e) {
  if (!S.recording || $("#modal-root").childElementCount) return;
  e.preventDefault();
  e.stopPropagation();
  const bit = MOD_CODES[e.code];
  if (e.type === "keyup") {
    if (bit) S.held &= ~bit;
    return;
  }
  if (bit) { S.held |= bit; return; }
  if (e.repeat) return;
  // forget modifiers released while the window was not focused
  if (!e.ctrlKey) S.held &= ~0x11;
  if (!e.shiftKey) S.held &= ~0x22;
  if (!e.altKey) S.held &= ~0x04;
  if (!e.metaKey) S.held &= ~0x88;
  if (!e.getModifierState("AltGraph")) S.held &= ~0x40;
  if (MEDIA_CODES[e.code]) {
    setAction({ kind: "media", media: MEDIA_CODES[e.code] });
    return;
  }
  const key = S.cat.keys.find((k) => k.dom === e.code);
  if (key) addChord(S.held, key.code);
  else toast(t("keys.unknown"), "info");
}

function startRecording() {
  if (S.sel == null) return;
  S.recording = true;
  S.held = 0;
  document.activeElement?.blur();
  render();
}

function stopRecording() {
  if (!S.recording) return;
  S.recording = false;
  render();
}

window.addEventListener("keydown", onRecordKey, true);
window.addEventListener("keyup", onRecordKey, true);
window.addEventListener("blur", () => { S.held = 0; });

// ---------------------------------------------------------------------------
// device
// ---------------------------------------------------------------------------
async function poll() {
  if (S.busy) return;
  let st;
  try {
    st = await invoke("poll");
  } catch (e) {
    st = { state: "error", error: e };
  }
  if (st.state === "busy" || S.busy) return;
  const wasConnected = S.status.state === "connected";
  S.status = st;
  const sig = JSON.stringify([st.state, st.node, st.layout, st.error, st.demo]);
  if (sig === S.pollSig) { // nothing new: keep the UI (and what is being typed) as is
    if (S.inspStale && !$("#inspector").contains(document.activeElement)) renderInspector();
    return;
  }
  S.pollSig = sig;
  if (st.state === "connected") {
    S.detected = st.layout;
    if (st.layout && S.modelChoice === "auto" && !sameLayout(S.layout, st.layout)) relayout(st.layout);
    if (!wasConnected) toast(t(st.demo ? "msg.demo_connected" : "msg.connected"));
  } else {
    if (wasConnected) toast(t("msg.disconnected"), "info");
    S.detected = null;
  }
  if (st.state === "permission" && !S.permissionShown) {
    S.permissionShown = true;
    stopRecording();
    showPermissionHelp();
  }
  softRender();
}

async function reconnect() {
  if (S.busy) return;
  try {
    await invoke("reconnect");
  } catch (e) {
    toast(errText(e), "err");
    return;
  }
  S.permissionShown = false;
  S.pollSig = ""; // show the outcome even if nothing changed
  await poll();
}

async function readDevice() {
  if (S.status.state !== "connected" || S.busy) return;
  if (pending() && !(await confirm(t("stage.unwritten"), t("dlg.read_body"), t("dlg.read_anyway")))) return;
  if (S.status.state !== "connected" || S.busy) return; // changed while the dialog was open
  const layout = S.layout;
  await busy(async () => {
    try {
      const keys = await invoke("read_config", { layout });
      if (!keys.length) {
        await modal({ title: t("dlg.read_unsupported"), body: h("p", {}, t("dlg.read_unsupported_body")), actions: [{ label: t("dlg.ok"), kind: "primary", value: true }] });
        return;
      }
      S.layers = [new Map(), new Map(), new Map()];
      edits.clear();
      for (const k of keys) S.layers[k.layer - 1].set(k.keyId, k.action);
      // LEDs cannot be read back: keep local values and their unsent state
      S.dirty = new Set([...S.dirty].filter((k) => k.endsWith(":0")));
      const missing = LAYERS * keyIds(layout).length - keys.length;
      toast(missing ? t("msg.read_partial", { n: missing }) : t("msg.read"));
    } catch (e) {
      toast(t("msg.read_failed", { error: errText(e) }), "err");
    }
  });
}

async function writeDevice() {
  if (S.status.state !== "connected" || S.busy || !pending()) return;
  const ids = new Set(keyIds(S.layout));
  const sent = [];
  const keys = [];
  const leds = [];
  for (const k of S.dirty) {
    const [layer, id] = k.split(":").map(Number);
    if (id === 0 && S.leds[layer - 1]) {
      leds.push({ layer, ...S.leds[layer - 1] });
      sent.push([k, edits.get(k)]);
    } else if (ids.has(id)) {
      keys.push({ layer, keyId: id, action: strip(get(layer, id)) });
      sent.push([k, edits.get(k)]);
    }
  }
  await busy(async () => {
    try {
      await invoke("write_changes", { keys, leds });
      for (const [k, n] of sent) if (edits.get(k) === n) S.dirty.delete(k); // edited during the write: still dirty
      toast(t("msg.written"));
    } catch (e) {
      toast(t("msg.write_failed", { error: errText(e) }), "err");
    }
  });
}

// ---------------------------------------------------------------------------
// profiles
// ---------------------------------------------------------------------------
async function newProfile() {
  if (pending() && !(await confirm(t("top.new"), t("dlg.lost_body"), t("dlg.discard")))) return;
  S.layers = [new Map(), new Map(), new Map()];
  S.leds = [null, null, null];
  S.dirty = new Set();
  edits.clear();
  render();
}

async function openProfile() {
  if (S.busy) return;
  try {
    const p = await invoke("open_profile", { title: t("file.open"), filter: t("file.filter") });
    if (!p) return;
    if (pending() && !(await confirm(t("file.open"), t("dlg.lost_body"), t("dlg.discard")))) return;
    if (S.busy) return;
    S.layers = p.layers.map((obj) => new Map(Object.entries(obj).map(([k, v]) => [Number(k), v])));
    S.leds = p.leds;
    S.layout = p.layout;
    S.dirty = new Set();
    edits.clear();
    if (S.detected && !sameLayout(S.detected, p.layout)) {
      toast(t("msg.profile_adapted", { from: layoutLabel(p.layout), to: layoutLabel(S.detected) }), "info");
      relayout(S.detected);
    } else {
      toast(t("msg.profile_loaded"));
    }
    // after relayout: keys the profile does not cover are cleared on write
    markAllDirty();
    fixSel();
    S.modelChoice = "auto";
    render();
  } catch (e) {
    toast(errText(e), "err");
  }
}

async function saveProfile() {
  const layers = S.layers.map((m) => Object.fromEntries([...m].filter(([, a]) => a.kind !== "none").map(([id, a]) => [id, strip(a)])));
  try {
    const path = await invoke("save_profile", { profile: { layout: S.layout, layers, leds: S.leds },
      title: t("file.save"), filter: t("file.filter"), fileName: "minikeyboard.json" });
    if (path) toast(t("msg.profile_saved", { name: path.split("/").pop() }));
  } catch (e) {
    toast(errText(e), "err");
  }
}

// ---------------------------------------------------------------------------
// rendering: top bar
// ---------------------------------------------------------------------------
function renderTopbar() {
  const st = S.status;
  const pill = $("#device");
  pill.className = `device-pill ${st.state}`;
  const name = pill.querySelector(".name");
  const meta = pill.querySelector(".meta");
  if (st.state === "connected") {
    name.textContent = st.demo ? t("device.demo") : (st.node?.name || t("device.keyboard"));
    const hex = (n) => n.toString(16).padStart(4, "0").toUpperCase();
    meta.textContent = [layoutLabel(S.detected) || t("device.no_model"),
      st.demo ? t("device.simulated") : `${hex(st.node.vid)}:${hex(st.node.pid)}`].join(" · ");
  } else if (st.state === "permission") {
    name.textContent = t("device.permission");
    meta.textContent = st.node?.path || "";
  } else if (st.state === "error") {
    name.textContent = t("device.error");
    meta.textContent = errText(st.error);
  } else {
    name.textContent = t("device.none");
    meta.textContent = t("device.waiting");
  }

  const sel = $("#model");
  if (sel.dataset.lang !== getLang()) { // option texts follow the language
    sel.dataset.lang = getLang();
    sel.replaceChildren(h("option", { value: "auto" }, t("top.model_auto")),
      ...S.cat.layouts.map((l, i) => h("option", { value: String(i) }, layoutLabel(l))));
  }
  sel.value = S.modelChoice;
  sel.disabled = S.busy;
  sel.title = t("top.model");
  $("#lang").value = getLang();
  $("#lang").title = t("top.lang");
  $("#btn-new").title = t("top.new");
  $("#btn-open").title = t("top.open");
  $("#btn-save").title = t("top.save");

  const connected = st.state === "connected";
  const r = $("#btn-read");
  r.disabled = !connected || S.busy;
  r.replaceChildren(icon("read"), t("top.read"));
  const n = pending();
  const w = $("#btn-write");
  w.disabled = !connected || S.busy || !n;
  w.replaceChildren(icon("write"), t("top.write"), ...(n ? [h("span", { class: "badge" }, n)] : []));
  for (const id of ["#btn-new", "#btn-open", "#btn-save"]) $(id).disabled = S.busy;
}

// ---------------------------------------------------------------------------
// rendering: stage (the device)
// ---------------------------------------------------------------------------
function grid(n) {
  if (n <= 5) return [1, Math.max(n, 1)];
  for (const c of [3, 4, 5, 7]) if (n === c * 2 || n === c * 3 || (c === 4 && n === 16)) return [Math.ceil(n / c), c];
  return [Math.ceil(n / 4), 4];
}

const KIND_ICON = { keys: null, media: "media", mouse: "mouse" };

function capContent(a, emptyText = "—") {
  return a.kind === "none"
    ? h("span", { class: "lbl empty" }, emptyText)
    : h("span", { class: "lbl" }, ...(actionLabel(a) + (a.delay ? " ⏱" : "")).split("+").flatMap((p, i) => (i ? ["+", h("wbr"), p] : [p])));
}

function renderStage() {
  const stage = $("#stage");
  const layout = S.layout;
  const layerBtns = [];
  for (let l = 1; l <= LAYERS; l++) {
    layerBtns.push(h("button", { class: l === S.layer ? "on" : "", "data-layer": l, onclick: () => switchLayer(l) },
      t("stage.layer", { n: l }), layerDirty(l) ? h("span", { class: "pip", title: t("stage.unwritten") }) : null));
  }

  const banner = (() => {
    const st = S.status;
    if (st.state === "permission") {
      return h("div", { class: "banner warn" }, icon("alert"),
        h("span", { class: "grow" }, t("banner.permission")),
        h("button", { class: "btn sm", "data-action": "perm", onclick: showPermissionHelp }, t("banner.how_fix")));
    }
    if (st.state === "connected" && !S.detected) {
      return h("div", { class: "banner warn" }, icon("alert"),
        h("span", { class: "grow" }, t("banner.no_model")),
        h("button", { class: "btn sm", onclick: reconnect }, t("banner.reconnect")));
    }
    if (st.state === "error") {
      return h("div", { class: "banner warn" }, icon("alert"),
        h("span", { class: "grow" }, t("banner.error")),
        h("button", { class: "btn sm", onclick: reconnect }, t("banner.reconnect")));
    }
    if (st.state !== "connected") {
      return h("div", { class: "banner" }, icon("plug"),
        h("span", { class: "grow" }, t("banner.disconnected")));
    }
    return null;
  })();

  const isSel = (id) => (id === S.sel ? " sel" : "");
  const isDirty = (id) => (S.dirty.has(keyOf(S.layer, id)) ? " dirty" : "");

  const device = h("div", { class: "device" });
  if (layout.keys) {
    const [, cols] = grid(layout.keys);
    if (cols > 5) device.classList.add("stack"); // wide pads: knobs go under the keys
    const keys = h("div", { class: "keys", style: `--cols: ${cols}` });
    for (let id = 1; id <= layout.keys; id++) {
      const a = get(S.layer, id);
      const label = actionLabel(a);
      keys.append(h("button", { class: `cap${isSel(id)}${isDirty(id)}`, "data-id": id, title: `${t("stage.key", { n: id })}${label ? `: ${label}` : ""}`, onclick: () => select(id) },
        h("span", { class: "id" }, `K${id}`), capContent(a),
        KIND_ICON[a.kind] ? h("span", { class: "kind" }, icon(KIND_ICON[a.kind])) : null));
    }
    device.append(keys);
  }
  if (layout.groups.length) {
    const knobs = h("div", { class: "knobs" });
    layout.groups.forEach((g, k) => {
      const part = (p) => {
        const id = g[p];
        if (id == null) return h("div", { class: p === 1 ? "dial ghost" : "turn ghost", title: t("stage.absent") });
        const a = get(S.layer, id);
        const label = actionLabel(a);
        const title = `${t("stage.knob", { n: k + 1 })} · ${partName(p)}${label ? `: ${label}` : ""}`;
        const attrs = { "data-id": id, "data-part": p, title, onclick: () => select(id) };
        const empty = t(`knob.${KNOB_PARTS[p].key}_short`);
        if (p === 1) return h("button", { class: `dial${isSel(id)}${isDirty(id)}`, ...attrs }, capContent(a, empty));
        return h("button", { class: `turn${isSel(id)}${isDirty(id)}`, ...attrs }, icon(KNOB_PARTS[p].icon), capContent(a, empty));
      };
      knobs.append(h("div", { class: "knob" }, h("div", { class: "knob-title" }, t("stage.knob", { n: k + 1 })), part(0), part(1), part(2)));
    });
    device.append(knobs);
  }

  stage.replaceChildren(
    h("div", { class: "stage-head" }, h("div", { class: "segmented" }, layerBtns), h("div", { class: "spacer" }),
      h("div", { class: "legend" }, h("span", {}, h("i", { class: "sw sel" }), t("stage.selected")), h("span", {}, h("i", { class: "sw dirty" }), t("stage.to_write")))),
    ...(banner ? [banner] : []), device);
}

// ---------------------------------------------------------------------------
// rendering: inspector
// ---------------------------------------------------------------------------
// virtual keyboard rows: [token, width in units, cap name]; token null = gap, "mod:x" = modifier.
// The cap shows the label of `cap name` (default: the token); right modifiers look like the left ones.
const VK_ROWS = [
  [["escape", 1.5], [null, 0.5], ...[1, 2, 3, 4].map((i) => [`f${i}`, 1]), [null, 0.5],
    ...[5, 6, 7, 8].map((i) => [`f${i}`, 1]), [null, 0.5], ...[9, 10, 11, 12].map((i) => [`f${i}`, 1])],
  [["grave", 1], ..."1234567890".split("").map((c) => [c, 1]), ["minus", 1], ["equal", 1], ["backspace", 2]],
  [["tab", 1.5], ..."qwertyuiop".split("").map((c) => [c, 1]), ["leftbracket", 1], ["rightbracket", 1], ["backslash", 1.5]],
  [["capslock", 1.75], ..."asdfghjkl".split("").map((c) => [c, 1]), ["semicolon", 1], ["quote", 1], ["enter", 2.25]],
  [["mod:shift", 1.25], ["nonusbackslash", 1], ..."zxcvbnm".split("").map((c) => [c, 1]),
    ["comma", 1], ["dot", 1], ["slash", 1], ["mod:rshift", 2.75, "mod:shift"]],
  [["mod:ctrl", 1.25], ["mod:win", 1.25], ["mod:alt", 1.25], ["space", 6.25],
    ["mod:ralt", 1.25], ["mod:rwin", 1.25, "mod:win"], ["application", 1.25], ["mod:rctrl", 1.25, "mod:ctrl"]],
];
const MORE_KEYS = ["printscreen", "scrolllock", "pause", "insert", "home", "pageup", "delete", "end", "pagedown",
  "up", "down", "left", "right", "numlock", "numpadslash", "numpadasterisk", "numpadminus", "numpadplus", "numpadenter",
  "numpad1", "numpad2", "numpad3", "numpad4", "numpad5", "numpad6", "numpad7", "numpad8", "numpad9", "numpad0",
  "numpaddot", "numpadequal", "nonushash", "power", ...Array.from({ length: 12 }, (_, i) => `f${i + 13}`)];

function keyByName(name) {
  return S.cat.keys.find((k) => k.name === name);
}

function chordView(mods, code) {
  const parts = modLabels(mods);
  if (code) parts.push(keyLabel(code));
  const out = [];
  parts.forEach((p, i) => { if (i) out.push(h("span", { class: "plus" }, "+")); out.push(h("kbd", {}, p)); });
  return out;
}

function renderKeysTab() {
  const a = get(S.layer, S.sel);
  const chords = currentChords();
  const seq = h("div", { class: `seq${chords.length ? " has" : ""}` });
  if (!chords.length) {
    seq.append(h("span", { class: "empty" }, t(S.recording ? "keys.recording" : "keys.empty")));
  }
  chords.forEach(([m, c], i) => {
    if (i) seq.append(h("span", { class: "arrow" }, "→"));
    seq.append(h("span", { class: "chord" }, chordView(m, c),
      h("button", { title: t("keys.remove"), onclick: () => removeChord(i) }, icon("x"))));
  });

  const vk = h("div", { class: "vk" });
  for (const row of VK_ROWS) {
    for (const [token, w, cap = token] of row) {
      const span = `grid-column: span ${Math.round(w * 4)}`;
      if (!token) { vk.append(h("span", { class: "gap", style: span })); continue; }
      if (token.startsWith("mod:")) {
        const bit = S.cat.modifiers.find(([, name]) => name === token.slice(4))[0];
        vk.append(h("button", { class: `mod${S.mods & bit ? " on" : ""}`, style: span, title: t("keys.mod_title"), onclick: () => toggleMod(bit) },
          t(`mod.${cap.slice(4)}`)));
      } else {
        const k = keyByName(token);
        vk.append(h("button", { style: span, title: `${token} (HID 0x${k.code.toString(16).padStart(2, "0")})`, onclick: () => clickKey(k.code) },
          keyNameLabel(cap)));
      }
    }
  }

  const modRow = h("div", { class: "row" },
    S.cat.modifiers.map(([bit, name]) => h("button", { class: `toggle${S.mods & bit ? " on" : ""}`, "data-mod": name, onclick: () => toggleMod(bit) },
      t(`mod.${name}`))));

  const delay = a.kind === "keys" ? a.delay : 0;
  const setDelay = (v) => {
    const cur = get(S.layer, S.sel);
    if (cur.kind !== "keys") return;
    const d = Math.max(0, Math.min(S.cat.maxDelay, Math.round(Number(v)) || 0));
    setAction({ ...strip(cur), delay: d });
  };

  // the key this input shows, even if the selection moves while Rust parses
  const [layer, id] = [S.layer, S.sel];
  const syntax = h("input", { class: `text-input${S.syntaxError ? " bad" : ""}`, spellcheck: "false", placeholder: t("keys.syntax_placeholder"),
    value: a.text, oninput: async (e) => {
      const k = keyOf(layer, id);
      const n = bump(k);
      const cur = get(layer, id);
      try {
        const view = await invoke("parse_action", { text: e.target.value, delay: cur.kind === "keys" ? cur.delay : 0 });
        if (edits.get(k) !== n) return; // a newer edit owns this key
        S.layers[layer - 1].set(id, view);
        S.dirty.add(k);
        S.syntaxError = null;
        e.target.classList.remove("bad");
        softRender();
        $("#syntax-error").textContent = "";
      } catch (err) {
        if (edits.get(k) !== n) return;
        S.syntaxError = err;
        e.target.classList.add("bad");
        $("#syntax-error").textContent = errText(err);
      }
    } });

  return [
    h("div", { class: "section" },
      h("div", { class: "section-title" }, t("keys.sequence"), h("span", { class: "grow" }), h("span", {}, `${chords.length}/${S.cat.maxChords}`)),
      seq,
      h("div", { class: "row" },
        h("button", { class: `btn sm rec${S.recording ? " on" : ""}`, onclick: () => (S.recording ? stopRecording() : startRecording()) },
          h("span", { class: "recdot" }), t(S.recording ? "keys.stop" : "keys.record")),
        h("button", { class: "btn sm", disabled: !S.mods, onclick: () => addChord(S.mods, 0) }, t("keys.mods_only")),
        h("button", { class: "btn sm ghost", disabled: !chords.length, onclick: () => setAction(NONE) }, t("keys.clear")))),
    h("div", { class: "section" }, h("label", {}, t("keys.modifiers")), modRow,
      h("div", { class: "hint" }, t("keys.mods_hint"))),
    h("div", { class: "section" }, vk,
      h("details", { open: S.open.more, ontoggle: (e) => { S.open.more = e.target.open; } },
        h("summary", {}, t("keys.more")),
        h("div", { class: "more-keys" }, MORE_KEYS.map((n) => {
          const k = keyByName(n);
          return h("button", { title: n, onclick: () => clickKey(k.code) }, keyNameLabel(n));
        })))),
    h("div", { class: "section" },
      h("label", {}, icon("clock"), t("keys.delay")),
      h("div", { class: "slider-row" },
        h("input", { type: "range", min: 0, max: S.cat.maxDelay, step: 50, value: delay, disabled: !chords.length,
          oninput: (e) => { e.target.nextSibling.value = e.target.value; }, onchange: (e) => setDelay(e.target.value) }),
        h("input", { class: "num", type: "number", min: 0, max: S.cat.maxDelay, step: 50, value: delay, disabled: !chords.length,
          onchange: (e) => setDelay(e.target.value) }), h("span", { class: "hint" }, "ms"))),
    h("details", { open: S.open.syntax, ontoggle: (e) => { S.open.syntax = e.target.open; } },
      h("summary", {}, t("keys.syntax")),
      h("div", { class: "section" }, syntax, h("div", { class: "error", id: "syntax-error" }, errText(S.syntaxError)),
        h("div", { class: "hint" }, t("keys.syntax_hint")))),
  ];
}

function renderMediaTab() {
  const a = get(S.layer, S.sel);
  return [h("div", { class: "section" }, h("label", {}, t("media.title")),
    h("div", { class: "tiles" }, S.cat.media.map((m) =>
      h("button", { class: `tile${a.kind === "media" && a.media === m.code ? " on" : ""}`, title: `${m.name} (0x${m.code.toString(16)})`,
        onclick: () => setAction({ kind: "media", media: m.code }) }, icon(m.name), t(`media.${m.name}`)))),
    h("div", { class: "hint" }, t("media.hint")))];
}

function renderMouseTab() {
  const a = get(S.layer, S.sel);
  const m = a.kind === "mouse" ? a : { ...NONE, kind: "mouse" };
  // patch from the current action, not the one this tab was drawn with
  const update = (patch) => {
    const cur = get(S.layer, S.sel);
    const base = cur.kind === "mouse" ? cur : { ...NONE, kind: "mouse" };
    setAction({ ...strip(base), kind: "mouse", ...patch(base) });
  };
  const wheel = [[0, t("mouse.wheel_none")], [WHEEL_UP, t("mouse.wheel_up")], [WHEEL_DOWN, t("mouse.wheel_down")]];
  const mods = [[0, t("mouse.mod_none")], ...[0x01, 0x02, 0x04].map((bit) => [bit, modLabels(bit)[0]])];
  if (m.mouseMod && !mods.some(([v]) => v === m.mouseMod)) mods.push([m.mouseMod, modLabels(m.mouseMod).join("+")]);
  return [
    h("div", { class: "section" }, h("label", {}, t("mouse.buttons")),
      h("div", { class: "tiles" }, [[0x01, "click"], [0x04, "mclick"], [0x02, "rclick"]].map(([bit, name]) =>
        h("button", { class: `tile${m.buttons & bit ? " on" : ""}`, onclick: () => update((c) => ({ buttons: c.buttons ^ bit })) }, icon("mouse"), t(`mouse.${name}`))))),
    h("div", { class: "section" }, h("label", {}, t("mouse.wheel")),
      h("div", { class: "segmented full" }, wheel.map(([v, label]) =>
        h("button", { class: m.wheel === v ? "on" : "", onclick: () => update(() => ({ wheel: v })) }, label)))),
    h("div", { class: "section" }, h("label", {}, t("mouse.modifier")),
      h("div", { class: "segmented full" }, mods.map(([v, label]) =>
        h("button", { class: m.mouseMod === v ? "on" : "", onclick: () => update(() => ({ mouseMod: v })) }, label))),
      h("div", { class: "hint" }, t("mouse.hint"))),
  ];
}

function renderLedTab() {
  const led = S.leds[S.layer - 1];
  const setLed = (patch) => {
    S.leds[S.layer - 1] = { mode: 1, color: 1, ...(S.leds[S.layer - 1] || {}), ...patch };
    bump(keyOf(S.layer, 0));
    S.dirty.add(keyOf(S.layer, 0));
    render();
  };
  return [
    h("div", { class: "section" }, h("div", { class: "section-title" }, t("led.title", { n: S.layer }), h("span", { class: "grow" }),
      led ? null : h("span", {}, t("led.unchanged"))),
      h("div", { class: "options" }, LED_MODES.map((mode) =>
        h("button", { class: `option${led?.mode === mode ? " on" : ""}`, "data-mode": mode, onclick: () => setLed({ mode }) },
          h("span", { class: "radio" }), h("span", {}, h("b", {}, t(`led.mode${mode}`)), mode ? ` — ${t(`led.desc${mode}`)}` : ""))))),
    h("div", { class: "section" }, h("label", {}, t("led.color")),
      h("div", { class: `swatches${led && (led.mode === 0 || led.mode === 5) ? " disabled" : ""}` }, LED_COLORS.map(([idx, name, rgb]) =>
        h("button", { class: `swatch${led?.color === idx ? " on" : ""}`, title: t(`led.${name}`), style: `background:${rgb}`, onclick: () => setLed({ color: idx }) })))),
    h("div", { class: "hint" }, t("led.hint")),
  ];
}

function renderInspectorHead() {
  const head = $("#insp-head");
  if (!head) return;
  if (S.sel == null) {
    head.replaceChildren(h("h1", {}, t("stage.layer", { n: S.layer })), h("div", { class: "current" }, t("insp.select")));
    return;
  }
  const a = get(S.layer, S.sel);
  head.replaceChildren(
    h("h1", {}, keyTitle(S.sel), h("span", { class: "layer-tag" }, t("stage.layer", { n: S.layer }))),
    h("div", { class: "current" },
      h("span", { class: `chip-now${a.kind === "none" ? " none" : ""}` }, actionLabel(a) || t("insp.none")),
      a.delay ? h("span", { class: "hint" }, t("insp.delay", { ms: a.delay })) : null,
      a.text ? h("code", {}, a.text) : null));
}

function renderInspector() {
  const insp = $("#inspector");
  S.inspStale = false;
  const tabs = ["keys", "media", "mouse", "led"]; // id = icon name, label tab.<id>
  if (S.sel == null && S.tab !== "led") S.tab = "led";
  const body = { keys: renderKeysTab, media: renderMediaTab, mouse: renderMouseTab, led: renderLedTab }[S.tab]();
  const oldBody = insp.querySelector(".insp-body");
  const scroll = oldBody && insp.dataset.view === `${S.tab}:${S.sel}` ? oldBody.scrollTop : 0;
  insp.dataset.view = `${S.tab}:${S.sel}`;
  insp.replaceChildren(
    h("div", { class: "insp-head", id: "insp-head" }),
    h("div", { class: "insp-tabs" }, h("div", { class: "segmented full" }, tabs.map((id) =>
      h("button", { class: S.tab === id ? "on" : "", "data-tab": id, disabled: S.sel == null && id !== "led",
        onclick: () => { S.tab = id; stopRecording(); render(); } }, icon(id), t(`tab.${id}`))))),
    h("div", { class: "insp-body" }, body),
    h("div", { class: "insp-foot" },
      h("button", { class: "btn sm", disabled: S.sel == null || get(S.layer, S.sel).kind === "none", onclick: () => setAction(NONE) }, icon("trash"), t("insp.clear_key")),
      h("div", { class: "spacer" }),
      h("button", { class: "btn sm ghost danger", onclick: clearLayer }, t("insp.clear_layer"))));
  insp.querySelector(".insp-body").scrollTop = scroll;
  // translated cap names can be longer than the cap: smaller text, on two lines if needed
  for (const b of insp.querySelectorAll(".vk button")) if (b.scrollWidth > b.clientWidth) b.classList.add("tight");
  renderInspectorHead();
}

async function clearLayer() {
  if (!(await confirm(t("insp.clear_layer"), t("dlg.clear_layer_body", { n: S.layer }), t("dlg.clear"), "danger"))) return;
  for (const id of keyIds(S.layout)) {
    S.layers[S.layer - 1].set(id, NONE);
    bump(keyOf(S.layer, id));
    S.dirty.add(keyOf(S.layer, id));
  }
  render();
}

function render() {
  renderTopbar();
  renderStage();
  renderInspector();
  $("#main").classList.toggle("locked", S.busy);
}

const typingInInspector = () => !!document.activeElement?.matches("#inspector input, #inspector textarea");

/** render(), but an inspector input with focus keeps its text and caret: rebuilt once focus leaves. */
function softRender() {
  if (!typingInInspector()) return render();
  renderTopbar();
  renderStage();
  renderInspectorHead();
  $("#main").classList.toggle("locked", S.busy);
  S.inspStale = true;
}

// ---------------------------------------------------------------------------
// start
// ---------------------------------------------------------------------------
async function init() {
  setLang(detectLang());
  S.cat = await invoke("catalog");
  S.layout = S.cat.layouts.find((l) => l.keys === 12 && l.knobs === 2);
  S.sel = 1;

  $("#btn-new").append(icon("new"));
  $("#btn-open").append(icon("open"));
  $("#btn-save").append(icon("save"));
  const lang = $("#lang");
  lang.append(...Object.entries(LANGS).map(([code, name]) => h("option", { value: code }, name)));
  lang.addEventListener("change", (e) => {
    setLang(e.target.value, true);
    render(); // all state is kept, only texts change
  });
  $("#btn-new").addEventListener("click", newProfile);
  $("#btn-open").addEventListener("click", openProfile);
  $("#btn-save").addEventListener("click", saveProfile);
  $("#btn-read").addEventListener("click", readDevice);
  $("#btn-write").addEventListener("click", writeDevice);
  $("#model").addEventListener("change", (e) => {
    S.modelChoice = e.target.value;
    if (S.modelChoice === "auto") {
      if (S.detected) relayout(S.detected);
      else toast(t("msg.no_model"), "info");
    } else {
      relayout(S.cat.layouts[Number(S.modelChoice)]);
    }
    render();
  });
  document.addEventListener("keydown", (e) => {
    if (S.recording || !e.ctrlKey || $("#modal-root").childElementCount) return;
    if (e.key === "o") { e.preventDefault(); openProfile(); }
    if (e.key === "s") { e.preventDefault(); saveProfile(); }
  });
  // catch up once focus leaves the inspector (a click inside it renders by itself)
  $("#inspector").addEventListener("focusout", (e) => {
    if (S.inspStale && !$("#inspector").contains(e.relatedTarget)) renderInspector();
  });
  T.event.listen("progress", (e) => {
    const [done, total] = e.payload;
    setProgress(done, total);
  });

  render();
  await poll();
  setInterval(poll, 1500);
}

init().catch((e) => {
  document.body.replaceChildren(h("pre", { style: "padding:20px;color:#f2555a" }, t("app.start_error", { error: errText(e) })));
});
