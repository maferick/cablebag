// Writes latest.json (the file installed copies check for updates) and notes.md next to the built installer:
// node scripts/latest.mjs
//
// The notes are the message of the version's tag (release.mjs puts them there), or else the version's section
// of CHANGELOG.md. Used by the Release workflow after the build.
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const REPO = "maferick/cablebag";
const version = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8")).version;
const bundle = "src-tauri/target/release/bundle/nsis";
const setup = `Cablebag_${version}_x64-setup.exe`;
if (!existsSync(join(bundle, setup)) || !existsSync(join(bundle, `${setup}.sig`))) throw new Error(`${setup} or its signature is missing.`);

function tagMessage(v) {
  try {
    return execFileSync("git", ["tag", "-l", "--format=%(contents)", `v${v}`], { encoding: "utf8" }).trim();
  } catch {
    return "";
  }
}
function changelogSection(v) {
  const lines = existsSync("CHANGELOG.md") ? readFileSync("CHANGELOG.md", "utf8").split(/\r?\n/) : [];
  const start = lines.findIndex((l) => l === `## ${v}` || l.startsWith(`## ${v} `));
  if (start < 0) return "";
  const end = lines.findIndex((l, i) => i > start && l.startsWith("## "));
  return lines.slice(start + 1, end < 0 ? undefined : end).join("\n").trim();
}
// A lightweight tag's "contents" is the commit message (Cablebag x.y.z): only a real message counts.
const fromTag = tagMessage(version);
const notes = (fromTag && fromTag !== `Cablebag ${version}` ? fromTag : "") || changelogSection(version) || `Cablebag ${version}`;

const latest = {
  version,
  notes,
  pub_date: new Date().toISOString().replace(/\.\d+Z$/, "Z"),
  platforms: {
    "windows-x86_64": {
      signature: readFileSync(join(bundle, `${setup}.sig`), "utf8").trim(),
      url: `https://github.com/${REPO}/releases/download/v${version}/${setup}`,
    },
  },
};
writeFileSync(join(bundle, "latest.json"), JSON.stringify(latest, null, 2) + "\n");
writeFileSync(join(bundle, "notes.md"), notes + "\n");
console.log(`latest.json and notes.md written for ${version}.`);
