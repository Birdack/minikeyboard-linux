// Consistency check of ui/i18n.js: `node dev/check-i18n.mjs`, exits 1 on any problem.
// - every language has exactly the keys of English, no empty texts, the same {placeholders}
// - every t("literal") in ui/app.js exists
// - every name the UI translates from the catalog (dev/catalog.json) and every Rust error code has a text
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const { DICTS, LANGS } = await import(pathToFileURL(join(root, "ui/i18n.js")).href);
const problems = [];
const fail = (msg) => problems.push(msg);

const isPlural = (v) => v && typeof v === "object" && typeof v.other === "string";

/** key path -> text (plural forms as "path#form") */
function flatten(obj, prefix = "", out = new Map()) {
  for (const [k, v] of Object.entries(obj)) {
    const path = prefix ? `${prefix}.${k}` : k;
    if (typeof v === "string") out.set(path, v);
    else if (isPlural(v)) for (const [form, text] of Object.entries(v)) out.set(`${path}#${form}`, text);
    else if (v && typeof v === "object") flatten(v, path, out);
    else fail(`${path}: not a text (${typeof v})`);
  }
  return out;
}

const placeholders = (s) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(",");
const en = flatten(DICTS.en);
const keyOf = (path) => path.split("#")[0];
const enKeys = new Set([...en.keys()].map(keyOf));

if (JSON.stringify(Object.keys(LANGS).sort()) !== JSON.stringify(Object.keys(DICTS).sort())) {
  fail(`LANGS (${Object.keys(LANGS)}) and DICTS (${Object.keys(DICTS)}) list different languages`);
}

for (const [lang, dict] of Object.entries(DICTS)) {
  const flat = flatten(dict);
  const keys = new Set([...flat.keys()].map(keyOf));
  for (const k of enKeys) if (!keys.has(k)) fail(`${lang}: missing ${k}`);
  for (const k of keys) if (!enKeys.has(k)) fail(`${lang}: extra key ${k}`);
  for (const [path, text] of flat) {
    if (!text.trim()) fail(`${lang}: empty text ${path}`);
    const form = path.includes("#");
    if (form && !flat.has(`${keyOf(path)}#other`)) fail(`${lang}: plural ${keyOf(path)} without "other"`);
    // a plural form is compared with the English "other" form (forms differ between languages)
    const ref = en.get(path) ?? en.get(`${keyOf(path)}#other`) ?? en.get(keyOf(path));
    if (ref !== undefined && placeholders(ref) !== placeholders(text)) {
      fail(`${lang}: ${path} has {${placeholders(text)}}, English has {${placeholders(ref)}}`);
    }
  }
}

// static keys used by the UI
const app = readFileSync(join(root, "ui/app.js"), "utf8");
const used = [...app.matchAll(/\bt\(\s*(["'])([^"'`$]+)\1/g)].map((m) => m[2]);
for (const k of new Set(used)) if (!enKeys.has(k)) fail(`ui/app.js uses unknown key t("${k}")`);
if (!used.length) fail("no t(\"…\") call found in ui/app.js");

// keys built from catalog names: t(`media.${name}`), t(`mod.${name}`), t(`mouse.${name}`)
const cat = JSON.parse(readFileSync(join(root, "dev/catalog.json"), "utf8"));
for (const m of cat.media) if (!enKeys.has(`media.${m.name}`)) fail(`no text for media.${m.name}`);
for (const [, name] of cat.modifiers) if (!enKeys.has(`mod.${name}`)) fail(`no text for mod.${name}`);
for (const [, name] of cat.mouseButtons) if (!enKeys.has(`mouse.${name}`)) fail(`no text for mouse.${name}`);
for (const l of cat.layouts) if (l.kind !== "standard" && !enKeys.has(`layout.${l.kind}`)) fail(`no text for layout.${l.kind}`);

// error codes produced by Rust: Error::new("code")
const src = join(root, "src-tauri/src");
const codes = new Set();
for (const f of readdirSync(src).filter((f) => f.endsWith(".rs"))) {
  const rs = readFileSync(join(src, f), "utf8").split(/#\[cfg\(test\)\]\s*mod tests/)[0];
  for (const m of rs.matchAll(/Error::new\("(\w+)"\)/g)) codes.add(m[1]);
}
if (!codes.size) fail("no Error::new(\"…\") found in src-tauri/src");
for (const c of codes) if (!enKeys.has(`err.${c}`)) fail(`no text for Rust error code err.${c}`);

if (problems.length) {
  console.error(`i18n check failed (${problems.length}):\n  ${problems.join("\n  ")}`);
  process.exit(1);
}
console.log(`i18n ok: ${Object.keys(DICTS).length} languages, ${enKeys.size} keys, ${new Set(used).size} used statically, ${codes.size} error codes`);
