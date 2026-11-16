// Checks what tools/extension/pack.ps1 built (target/extension) before it is uploaded:
//   node tools/extension/check-pack.mjs
// The Edge package carries the app's version, only English, and every manifest resource.
// Its key must keep the ID the native host serves (crates/ipc/src/extension.rs).
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import assert from "node:assert/strict";

const root = new URL("../../", import.meta.url);
const out = new URL("target/extension/", root);
const cargo = readFileSync(new URL("Cargo.toml", root), "utf8");
const version = cargo.match(/^version\s*=\s*"([^"]+)"/m)[1];
const ipc = readFileSync(new URL("crates/ipc/src/extension.rs", root), "utf8");
const unpackedId = ipc.match(/UNPACKED_ID: &str = "([a-p]{32})"/)[1];

// A Chromium extension's ID: the first 32 hex digits of SHA-256 of its public key, 0-f as a-p.
const idOf = (key) =>
  [...createHash("sha256").update(Buffer.from(key, "base64")).digest("hex").slice(0, 32)]
    .map((c) => String.fromCharCode(97 + parseInt(c, 16)))
    .join("");

assert.deepEqual(readdirSync(out).sort(), ["chromium", "mdm-chromium.zip"]);
for (const kind of ["chromium"]) {
  const dir = new URL(`${kind}/`, out);
  const m = JSON.parse(readFileSync(new URL("manifest.json", dir), "utf8"));
  assert.equal(m.version, version, `${kind}: the app's version`);
  const files = [
    m.background.service_worker,
    ...(m.background.scripts || []),
    m.action && m.action.default_popup,
    ...Object.values(m.icons),
    ...m.content_scripts.flatMap((c) => c.js),
  ].filter(Boolean);
  for (const f of files) assert.ok(existsSync(new URL(f, dir)), `${kind}: ${f} is there`);
  assert.ok(existsSync(new URL("_locales/en/messages.json", dir)), `${kind}: English texts`);
  assert.deepEqual(readdirSync(new URL("_locales/", dir)), ["en"], "English only");
  assert.equal(m.default_locale, "en");
  assert.equal(idOf(m.key), unpackedId, "the key gives the ID the host serves");
  for (const f of ["LICENSE.txt", "ATTRIBUTION.txt"]) {
    assert.ok(existsSync(new URL(f, dir)), `${kind}: ${f}`);
  }
}
console.log(`packages ok (version ${version}, unpacked ID ${unpackedId})`);
