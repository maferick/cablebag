// Publishes a new Cablebag version: node scripts/release.mjs <version> "<what changed>" [--dry]
//
// 1. Sets the version, writes the third-party notices, commits and tags in the private repository (origin).
// 2. Builds the installer, signed with the update key from Vaultwarden (held in memory only).
// 3. Writes latest.json, the file installed copies check for updates.
// 4. Copies the publishable files into the public repository (a separate folder with its own history,
//    commits under the GitHub no-reply address) and scans everything for private words first.
// 5. Pushes the public repository and creates the GitHub release with the installer, its signature and
//    latest.json.
// With --dry it stops before anything leaves this PC (no push, no release).
import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";

const REPO = "maferick/cablebag";
const PUBLIC_DIR = join(dirname(process.cwd()), "cablebag-public");
const AUTHOR = ["-c", "user.name=maferick", "-c", "user.email=5728058+maferick@users.noreply.github.com"];
/** Private settings of the PC that releases, outside the repository: { "vaultHost": "user@host" }. */
const LOCAL = JSON.parse(readFileSync(join(homedir(), ".cablebag", "release.json"), "utf8"));
const VAULT_HOST = LOCAL.vaultHost;
const VAULT_ITEM = "cablebag-updater-key";
/** Never published: private notes and plans. */
const PRIVATE = [/^docs\//, /^PLAN\.md$/, /^\.claude\//];

const [version, notes = "", ...flags] = process.argv.slice(2);
const dry = flags.includes("--dry") || notes === "--dry";
if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) throw new Error('Usage: node scripts/release.mjs 0.2.1 "What changed" [--dry]');
const text = notes === "--dry" ? "" : notes;

const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { stdio: "inherit", shell: false, ...opts });
const out = (cmd, args, opts = {}) => execFileSync(cmd, args, { encoding: "utf8", ...opts }).trim();
const step = (s) => console.log(`\n== ${s}`);

// 1. Version, notices, commit and tag (private).
step(`Version ${version}`);
if (out("git", ["status", "--porcelain"])) throw new Error("The working tree has uncommitted changes; commit them first.");
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
if (out("git", ["status", "--porcelain"])) run("git", [...AUTHOR, "commit", "-q", "-m", `Cablebag ${version}`]);
run("git", [...AUTHOR, "tag", "-f", `v${version}`]);

// 2. Build, signed. Paths of this PC are taken out of the programs.
step("Build");
const key = JSON.parse(
  out("ssh", ["-o", "BatchMode=yes", VAULT_HOST, `export PATH=$PATH:/snap/bin; timeout 40 bw get item ${VAULT_ITEM} --session "$(cat ~/.bw_session)" </dev/null`], { timeout: 90_000 }),
);
const env = {
  ...process.env,
  TAURI_SIGNING_PRIVATE_KEY: key.notes.split("\n")[0].trim(),
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD: key.login.password,
  RUSTFLAGS: `${process.env.RUSTFLAGS ?? ""} --remap-path-prefix=${homedir()}=~ --remap-path-prefix=${process.cwd()}=cablebag`.trim(),
};
run("npm", ["run", "tauri", "build"], { env, shell: true });

const bundle = "src-tauri/target/release/bundle/nsis";
const setup = `Cablebag_${version}_x64-setup.exe`;
if (!existsSync(join(bundle, setup)) || !existsSync(join(bundle, `${setup}.sig`))) throw new Error(`${setup} or its signature is missing.`);

// 3. The update list.
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

// 4. The public copy, and the scan for private words.
step("Public copy");
if (!existsSync(join(PUBLIC_DIR, ".git"))) {
  mkdirSync(PUBLIC_DIR, { recursive: true });
  run("git", ["init", "-q", "-b", "main"], { cwd: PUBLIC_DIR });
  run("git", ["remote", "add", "origin", `https://github.com/${REPO}.git`], { cwd: PUBLIC_DIR });
}
for (const name of readdirSync(PUBLIC_DIR)) if (name !== ".git") rmSync(join(PUBLIC_DIR, name), { recursive: true, force: true });
const files = out("git", ["ls-files"]).split("\n").filter((f) => f && !PRIVATE.some((p) => p.test(f)));
for (const f of files) {
  mkdirSync(dirname(join(PUBLIC_DIR, f)), { recursive: true });
  cpSync(f, join(PUBLIC_DIR, f));
}
run("node", ["scripts/scan-private.mjs", PUBLIC_DIR, "src-tauri/target/release/cablebag.exe", "rdp-helper/publish/cablebag-rdp.exe", "build", join(bundle, setup), join(bundle, "latest.json")]);
run("git", ["add", "-A"], { cwd: PUBLIC_DIR });
if (out("git", ["status", "--porcelain"], { cwd: PUBLIC_DIR })) run("git", [...AUTHOR, "commit", "-q", "-m", `Cablebag ${version}`], { cwd: PUBLIC_DIR });
run("git", [...AUTHOR, "tag", "-f", `v${version}`], { cwd: PUBLIC_DIR });

if (dry) {
  console.log(`\nDry run done: ${join(bundle, setup)} and latest.json are ready, the public copy is in ${PUBLIC_DIR}. Nothing was pushed.`);
  process.exit(0);
}

// 5. Out: the private repository, the public one and the release.
step("Publish");
run("git", ["push", "-q", "origin", "main", `v${version}`]);
try {
  out("gh", ["repo", "view", REPO, "--json", "name"]);
} catch {
  run("gh", ["repo", "create", REPO, "--public", "--description", "All-in-one remote access workstation for Windows: terminals, SSH, RDP, VNC, Proxmox. Free to use, not open source.", "--homepage", `https://github.com/${REPO}/releases/latest`, "--disable-wiki"]);
}
run("git", ["push", "-q", "-f", "origin", "main", `v${version}`], { cwd: PUBLIC_DIR });
run("gh", ["release", "create", `v${version}`, "--repo", REPO, "--title", `Cablebag ${version}`, "--notes", text || `Cablebag ${version}`, join(bundle, setup), join(bundle, `${setup}.sig`), join(bundle, "latest.json")]);
console.log(`\nReleased: https://github.com/${REPO}/releases/tag/v${version}`);
