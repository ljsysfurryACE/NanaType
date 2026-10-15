// Reads the built-in skin manifests and writes src/skins.generated.json
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { join, dirname } from 'node:path';
import { parse } from 'smol-toml';

const here = dirname(fileURLToPath(import.meta.url));
const dir = join(here, '../../crates/theme/builtin');
const raw = {};
for (const f of readdirSync(dir).filter((f) => f.endsWith('.toml'))) {
  const t = parse(readFileSync(join(dir, f), 'utf8'));
  raw[t.meta.id] = t;
}
const isObj = (v) => v && typeof v === 'object' && !Array.isArray(v);
const merge = (a, b) => {
  const o = { ...a };
  for (const [k, v] of Object.entries(b)) o[k] = isObj(v) && isObj(a?.[k]) ? merge(a[k], v) : v;
  return o;
};
const resolve = (id) => {
  const t = raw[id];
  return t.meta.base ? merge(resolve(t.meta.base), t) : t;
};
const out = {};
for (const id of Object.keys(raw)) {
  const r = resolve(id);
  out[id.replace('nanatype.', '')] = r;
}
writeFileSync(join(here, '../src/skins.generated.json'), JSON.stringify(out, null, 1));
console.log(Object.keys(out).length, 'skins');
