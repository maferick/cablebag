// A problem report as GitHub's bug form (.github/ISSUE_TEMPLATE/bug_report.yml) with its fields filled in
// through the address. Cablebag never posts anything itself: the browser opens the form, the user signs in,
// reads it over and submits it there.

export const ISSUE_FORM = "https://github.com/maferick/cablebag/issues/new";

/** The choices of the form's "Session type" dropdown, word for word: GitHub picks the option by its text. */
export const SESSION_TYPES = ["SSH", "SFTP", "RDP", "VNC", "Telnet", "Serial", "FTP", "Local shell/WSL", "Web", "Proxmox", "WireGuard", "Assistant", "Other"];

/** Browsers take far longer addresses, but GitHub answers "URI too long" well before they do. */
export const MAX_URL = 6000;

export interface Report {
  title: string;
  version: string;
  windows: string;
  session: string;
  what: string;
  expected: string;
  steps: string;
  extra: string;
}

/** The text fields that may be shortened, with the form's field id and the heading in the full text. */
const LONG: { key: keyof Report; id: string; heading: string }[] = [
  { key: "what", id: "what-happened", heading: "What happened" },
  { key: "expected", id: "expected", heading: "What you expected" },
  { key: "steps", id: "steps", heading: "Steps to make it happen again" },
  { key: "extra", id: "extra", heading: "Anything else" },
];

const SHORTENED = "\n\n[Shortened: the full report is under Anything else.]";

/** The issue title: the user's own, or the first line of what happened, with the session type in front. */
export function titleOf(r: Report): string {
  const own = r.title.trim() || r.what.trim().split("\n")[0].trim();
  const cut = [...own].length > 90 ? [...own].slice(0, 88).join("").trimEnd() + "..." : own;
  return r.session && cut ? `${r.session}: ${cut}` : cut;
}

function build(r: Report, long: Record<string, string>): string {
  const q = new URLSearchParams({ template: "bug_report.yml", title: titleOf(r), version: r.version, windows: r.windows });
  if (r.session) q.set("session", r.session);
  for (const f of LONG) if (long[f.key]) q.set(f.id, long[f.key]);
  // URLSearchParams writes spaces as "+", which GitHub reads back as spaces; "%20" is the same length times three.
  return `${ISSUE_FORM}?${q}`;
}

/** Everything the user typed, as one text to paste under Anything else when the address cannot hold it. */
export function fullText(r: Report): string {
  return LONG.filter((f) => r[f.key].trim())
    .map((f) => `### ${f.heading}\n\n${r[f.key].trim()}`)
    .join("\n\n");
}

/** The address of the filled-in form. When the report does not fit, the longest texts are cut, each with a
 *  note, and `shortened` says so: the full text then goes along by the clipboard. */
export function issueUrl(r: Report): { url: string; shortened: boolean } {
  const long: Record<string, string> = Object.fromEntries(LONG.map((f) => [f.key, r[f.key].trim()]));
  let url = build(r, long);
  if (url.length <= MAX_URL) return { url, shortened: false };
  // The full text goes under Anything else, so that field is left for it.
  long.extra = "";
  const chars: Record<string, string[]> = Object.fromEntries(LONG.map((f) => [f.key, [...long[f.key]]]));
  while ((url = build(r, long)).length > MAX_URL) {
    // Take a tenth off the longest text each round, so the shorter ones stay whole as long as possible.
    const key = LONG.map((f) => f.key).reduce((a, b) => (chars[b].length > chars[a].length ? b : a));
    const n = chars[key].length;
    if (!n) break;
    chars[key] = chars[key].slice(0, n > 40 ? Math.floor(n * 0.9) : 0);
    long[key] = chars[key].length ? chars[key].join("").trimEnd() + SHORTENED : SHORTENED.trim();
  }
  return { url, shortened: true };
}
