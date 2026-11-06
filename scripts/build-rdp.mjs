// The Remote Desktop helper is a Windows program; builds for other systems skip it.
import { execFileSync } from "node:child_process";
if (process.platform !== "win32") {
  console.log(`cablebag-rdp helper: Windows only, skipped on ${process.platform}.`);
} else {
  execFileSync("dotnet", ["publish", "rdp-helper/CablebagRdp.csproj", "-c", "Release", "-o", "rdp-helper/publish", "-nologo", "-v", "q"], { stdio: "inherit" });
}
