// Cablebag's own keyboard shortcuts. Terminals and remote screens let these through to the window
// instead of sending them on: otherwise xterm turns Ctrl+Shift+W into Ctrl+W for the shell and the
// window never sees it.
import { macros } from "./macros.svelte";

/** Keys typed by automation or some remote tools carry no physical key code, only the character. */
export const letter = (e: KeyboardEvent, l: string) => (e.code ? e.code === `Key${l}` : e.key.toUpperCase() === l);

export function isAppShortcut(e: KeyboardEvent): boolean {
  if ((e.ctrlKey || e.altKey) && macros.byHotkey(e)) return true;
  if (!e.ctrlKey) return false;
  // Ctrl+Shift+T new terminal, W close tab, D duplicate, N new session, K command palette, H Home,
  // E remote files, plus and minus interface size.
  if (e.shiftKey && !e.altKey && ["T", "W", "D", "N", "K", "H", "E"].some((l) => letter(e, l))) return true;
  if (e.shiftKey && !e.altKey && ["Equal", "Minus", "NumpadAdd", "NumpadSubtract"].includes(e.code)) return true;
  // Ctrl+comma Settings.
  if (e.key === "," && !e.shiftKey && !e.altKey) return true;
  // Ctrl+Tab and Ctrl+Shift+Tab switch tabs.
  if (e.key === "Tab") return true;
  return false;
}
