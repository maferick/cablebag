// Publishes a new Cablebag version: node scripts/release.mjs <version> ["<what changed>"] [--dry]
//
// 1. Sets the version, writes the third-party notices, commits and tags (on main, which is GitHub's main); the
//    tag carries the notes.
// 2. Scans every tracked file for private words, before anything leaves this PC.
// 3. Pushes main and the tag. GitHub builds the installer on its own servers (.github/workflows/release.yml),
//    signed with the update key kept in the repository's secrets, and makes a draft release with the installer,
//    its signature and latest.json, the file installed copies check for updates.
// 4. Downloads that build and scans the programs for private words too (the list never leaves this PC) and
//    with Microsoft Defender, then publishes the draft. A private word deletes the draft; a Defender hit keeps it
//    unpublished.
// With --dry it stops before anything leaves this PC (the commit and tag stay local). A test build without a
// release: Actions > Release > Run workflow.
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const REPO = "maferick/cablebag";

const [version, notes = "", ...flags] = process.argv.slice(2);
const dry = flags.includes("--dry") || notes === "--dry";
if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) throw new Error('Usage: node scripts/release.mjs 0.2.1 "What changed" [--dry]');
/** The notes: given on the command line, or else this version's section of CHANGELOG.md. */
function changelogSection(v) {
  const lines = existsSync("CHANGELOG.md") ? readFileSync("CHANGELOG.md", "utf8").split(/\r?\n/) : [];
  const start = lines.findIndex((l) => l === `## ${v}` || l.startsWith(`## ${v} `));
  if (start < 0) return "";
  const end = lines.findIndex((l, i) => i > start && l.startsWith("## "));
  return lines.slice(start + 1, end < 0 ? undefined : end).join("\n").trim();
}
const given = notes === "--dry" ? "" : notes;
const text = given || changelogSection(version);
if (!text) throw new Error(`Write the changes for ${version} in CHANGELOG.md (a "## ${version}" section) or pass them on the command line.`);

const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { stdio: "inherit", shell: false, ...opts });
const out = (cmd, args, opts = {}) => execFileSync(cmd, args, { encoding: "utf8", ...opts }).trim();
const step = (s) => console.log(`\n== ${s}`);

// 1. Version, notices, commit and tag.
step(`Version ${version}`);
if (out("git", ["status", "--porcelain"])) throw new Error("The working tree has uncommitted changes; commit them first.");
if (out("git", ["branch", "--show-current"]) !== "main") throw new Error("Release from main.");
// What others pushed comes first, so the release is made on top of it.
run("git", ["pull", "-q", "--ff-only", "origin", "main"]);
const conf = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
conf.version = version;
writeFileSync("src-tauri/tauri.conf.json", JSON.stringify(conf, null, 2) + "\n");
const pkg = JSON.parse(readFileSync("package.json", "utf8"));
pkg.version = version;
writeFileSync("package.json", JSON.stringify(pkg, null, 2) + "\n");
writeFileSync("src-tauri/Cargo.toml", readFileSync("src-tauri/Cargo.toml", "utf8").replace(/^version = ".*"$/m, `version = "${version}"`));
run("node", ["scripts/notices.mjs"]);
run("cargo", ["update", "-p", "cablebag", "--offline", "--manifest-path", "src-tauri/Cargo.toml"]);
run("git", ["add", "-A"]);
if (out("git", ["status", "--porcelain"])) run("git", ["commit", "-q", "-m", `Cablebag ${version}`]);
// verbatim: lines starting with # (headings in the notes) stay in.
run("git", ["tag", "-a", "-f", "--cleanup=verbatim", `v${version}`, "-m", text]);

// 2. Nothing personal in what is published.
step("Scan");
run("node", ["scripts/scan-private.mjs", ...out("git", ["ls-files"]).split("\n").filter(Boolean)]);

// 3. Out: GitHub builds it.
if (dry) {
  console.log(`\nDry run done: the commit and tag v${version} are local. Nothing was pushed.`);
  process.exit(0);
}
step("Build on GitHub");
run("git", ["push", "-q", "origin", "main", `v${version}`]);
const tag = `v${version}`;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let id = "";
for (let i = 0; i < 40 && !id; i++) {
  await sleep(5000);
  const runs = JSON.parse(out("gh", ["run", "list", "--repo", REPO, "--workflow", "release.yml", "--limit", "20", "--json", "databaseId,headBranch,event"]));
  id = String(runs.find((r) => r.headBranch === tag && r.event === "push")?.databaseId ?? "");
}
if (!id) throw new Error(`GitHub did not start building ${tag}; see https://github.com/${REPO}/actions`);
console.log(`Building: https://github.com/${REPO}/actions/runs/${id}`);
run("gh", ["run", "watch", id, "--repo", REPO, "--exit-status", "--interval", "30"]);

// 4. The programs too, then public.
step("Scan the build");
const dir = mkdtempSync(join(tmpdir(), `cablebag-${version}-`));
run("gh", ["run", "download", id, "--repo", REPO, "--name", "release", "--dir", dir]);
const nsis = join(dir, "src-tauri", "target", "release", "bundle", "nsis");
const setup = `Cablebag_${version}_x64-setup.exe`;
try {
  run("node", [
    "scripts/scan-private.mjs",
    join(dir, "src-tauri", "target", "release", "cablebag.exe"),
    join(dir, "rdp-helper", "publish", "cablebag-rdp.exe"),
    join(dir, "build"),
    join(nsis, "latest.json"),
    "--packed",
    join(nsis, setup),
  ]);
} catch {
  run("gh", ["release", "delete", tag, "--repo", REPO, "--yes"]);
  throw new Error(`A private word is in the build of ${version}; the draft release was deleted and nothing was published.`);
}

// 5. Microsoft Defender must not take it for malware: a version it flags is removed from users' PCs. Report-only
// scans (-DisableRemediation); Defender's own real-time protection may already have removed a flagged file.
step("Defender");
const programs = [join(dir, "src-tauri", "target", "release", "cablebag.exe"), join(dir, "rdp-helper", "publish", "cablebag-rdp.exe"), join(nsis, setup)];
const mpcmd = join(process.env.ProgramFiles ?? "C:\\Program Files", "Windows Defender", "MpCmdRun.exe");
if (!existsSync(mpcmd)) throw new Error(`Microsoft Defender's scanner is missing (${mpcmd}); the draft of ${version} stays unpublished.`);
const flagged = programs.filter((file) => {
  if (!existsSync(file)) return true;
  try {
    const said = out(mpcmd, ["-Scan", "-ScanType", "3", "-File", file, "-DisableRemediation"]);
    console.log(said.split("\n").pop());
    return !said.includes("found no threats");
  } catch {
    return true;
  }
});
if (flagged.length) {
  throw new Error(`Microsoft Defender flags ${flagged.join(", ")}. The draft of ${version} stays unpublished (https://github.com/${REPO}/releases); report the files at https://www.microsoft.com/wdsi/filesubmission.`);
}

step("Publish");
run("gh", ["release", "edit", tag, "--repo", REPO, "--draft=false", "--latest"]);
rmSync(dir, { recursive: true, force: true });
console.log(`\nReleased: https://github.com/${REPO}/releases/tag/${tag}`);
