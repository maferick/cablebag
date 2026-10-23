// Signs Windows programs with the Cablebag code-signing certificate (Authenticode), so Windows SmartScreen and
// Defender see a known publisher instead of an unknown program: node scripts/sign.mjs <file>...
// With --verify it checks the signatures instead: node scripts/sign.mjs --verify <file>...
// With --signtool it prints the path of signtool.exe (for TAURI_WINDOWS_SIGNTOOL_PATH).
//
// Tauri runs this for cablebag.exe, the installer and every program among the resources. VcXsrv is left
// alone: it ships exactly as its makers published it (see THIRD-PARTY-NOTICES.md), not under our name.
//
// The certificate is picked by its thumbprint in CABLEBAG_SIGN_THUMBPRINT, and the timestamp server comes from
// CABLEBAG_SIGN_TIMESTAMP; scripts/release.mjs sets both from the private release settings. Without them this
// does nothing, so ordinary builds stay unsigned. The private key stays with the certificate's cloud signing
// service or token: its desktop tool puts the certificate in the Windows certificate store, where signtool
// finds it by the thumbprint.
import { execFileSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import { join } from "node:path";

const verify = process.argv.includes("--verify");
const files = process.argv.slice(2).filter((a) => a !== "--verify" && a !== "--signtool");
const thumbprint = (process.env.CABLEBAG_SIGN_THUMBPRINT ?? "").replace(/\s/g, "").toUpperCase();
const timestamp = process.env.CABLEBAG_SIGN_TIMESTAMP ?? "";

if (process.argv.includes("--signtool")) {
  console.log(signtool());
  process.exit(0);
}
if (!thumbprint) {
  if (!verify) console.log(`Not signed (no code-signing certificate set): ${files.join(", ")}`);
  process.exit(0);
}
if (!/^[0-9A-F]{40}$/.test(thumbprint)) throw new Error("CABLEBAG_SIGN_THUMBPRINT is not a certificate thumbprint (40 hexadecimal characters).");
if (!verify && !timestamp) throw new Error("CABLEBAG_SIGN_TIMESTAMP is missing: the timestamp server of the certificate's issuer.");
if (!files.length) throw new Error("Usage: node scripts/sign.mjs [--verify] <file>...");

/** signtool from the newest Windows SDK, or the one in CABLEBAG_SIGNTOOL. */
function signtool() {
  if (process.env.CABLEBAG_SIGNTOOL) return process.env.CABLEBAG_SIGNTOOL;
  const kits = join(process.env["ProgramFiles(x86)"] ?? "C:\\Program Files (x86)", "Windows Kits", "10", "bin");
  const versions = existsSync(kits) ? readdirSync(kits).filter((v) => /^10\./.test(v)) : [];
  versions.sort((a, b) => b.localeCompare(a, undefined, { numeric: true }));
  for (const v of versions) {
    const exe = join(kits, v, "x64", "signtool.exe");
    if (existsSync(exe)) return exe;
  }
  throw new Error("signtool.exe was not found; install the Windows SDK or set CABLEBAG_SIGNTOOL.");
}
const tool = signtool();

for (const file of files) {
  if (!existsSync(file)) throw new Error(`${file} does not exist.`);
  if (/[\\/]vcxsrv[\\/]/i.test(file)) {
    if (!verify) console.log(`Left as its makers ship it: ${file}`);
    continue;
  }
  if (verify) {
    execFileSync(tool, ["verify", "/pa", "/q", file], { stdio: "inherit" });
    console.log(`Signature checked: ${file}`);
    continue;
  }
  const args = ["sign", "/sha1", thumbprint, "/fd", "sha256", "/tr", timestamp, "/td", "sha256", "/d", "Cablebag", "/du", "https://github.com/maferick/cablebag", "/q", file];
  // Timestamp servers hiccup now and then; a few tries before the build fails.
  for (let attempt = 1; ; attempt++) {
    try {
      execFileSync(tool, args, { stdio: "inherit" });
      break;
    } catch (e) {
      if (attempt === 3) throw e;
      console.log(`Signing ${file} failed, trying again (${attempt} of 3).`);
      execFileSync(process.execPath, ["-e", "setTimeout(() => {}, 5000)"]);
    }
  }
  console.log(`Signed: ${file}`);
}
