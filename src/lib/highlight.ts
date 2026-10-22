// Colours words such as "error", "warning" and "ok" in terminal output. Works on the byte stream:
// escape sequences are passed through untouched, and words
// are only coloured while the program itself has not set a colour and is not in a full-screen view.

export interface SyntaxSet {
  id: string;
  name: string;
  rules: { color: string; patterns: string[] }[];
  regex?: boolean;
  caseSensitive?: boolean;
}

const C = {
  red: "\x1b[1;31m",
  green: "\x1b[1;32m",
  yellow: "\x1b[1;33m",
  blue: "\x1b[1;34m",
  magenta: "\x1b[1;35m",
  cyan: "\x1b[1;36m",
  underline: "\x1b[4m",
} as Record<string, string>;
const RESET = "\x1b[22;24;39m";

export const BUILTIN_SYNTAX: SyntaxSet[] = [
  { id: "none", name: "None", rules: [] },
  {
    id: "keywords",
    name: "Status words (OK, warning, error)",
    rules: [
      { color: "red", patterns: ["error", "errors", "failed", "failure", "fatal", "denied", "refused", "critical", "unreachable", "timeout", "timed out", "panic", "exception"] },
      { color: "yellow", patterns: ["warning", "warnings", "warn", "deprecated", "degraded", "retrying"] },
      { color: "green", patterns: ["ok", "success", "successful", "succeeded", "done", "passed", "active", "running", "enabled", "online", "healthy"] },
    ],
  },
  {
    id: "network",
    name: "Network devices (Cisco style)",
    rules: [
      { color: "red", patterns: ["down", "shutdown", "administratively down", "err-disabled", "deny", "error"] },
      { color: "green", patterns: ["up", "permit", "connected", "full"] },
      { color: "yellow", patterns: ["notconnect", "half", "warning"] },
      { color: "cyan", patterns: ["interface", "vlan", "ip address", "router", "ospf", "bgp"] },
    ],
  },
];

function escapeRe(s: string) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export function compile(set: SyntaxSet): { re: RegExp; colors: string[] } | null {
  const parts: string[] = [];
  const colors: string[] = [];
  for (const r of set.rules) {
    const pats = r.patterns.map((p) => p.trim()).filter(Boolean);
    if (!pats.length || !C[r.color]) continue;
    const body = pats.map((p) => (set.regex ? p : escapeRe(p))).join("|");
    parts.push(set.regex ? `(${body})` : `(\\b(?:${body})\\b)`);
    colors.push(C[r.color]);
  }
  if (!parts.length) return null;
  try {
    return { re: new RegExp(parts.join("|"), set.caseSensitive ? "g" : "gi"), colors };
  } catch {
    return null;
  }
}

// Escape sequences: CSI, OSC (ended by BEL or ST), and two-byte escapes.
const ESC_RE = /\x1b\[[0-?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[^[\]]/g;

export class Highlighter {
  private colored = false;
  private altScreen = false;
  private tail = "";

  constructor(private rule: { re: RegExp; colors: string[] } | null) {}

  process(text: string): string {
    if (!this.rule) return text;
    // Keep an unfinished escape sequence for the next chunk.
    text = this.tail + text;
    this.tail = "";
    const lastEsc = text.lastIndexOf("\x1b");
    if (lastEsc >= 0 && lastEsc > text.length - 64) {
      ESC_RE.lastIndex = lastEsc;
      const m = ESC_RE.exec(text);
      if (!m || m.index !== lastEsc) {
        this.tail = text.slice(lastEsc);
        text = text.slice(0, lastEsc);
      }
    }
    let out = "";
    let pos = 0;
    ESC_RE.lastIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = ESC_RE.exec(text))) {
      out += this.plain(text.slice(pos, m.index));
      this.track(m[0]);
      out += m[0];
      pos = m.index + m[0].length;
    }
    return out + this.plain(text.slice(pos));
  }

  private track(seq: string) {
    if (seq.endsWith("m") && seq.startsWith("\x1b[")) {
      const params = seq.slice(2, -1).split(";").filter(Boolean).map(Number);
      if (!params.length || params.includes(0) || params.includes(39)) this.colored = false;
      if (params.some((p) => (p >= 30 && p <= 38) || (p >= 90 && p <= 97))) this.colored = true;
    } else if (/\x1b\[\?(1049|47|1047)[hl]/.test(seq)) {
      this.altScreen = seq.endsWith("h");
    }
  }

  private plain(s: string): string {
    if (!s || this.colored || this.altScreen || !this.rule) return s;
    const { re, colors } = this.rule;
    re.lastIndex = 0;
    return s.replace(re, (...args) => {
      const groups = args.slice(1, 1 + colors.length);
      const i = groups.findIndex((g) => g !== undefined);
      return i >= 0 ? `${colors[i]}${args[0]}${RESET}` : args[0];
    });
  }
}
