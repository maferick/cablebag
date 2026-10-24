import { ask } from "./ask.svelte";
import { api } from "./api";
import { app } from "./state.svelte";
import { onOutput } from "./terms";

export type MacroStep =
  | { type: "text"; text: string }
  | { type: "sleep"; ms: number }
  | { type: "wait"; pattern: string; timeout: number };

export interface Macro {
  id: string;
  name: string;
  /** The id of the folder whose sessions show this macro; every session when empty. */
  folder?: string;
  hotkey?: string;
  steps: MacroStep[];
}

/** {{name}} in a text step is a blank, asked when the macro runs. A name starts with a letter and has no dots
 *  or quotes, so Go templates such as docker's {{.Names}} or Jinja's {{ item }} stay as they are. */
const BLANK = /\{\{([A-Za-z][A-Za-z0-9_ -]{0,40})\}\}/g;

/** Blanks Cablebag fills in itself, from the terminal's session. */
const AUTOMATIC: Record<string, (tab: string) => string> = {
  host: (tab) => sessionOf(tab)?.host ?? "",
  user: (tab) => sessionOf(tab)?.username ?? "",
  session: (tab) => app.tabs.find((t) => t.id === tab)?.title ?? "",
};

function sessionOf(tab: string) {
  const s = app.tabs.find((t) => t.id === tab)?.session;
  return s ? app.withFolderDefaults(s) : undefined;
}

/** The blanks of a macro that someone has to fill in, in the order they first appear. */
export function blanks(m: Macro): string[] {
  const names: string[] = [];
  for (const s of m.steps)
    if (s.type === "text")
      for (const [, name] of s.text.matchAll(BLANK)) {
        const n = name.trim();
        if (!(n.toLowerCase() in AUTOMATIC) && !names.includes(n)) names.push(n);
      }
  return names;
}

export function fill(text: string, values: Record<string, string>, tab: string): string {
  return text.replace(BLANK, (whole, name: string) => {
    const n = name.trim();
    const auto = AUTOMATIC[n.toLowerCase()];
    return auto ? auto(tab) : (values[n] ?? whole);
  });
}

/** A whole line selected in a terminal starts with the prompt: user@host:~$, [root@host ~]#, PS C:\x>, C:\x>. */
const PROMPT = /^(?:[\w.-]+@[\w.-]+(?::[^$#\n]*)?\s*[$#]\s+|\[[^\]\n]+\][$#]\s+|PS [^>\n]+>\s*|[A-Za-z]:\\[^>\n]*>)/;

export function withoutPrompt(text: string): string {
  return text.replace(PROMPT, "");
}

class Macros {
  recording = $state<{ tab: string; steps: MacroStep[]; last: number } | null>(null);
  running = $state<string | null>(null);
  /** The macro open in the editor, wherever it was opened from. */
  editing = $state<Macro | null>(null);
  private cancel = false;
  /** What was filled in last time, per macro, offered again (only while Cablebag runs: it may be secret). */
  private answers = new Map<string, Record<string, string>>();

  get list(): Macro[] {
    return (app.settings["macros"] as Macro[]) ?? [];
  }

  /** The macros for a tab: those of every session and those of the folder its session is in. */
  forTab(tab: string | undefined): Macro[] {
    const folder = app.tabs.find((t) => t.id === tab)?.session?.folder_id ?? null;
    return this.list.filter((m) => !m.folder || (folder !== null && app.inFolder(folder, m.folder)));
  }

  /** Connected terminals, to run a macro in all of them. */
  get terminals(): string[] {
    return app.tabs.filter((t) => t.type === "terminal" && t.status === "connected").map((t) => t.id);
  }

  /** Opens the editor with a new macro that types the given text, for example a command selected in a terminal. */
  newFromText(text: string, folder?: string) {
    const line = withoutPrompt(text.replace(/\r?\n$/, ""));
    this.editing = {
      id: crypto.randomUUID(),
      name: line.split("\n")[0].slice(0, 40) || `Macro ${this.list.length + 1}`,
      folder,
      steps: [{ type: "text", text: line.replace(/\r?\n/g, "\r") + "\r" }],
    };
  }

  async save(list: Macro[]) {
    await app.setSetting("macros", list);
  }

  startRecording(tab: string) {
    this.recording = { tab, steps: [], last: Date.now() };
  }

  /** Called by a terminal for every keystroke while recording. */
  record(tab: string, data: string) {
    const r = this.recording;
    if (!r || r.tab !== tab) return;
    const now = Date.now();
    const gap = now - r.last;
    r.last = now;
    if (gap > 1500 && r.steps.length) r.steps.push({ type: "sleep", ms: Math.round(gap / 100) * 100 });
    const prev = r.steps[r.steps.length - 1];
    if (prev && prev.type === "text") prev.text += data;
    else r.steps.push({ type: "text", text: data });
  }

  async stopRecording(): Promise<Macro | null> {
    const r = this.recording;
    this.recording = null;
    if (!r || !r.steps.length) return null;
    const name = await ask.text("Save macro", "Name of the new macro", `Macro ${this.list.length + 1}`, "Save");
    if (!name?.trim()) return null;
    const m: Macro = { id: crypto.randomUUID(), name: name.trim(), steps: $state.snapshot(r.steps) as MacroStep[] };
    await this.save([...this.list, m]);
    app.notify(`Saved macro "${m.name}"`);
    return m;
  }

  stop() {
    this.cancel = true;
  }

  /** Types the macro into one or more terminal tabs, after asking for its blanks once. */
  async run(m: Macro, tabs: string[] = [app.activeId]) {
    tabs = tabs.filter((id) => app.tabs.find((x) => x.id === id)?.type === "terminal");
    if (!tabs.length) {
      app.notify("Open a terminal tab to run a macro in.", "error");
      return;
    }
    const names = blanks(m);
    let values: Record<string, string> = {};
    if (names.length) {
      const before = this.answers.get(m.id) ?? {};
      const got = await ask.fields(
        m.name,
        tabs.length > 1 ? `Runs in ${tabs.length} terminals.` : "",
        names.map((n) => ({ name: n, label: n, value: before[n] ?? "" })),
        "Run",
      );
      if (!got) return;
      values = got;
      this.answers.set(m.id, got);
    }
    this.cancel = false;
    this.running = m.name;
    try {
      await Promise.all(tabs.map((tab) => this.runIn(m, tab, values)));
    } finally {
      this.running = null;
    }
  }

  /** Types the macro into one terminal, waiting for patterns where asked. */
  private async runIn(m: Macro, tab: string, values: Record<string, string>) {
    let seen = "";
    const off = onOutput(tab, (text) => {
      seen = (seen + text).slice(-8000);
    });
    try {
      for (const s of m.steps) {
        if (this.cancel) break;
        if (s.type === "text") {
          seen = "";
          await api.input(tab, fill(s.text, values, tab));
        } else if (s.type === "sleep") {
          await new Promise((r) => setTimeout(r, s.ms));
        } else {
          let re: RegExp;
          try {
            re = new RegExp(s.pattern);
          } catch {
            re = new RegExp(s.pattern.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
          }
          const until = Date.now() + (s.timeout || 30000);
          while (!re.test(seen) && Date.now() < until && !this.cancel) await new Promise((r) => setTimeout(r, 100));
          if (!re.test(seen) && !this.cancel) {
            app.notify(`Macro "${m.name}" stopped: "${s.pattern}" did not appear in time.`, "error");
            break;
          }
        }
      }
    } finally {
      off();
    }
  }

  byHotkey(e: KeyboardEvent): Macro | undefined {
    const parts = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.key.length === 1 ? e.key.toUpperCase() : e.key].filter(Boolean);
    const combo = parts.join("+");
    // A folder's macro may share its hotkey with one of another folder: the active tab's folder decides.
    return this.forTab(app.activeId).find((m) => m.hotkey && m.hotkey.toLowerCase() === combo.toLowerCase());
  }
}

export const macros = new Macros();

/** Readable text for one macro step, used by the macro editor. */
export function describe(s: MacroStep): string {
  if (s.type === "sleep") return `Pause ${s.ms} ms`;
  if (s.type === "wait") return `Wait for "${s.pattern}"`;
  return `Type ${JSON.stringify(s.text).slice(1, -1).replace(/\\r/g, "⏎")}`;
}
