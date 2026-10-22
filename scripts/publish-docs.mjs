// Publishes the README and its pictures to the public repository without a new version:
// node scripts/publish-docs.mjs [--dry]
//
// Only README.md, assets/ and branding/ are copied, so code that is not released yet stays private. The same
// scan for private words as in a release runs first. The public repository gets a commit on main; no tag, no
// release, nothing for installed copies to update.
import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";

const PUBLIC_DIR = join(dirname(process.cwd()), "cablebag-public");
const AUTHOR = ["-c", "user.name=maferick", "-c", "user.email=5728058+maferick@users.noreply.github.com"];
const DOCS = [/^README\.md$/, /^assets\//, /^branding\//];
const dry = process.argv.includes("--dry");

const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { stdio: "inherit", shell: false, ...opts });
const out = (cmd, args, opts = {}) => execFileSync(cmd, args, { encoding: "utf8", ...opts }).trim();

if (out("git", ["status", "--porcelain"])) throw new Error("The working tree has uncommitted changes; commit them first.");
if (!existsSync(join(PUBLIC_DIR, ".git"))) throw new Error(`${PUBLIC_DIR} is missing: make a release first.`);
// The public copy must be what GitHub has, or the push below would be refused.
run("git", ["pull", "-q", "--ff-only", "origin", "main"], { cwd: PUBLIC_DIR });

const files = out("git", ["ls-files"]).split("\n").filter((f) => f && DOCS.some((p) => p.test(f)));
for (const f of files) {
  mkdirSync(dirname(join(PUBLIC_DIR, f)), { recursive: true });
  cpSync(f, join(PUBLIC_DIR, f));
}
run("node", ["scripts/scan-private.mjs", ...files.map((f) => join(PUBLIC_DIR, f))]);
run("git", ["add", "-A", "--", ...files], { cwd: PUBLIC_DIR });
if (out("git", ["status", "--porcelain"], { cwd: PUBLIC_DIR })) run("git", [...AUTHOR, "commit", "-q", "-m", "Documentation"], { cwd: PUBLIC_DIR });
if (dry) {
  console.log(`\nDry run done: the commit is in ${PUBLIC_DIR}, nothing was pushed.`);
  process.exit(0);
}
// Also a commit left by an earlier dry run goes out.
if (out("git", ["rev-list", "--count", "origin/main..main"], { cwd: PUBLIC_DIR }) === "0") {
  console.log("Nothing changed in the documentation.");
  process.exit(0);
}
run("git", ["push", "-q", "origin", "main"], { cwd: PUBLIC_DIR });
console.log("\nPublished: https://github.com/maferick/cablebag");
