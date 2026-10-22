// Fetches the X server that shows Linux windows from SSH sessions (VcXsrv), for the Windows bundle:
// node scripts/xserver.mjs
//
// VcXsrv is a separate program under the GNU GPL v3, shipped unchanged next to Cablebag in the folder vcxsrv;
// Cablebag only starts it and talks to it over the X protocol. The installer of the official release named in
// scripts/xserver.json is downloaded once into xserver/cache, checked against its SHA-256, and unpacked with
// 7-Zip into xserver/vcxsrv (everything but the installer's own uninstaller and plug-ins). The licence text of
// the same release and a note on where its source code is go next to it.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const X = JSON.parse(readFileSync("scripts/xserver.json", "utf8"));
const ROOT = "xserver";
const CACHE = join(ROOT, "cache");
const DIR = join(ROOT, "vcxsrv");
const STAMP = join(ROOT, "vcxsrv.version");

if (process.platform !== "win32") {
  console.log("The X server is only bundled on Windows; nothing to do.");
  process.exit(0);
}

const sha256 = (buf) => createHash("sha256").update(buf).digest("hex");

/** A file from the cache, or downloaded into it; either way checked against its checksum. */
async function fetchChecked(url, sum) {
  mkdirSync(CACHE, { recursive: true });
  const file = join(CACHE, url.split("/").pop());
  if (existsSync(file) && sha256(readFileSync(file)) === sum) return file;
  console.log(`Downloading ${url}`);
  const res = await fetch(url);
  if (!res.ok) throw new Error(`Download failed (${res.status}): ${url}`);
  const buf = Buffer.from(await res.arrayBuffer());
  const got = sha256(buf);
  if (got !== sum) throw new Error(`Checksum mismatch for ${url}: expected ${sum}, got ${got}`);
  writeFileSync(file, buf);
  return file;
}

function sevenZip() {
  const candidates = ["7z", join(process.env.ProgramFiles ?? "C:\\Program Files", "7-Zip", "7z.exe"), join(process.env["ProgramFiles(x86)"] ?? "C:\\Program Files (x86)", "7-Zip", "7z.exe")];
  for (const c of candidates) {
    try {
      execFileSync(c, ["i"], { stdio: "ignore" });
      return c;
    } catch {}
  }
  throw new Error("7-Zip is needed to unpack the X server (https://www.7-zip.org).");
}

const want = `${X.version} ${X.installerSha256}`;
if (existsSync(STAMP) && readFileSync(STAMP, "utf8").trim() === want && existsSync(join(DIR, "vcxsrv.exe"))) {
  console.log(`${X.name} ${X.version} is ready in ${DIR}.`);
  process.exit(0);
}

const installer = await fetchChecked(X.installer, X.installerSha256);
const licence = await fetchChecked(X.licenceText, X.licenceTextSha256);
rmSync(DIR, { recursive: true, force: true });
rmSync(STAMP, { force: true });
execFileSync(sevenZip(), ["x", "-y", `-o${DIR}`, installer, "-x!$PLUGINSDIR", "-x!uninstall.exe"], { stdio: ["ignore", "ignore", "inherit"] });
if (!existsSync(join(DIR, "vcxsrv.exe"))) throw new Error(`vcxsrv.exe is missing after unpacking ${installer}.`);
writeFileSync(join(DIR, "COPYING"), readFileSync(licence));
writeFileSync(
  join(DIR, "SOURCE.txt"),
  [
    `${X.name} ${X.version}, the X server Cablebag uses to show windows of Linux programs.`,
    "",
    `This folder holds the files of the official ${X.name} ${X.version} release, unchanged:`,
    X.installer,
    "",
    `${X.name} is free software under the GNU General Public License version 3 (see COPYING); its components`,
    "carry their own notices in the source code. It is a separate program: Cablebag starts it and talks to it",
    "over the X protocol, and the licence of Cablebag does not apply to it.",
    "",
    "The complete corresponding source code of this release:",
    X.source,
    X.sourceArchive,
    "",
    "For at least three years after the last Cablebag release that contains this version, a copy of that source",
    "code is also available from the Cablebag maintainer at no charge beyond the cost of providing it: ask in an",
    "issue at https://github.com/maferick/cablebag/issues.",
    "",
  ].join("\r\n"),
);
writeFileSync(STAMP, want + "\n");
console.log(`${X.name} ${X.version} unpacked into ${DIR}.`);
