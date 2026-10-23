// Fails when a Tauri npm package and its Rust crate are on different major.minor releases.
// `tauri build` refuses to bundle in that case, so catch it in CI instead of at release time.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = (p) => fileURLToPath(new URL(`../../../${p}`, import.meta.url));
const lock = readFileSync(root("Cargo.lock"), "utf8");

const pairs = {
  tauri: "@tauri-apps/api",
  "tauri-plugin-dialog": "@tauri-apps/plugin-dialog",
  "tauri-plugin-opener": "@tauri-apps/plugin-opener",
};

const crateVersion = (name) => lock.match(new RegExp(`name = "${name}"\\nversion = "([^"]+)"`))?.[1];
const npmVersion = (name) => JSON.parse(readFileSync(root(`apps/desktop/node_modules/${name}/package.json`), "utf8")).version;
const minor = (v) => v.split(".").slice(0, 2).join(".");

let failed = false;
for (const [crate, pkg] of Object.entries(pairs)) {
  const c = crateVersion(crate);
  const n = npmVersion(pkg);
  const ok = c && minor(c) === minor(n);
  console.log(`${ok ? "ok      " : "MISMATCH"} ${crate} ${c ?? "missing"}  ↔  ${pkg} ${n}`);
  failed ||= !ok;
}
if (failed) {
  console.error("\nAlign the versions: `cargo update -p <crate>` or change apps/desktop/package.json.");
  process.exit(1);
}
