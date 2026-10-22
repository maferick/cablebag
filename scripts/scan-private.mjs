// Refuses to let anything personal out: scans files and folders for the words in a private list that lives
// outside the repository (%USERPROFILE%\.cablebag\private-words.txt, one word per line), as plain text and
// as UTF-16 (the way Windows programs store text). Exits with 1 when a word is found or the list is missing.
// Usage: node scripts/scan-private.mjs <file or folder>...
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { join, relative } from "node:path";

const listFile = join(homedir(), ".cablebag", "private-words.txt");
if (!existsSync(listFile)) {
  console.error(`The list of private words is missing (${listFile}); refusing to publish without it.`);
  process.exit(1);
}
const words = readFileSync(listFile, "utf8")
  .split(/\r?\n/)
  .map((w) => w.trim().toLowerCase())
  .filter((w) => w && !w.startsWith("#"));
// The Windows user name hides in paths, so it is always on the list.
const user = (process.env.USERNAME ?? "").toLowerCase();
if (user.length >= 3) words.push(user);

const needles = words.flatMap((w) => [Buffer.from(w, "latin1"), Buffer.from(w, "utf16le")]);
const lower = (b) => {
  const c = Buffer.from(b);
  for (let i = 0; i < c.length; i++) if (c[i] >= 65 && c[i] <= 90) c[i] += 32;
  return c;
};

const skip = new Set([".git", "node_modules", "target", ".svelte-kit"]);
const hits = [];
function scan(path, root) {
  const st = statSync(path);
  if (st.isDirectory()) {
    for (const name of readdirSync(path)) if (!skip.has(name)) scan(join(path, name), root);
    return;
  }
  const data = lower(readFileSync(path));
  for (let i = 0; i < needles.length; i++) {
    if (data.includes(needles[i])) hits.push(`${relative(root, path) || path}: word ${Math.floor(i / 2) + 1} of the private list`);
  }
}
for (const target of process.argv.slice(2)) scan(target, statSync(target).isDirectory() ? target : ".");

if (hits.length) {
  console.error(`Private words found, nothing may be published:\n  ${[...new Set(hits)].join("\n  ")}`);
  process.exit(1);
}
console.log(`Clean: none of the ${words.length} private words in ${process.argv.slice(2).join(", ")}.`);
