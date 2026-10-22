// Writes THIRD-PARTY-NOTICES.md: every open-source component that ends up in Cablebag, with its licence.
// Run from the repository root: node scripts/notices.mjs
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const rows = [];

// Rust crates built into the program (Windows target only).
const meta = JSON.parse(
  execFileSync("cargo", ["metadata", "--format-version", "1", "--filter-platform", "x86_64-pc-windows-msvc", "--manifest-path", "src-tauri/Cargo.toml"], {
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  }),
);
const used = new Set(meta.resolve.nodes.map((n) => n.id));
for (const p of meta.packages) {
  if (!used.has(p.id) || p.name === "cablebag") continue;
  rows.push({ kind: "Rust", name: p.name, version: p.version, licence: p.license ?? "see the project", link: p.repository ?? `https://crates.io/crates/${p.name}` });
}

// JavaScript packages in the window (runtime dependencies only).
const lock = JSON.parse(readFileSync("package-lock.json", "utf8"));
for (const [path, p] of Object.entries(lock.packages)) {
  if (!path || p.dev) continue;
  const name = path.replace(/^.*node_modules\//, "");
  rows.push({ kind: "JavaScript", name, version: p.version, licence: p.license ?? "see the project", link: `https://www.npmjs.com/package/${name}` });
}

rows.sort((a, b) => a.kind.localeCompare(b.kind) || a.name.localeCompare(b.name) || a.version.localeCompare(b.version));
const seen = new Set();
const unique = rows.filter((r) => {
  const k = `${r.kind}:${r.name}:${r.version}`;
  return !seen.has(k) && seen.add(k);
});

const out = [
  "# Third-party notices",
  "",
  "Cablebag is built with the open-source components below. Each is used under its own licence, unchanged;",
  "their licence texts are in the linked projects. The licence of Cablebag itself (LICENSE) does not limit the",
  "rights these licences give. Components under the Mozilla Public License 2.0 are unchanged; their source code",
  "is available from the linked projects.",
  "",
  "The Remote Desktop helper uses only the .NET runtime (MIT) and the Remote Desktop control that is part of",
  "Windows. The window uses Microsoft Edge WebView2, which is part of Windows.",
  "",
  "| Component | Version | Licence | Where |",
  "| --- | --- | --- | --- |",
  ...unique.map((r) => `| ${r.name} (${r.kind}) | ${r.version} | ${r.licence} | ${r.link} |`),
  "",
];
writeFileSync("THIRD-PARTY-NOTICES.md", out.join("\n"));
console.log(`${unique.length} components written to THIRD-PARTY-NOTICES.md`);
