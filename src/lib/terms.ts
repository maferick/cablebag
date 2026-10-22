import type { Terminal } from "@xterm/xterm";

/** Open xterm instances by tab id, for actions from outside the terminal (save output, macros). */
export const terminals = new Map<string, Terminal>();

type Listener = (text: string) => void;
const listeners = new Map<string, Set<Listener>>();

/** Subscribes to the decoded output of one terminal. Returns the unsubscribe function. */
export function onOutput(tabId: string, fn: Listener): () => void {
  let set = listeners.get(tabId);
  if (!set) listeners.set(tabId, (set = new Set()));
  set.add(fn);
  return () => set!.delete(fn);
}

export function emitOutput(tabId: string, text: string) {
  listeners.get(tabId)?.forEach((fn) => fn(text));
}

/** Plain text of the whole buffer, scrollback included. */
export function bufferText(tabId: string, maxLines = 0): string {
  const t = terminals.get(tabId);
  if (!t) return "";
  const b = t.buffer.active;
  const start = maxLines > 0 ? Math.max(0, b.length - maxLines) : 0;
  const lines: string[] = [];
  for (let i = start; i < b.length; i++) lines.push(b.getLine(i)?.translateToString(true) ?? "");
  while (lines.length && !lines[lines.length - 1]) lines.pop();
  return lines.join("\n") + "\n";
}

/** Writes a dimmed note into a terminal on this PC only (the server never sees it), such as a command the
 *  assistant runs on its own channel, so you see what happens where you are looking. */
export function noteLocal(tabId: string, text: string) {
  const t = terminals.get(tabId);
  if (!t) return;
  t.write(`\r\n\x1b[2m${text.replace(/\r?\n/g, "\r\n")}\x1b[0m\r\n`);
}
