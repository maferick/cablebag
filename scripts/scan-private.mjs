// Refuses to let anything personal out: scans files and folders for the words in a private list that lives
// outside the repository (%USERPROFILE%\.cablebag\private-words.txt, one word per line), as plain text and
// as UTF-16 (the way Windows programs store text). Exits with 1 when a word is found or the list is missing.
// Usage: node scripts/scan-private.mjs [--packed <file>] <file or folder>...
// A file after --packed is mostly compressed data (the installer), where a short word turns up by chance between
// random bytes; there a word only counts when it stands in readable text. Its programs are scanned unpacked.
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
// Readable text next to a hit: four printable characters on one side, as plain text or as UTF-16.
const printable = (c) => (c >= 32 && c < 127) || c === 9 || c === 10 || c === 13;
function readable(data, at, len, wide) {
  const step = wide ? 2 : 1;
  const side = (from, dir) => {
    for (let k = 0; k < 4; k++) {
      const i = from + dir * k * step;
      if (i < 0 || i + step > data.length || !printable(data[i]) || (wide && data[i + 1] !== 0)) return false;
    }
    return true;
  };
  return side(at - step, -1) || side(at + len, 1);
}
function found(data, needle, wide, packed) {
  if (!packed) return data.includes(needle);
  for (let at = data.indexOf(needle); at !== -1; at = data.indexOf(needle, at + 1)) {
    if (readable(data, at, needle.length, wide)) return true;
  }
  return false;
}
function scan(path, root, packed = false) {
  const st = statSync(path);
  if (st.isDirectory()) {
    for (const name of readdirSync(path)) if (!skip.has(name)) scan(join(path, name), root);
    return;
  }
  const data = lower(readFileSync(path));
  for (let i = 0; i < needles.length; i++) {
    if (found(data, needles[i], i % 2 === 1, packed)) hits.push(`${relative(root, path) || path}: word ${Math.floor(i / 2) + 1} of the private list`);
  }
}
const args = process.argv.slice(2);
for (let a = 0; a < args.length; a++) {
  const packed = args[a] === "--packed";
  if (packed) a++;
  scan(args[a], statSync(args[a]).isDirectory() ? args[a] : ".", packed);
}

if (hits.length) {
  console.error(`Private words found, nothing may be published:\n  ${[...new Set(hits)].join("\n  ")}`);
  process.exit(1);
}
console.log(`Clean: none of the ${words.length} private words in ${args.filter((a) => a !== "--packed").join(", ")}.`);
