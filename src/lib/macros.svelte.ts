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
  folder?: string;
  hotkey?: string;
  steps: MacroStep[];
}

class Macros {
  recording = $state<{ tab: string; steps: MacroStep[]; last: number } | null>(null);
  running = $state<string | null>(null);
  private cancel = false;

  get list(): Macro[] {
    return (app.settings["macros"] as Macro[]) ?? [];
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

  /** Types the macro into a terminal tab, waiting for patterns where asked. */
  async run(m: Macro, tab = app.activeId) {
    const t = app.tabs.find((x) => x.id === tab);
    if (!t || t.type !== "terminal") {
      app.notify("Open a terminal tab to run a macro in.", "error");
      return;
    }
    this.cancel = false;
    this.running = m.name;
    let seen = "";
    const off = onOutput(tab, (text) => {
      seen = (seen + text).slice(-8000);
    });
    try {
      for (const s of m.steps) {
        if (this.cancel) break;
        if (s.type === "text") {
          seen = "";
          await api.input(tab, s.text);
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
      this.running = null;
    }
  }

  byHotkey(e: KeyboardEvent): Macro | undefined {
    const parts = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.key.length === 1 ? e.key.toUpperCase() : e.key].filter(Boolean);
    const combo = parts.join("+");
    return this.list.find((m) => m.hotkey && m.hotkey.toLowerCase() === combo.toLowerCase());
  }
}

export const macros = new Macros();

/** Readable text for one macro step, used by the macro editor. */
export function describe(s: MacroStep): string {
  if (s.type === "sleep") return `Pause ${s.ms} ms`;
  if (s.type === "wait") return `Wait for "${s.pattern}"`;
  return `Type ${JSON.stringify(s.text).slice(1, -1).replace(/\\r/g, "⏎")}`;
}
