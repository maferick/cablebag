// Publishes a new Cablebag version: node scripts/release.mjs <version> ["<what changed>"] [--dry]
//
// 1. Sets the version, writes the third-party notices, commits and tags (on main, which is GitHub's main).
// 2. Scans every tracked file for private words, before anything leaves this PC.
// 3. Builds the installer, signed with the update key from Vaultwarden (held in memory only). When a
//    code-signing certificate is set, cablebag.exe, cablebag-rdp.exe and the installer get an Authenticode
//    signature too (scripts/sign.mjs), so Windows sees a known publisher.
// 4. Writes latest.json, the file installed copies check for updates.
// 5. Scans the built programs too, pushes main and the tag, and creates the GitHub release with the
//    installer, its signature and latest.json.
// With --dry it stops before anything leaves this PC (the commit and tag stay local).
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const REPO = "maferick/cablebag";
/**
 * Private settings of the PC that releases, outside the repository:
 * { "vaultHost": "user@host", "signing": { "thumbprint": "<40 hex>", "timestampUrl": "http://..." } }.
 * "signing" is the code-signing certificate; leave it out and the programs are not Authenticode-signed.
 */
const LOCAL = JSON.parse(readFileSync(join(homedir(), ".cablebag", "release.json"), "utf8"));
const VAULT_HOST = LOCAL.vaultHost;
const SIGNING = LOCAL.signing;
const VAULT_ITEM = "cablebag-updater-key";

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
run("git", ["tag", "-f", `v${version}`]);

// 2. Nothing personal in what is published.
step("Scan");
run("node", ["scripts/scan-private.mjs", ...out("git", ["ls-files"]).split("\n").filter(Boolean)]);

// 3. Build, signed. Paths of this PC are taken out of the programs.
step("Build");
// The certificate has to be there before a long build starts: its cloud tool must be signed in.
const signEnv = {};
const buildArgs = ["run", "tauri", "build"];
if (SIGNING) {
  const thumb = String(SIGNING.thumbprint ?? "").replace(/\s/g, "").toUpperCase();
  if (!/^[0-9A-F]{40}$/.test(thumb) || !SIGNING.timestampUrl) throw new Error('release.json "signing" needs a "thumbprint" and a "timestampUrl".');
  const ready = out("powershell", ["-NoProfile", "-Command", `$c = Get-Item Cert:\\CurrentUser\\My\\${thumb} -ErrorAction SilentlyContinue; [bool]($c -and $c.HasPrivateKey)`]);
  if (ready !== "True") throw new Error("The code-signing certificate is not in the Windows certificate store; sign in to its signing tool (for example SimplySign Desktop) first.");
  Object.assign(signEnv, {
    CABLEBAG_SIGN_THUMBPRINT: thumb,
    CABLEBAG_SIGN_TIMESTAMP: SIGNING.timestampUrl,
    // Tauri looks for signtool only under the 64-bit SDK root, which on some PCs has none.
    TAURI_WINDOWS_SIGNTOOL_PATH: out("node", ["scripts/sign.mjs", "--signtool"]),
  });
  // Tauri signs cablebag.exe and the installer with the same script; the Remote Desktop helper is signed by build:rdp.
  const signConf = join(process.cwd(), "src-tauri", "target", "sign.conf.json");
  writeFileSync(signConf, JSON.stringify({ bundle: { windows: { signCommand: { cmd: process.execPath, args: [join(process.cwd(), "scripts", "sign.mjs"), "%1"] } } } }));
  buildArgs.push("--", "--config", signConf);
} else {
  console.log("No code-signing certificate set in release.json: the programs get no Authenticode signature.");
}
const key = JSON.parse(
  // A fresh vault session every time: a stale one makes bw ask for the master password instead.
  out(
    "ssh",
    [
      "-o",
      "BatchMode=yes",
      VAULT_HOST,
      `export PATH=$PATH:/snap/bin; S=$(BW_PASSWORD="$(cat ~/.bw_master)" timeout 60 bw unlock --passwordenv BW_PASSWORD --raw </dev/null) && timeout 40 bw get item ${VAULT_ITEM} --session "$S" </dev/null`,
    ],
    { timeout: 150_000 },
  ),
);
const env = {
  ...process.env,
  ...signEnv,
  TAURI_SIGNING_PRIVATE_KEY: key.notes.split("\n")[0].trim(),
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD: key.login.password,
  RUSTFLAGS: `${process.env.RUSTFLAGS ?? ""} --remap-path-prefix=${homedir()}=~ --remap-path-prefix=${process.cwd()}=cablebag`.trim(),
};
run("npm", buildArgs, { env, shell: true });

const bundle = "src-tauri/target/release/bundle/nsis";
const setup = `Cablebag_${version}_x64-setup.exe`;
if (!existsSync(join(bundle, setup)) || !existsSync(join(bundle, `${setup}.sig`))) throw new Error(`${setup} or its signature is missing.`);
if (SIGNING) {
  // Tauri puts the unsigned cablebag.exe back in target after bundling, so the programs are checked as the
  // installer holds them (unpacked with the 7-Zip that build:xserver needs anyway).
  const check = join("src-tauri", "target", "signcheck");
  rmSync(check, { recursive: true, force: true });
  const sevenZip = ["7z", join(process.env.ProgramFiles ?? "C:\\Program Files", "7-Zip", "7z.exe")].find((c) => {
    try {
      execFileSync(c, ["i"], { stdio: "ignore" });
      return true;
    } catch {
      return false;
    }
  });
  if (!sevenZip) throw new Error("7-Zip is needed to check the signatures in the installer.");
  execFileSync(sevenZip, ["x", "-y", `-o${check}`, join(bundle, setup), "cablebag.exe", "cablebag-rdp.exe", "uninstall.exe"], { stdio: ["ignore", "ignore", "inherit"] });
  run("node", ["scripts/sign.mjs", "--verify", join(bundle, setup), ...["cablebag.exe", "cablebag-rdp.exe", "uninstall.exe"].map((f) => join(check, f))], { env });
}

// 4. The update list.
const latest = {
  version,
  notes: text,
  pub_date: new Date().toISOString().replace(/\.\d+Z$/, "Z"),
  platforms: {
    "windows-x86_64": {
      signature: readFileSync(join(bundle, `${setup}.sig`), "utf8").trim(),
      url: `https://github.com/${REPO}/releases/download/v${version}/${setup}`,
    },
  },
};
writeFileSync(join(bundle, "latest.json"), JSON.stringify(latest, null, 2) + "\n");

// 5. The programs too, then out.
step("Publish");
run("node", ["scripts/scan-private.mjs", "src-tauri/target/release/cablebag.exe", "rdp-helper/publish/cablebag-rdp.exe", "build", join(bundle, setup), join(bundle, "latest.json")]);
if (dry) {
  console.log(`\nDry run done: ${join(bundle, setup)} and latest.json are ready; the commit and tag v${version} are local. Nothing was pushed.`);
  process.exit(0);
}
run("git", ["push", "-q", "origin", "main", `v${version}`]);
run("gh", ["release", "create", `v${version}`, "--repo", REPO, "--title", `Cablebag ${version}`, "--notes", text, join(bundle, setup), join(bundle, `${setup}.sig`), join(bundle, "latest.json")]);
console.log(`\nReleased: https://github.com/${REPO}/releases/tag/v${version}`);
