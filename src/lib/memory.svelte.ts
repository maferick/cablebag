// The assistant's memory, kept in Cablebag itself: what it learned per connection (a saved session), per
// folder and in general. It is a synced list (Sync, Git, encrypted per item), visible and editable in the
// session editor and in Settings, Assistant. Text is passed through the secret hiding in Rust before it is
// stored, so a password or token never ends up in it.
import { invoke } from "@tauri-apps/api/core";
import { app } from "./state.svelte";
import type { Session } from "./api";

export interface Memory {
  id: string;
  /** What it is about: one connection, a folder (and everything in it), or everything. */
  scope: "session" | "folder" | "general";
  /** The session or folder id; empty for general. */
  target: string;
  text: string;
  /** Unix time it was written or last changed. */
  at: number;
  by: "assistant" | "you";
}

const KEY = "ai.memory";
/** At most this much memory goes along with a conversation (newest first per level). */
const BUDGET = 6000;

class MemoryStore {
  get all(): Memory[] {
    return (app.settings[KEY] as Memory[]) ?? [];
  }

  private async save(list: Memory[]) {
    await app.setSetting(KEY, list);
  }

  of(scope: Memory["scope"], target = ""): Memory[] {
    return this.all.filter((m) => m.scope === scope && m.target === target).sort((a, b) => b.at - a.at);
  }

  /** The folders a session sits in, nearest first. */
  folderChain(s: Session | undefined): string[] {
    const out: string[] = [];
    let id = s?.folder_id ?? null;
    while (id) {
      out.push(id);
      id = app.folders.find((f) => f.id === id)?.parent_id ?? null;
    }
    return out;
  }

  /** What goes along with a conversation about this session: its own memory, its folders', the general one. */
  relevant(s: Session | undefined): Memory[] {
    const list = [
      ...(s?.id ? this.of("session", s.id) : []),
      ...this.folderChain(s).flatMap((f) => this.of("folder", f)),
      ...this.of("general"),
    ];
    const out: Memory[] = [];
    let used = 0;
    for (const m of list) {
      if (used + m.text.length > BUDGET) continue;
      used += m.text.length;
      out.push(m);
    }
    return out;
  }

  /** Stores a memory after hiding secrets in it. Returns it, or null when nothing was left to remember. */
  async add(scope: Memory["scope"], target: string, text: string, by: Memory["by"]): Promise<Memory | null> {
    const clean = (await invoke<string>("ai_redact", { text: text.trim() }).catch(() => "")).trim();
    if (!clean) return null;
    // The same fact again only refreshes it.
    const same = this.all.find((m) => m.scope === scope && m.target === target && m.text.toLowerCase() === clean.toLowerCase());
    if (same) {
      await this.save(this.all.map((m) => (m === same ? { ...m, at: now() } : m)));
      return same;
    }
    const m: Memory = { id: crypto.randomUUID(), scope, target, text: clean, at: now(), by };
    await this.save([...this.all, m]);
    return m;
  }

  async update(id: string, text: string) {
    const clean = (await invoke<string>("ai_redact", { text: text.trim() }).catch(() => "")).trim();
    if (!clean) return this.remove(id);
    await this.save(this.all.map((m) => (m.id === id ? { ...m, text: clean, at: now(), by: "you" } : m)));
  }

  async remove(id: string) {
    await this.save(this.all.filter((m) => m.id !== id));
  }

  /** Where a memory belongs, in words. */
  label(m: Memory): string {
    if (m.scope === "general") return "General";
    if (m.scope === "folder") return `Folder ${app.folderPath(m.target) || "(removed)"}`;
    const s = app.sessions.find((x) => x.id === m.target);
    return s ? `Session ${s.name || s.host}` : "A removed session";
  }
}

const now = () => Math.floor(Date.now() / 1000);

export const memory = new MemoryStore();
