// Unread counts of web tabs, read from the page title the way chat and mail sites show them ("(3) Chat |
// Microsoft Teams", "Inbox (12) - ..."), so no site needs a script of its own. The total goes on the taskbar.
import { invoke } from "@tauri-apps/api/core";

/** The unread count a page shows in its title; 0 when it shows none. */
export function unreadFromTitle(title: string): number {
  // One to three digits and a space after, so a title such as "(2024) Annual report" is not a count.
  const m = title.match(/^\s*\((\d{1,3})\+?\)\s/) ?? title.match(/\b(?:Inbox|Postvak IN)\s*\((\d{1,4})\)/i);
  return m ? Number(m[1]) : 0;
}

const SIZE = 32;

/** A red disc with the number, as RGBA pixels for the taskbar button. */
function badgePixels(n: number): number[] {
  const c = document.createElement("canvas");
  c.width = c.height = SIZE;
  const g = c.getContext("2d")!;
  g.fillStyle = "#c42b1c";
  g.beginPath();
  g.arc(SIZE / 2, SIZE / 2, SIZE / 2 - 0.5, 0, Math.PI * 2);
  g.fill();
  const text = n > 99 ? "99+" : String(n);
  g.fillStyle = "#ffffff";
  g.font = `bold ${text.length > 2 ? 13 : text.length > 1 ? 17 : 21}px Segoe UI, sans-serif`;
  g.textAlign = "center";
  g.textBaseline = "middle";
  g.fillText(text, SIZE / 2, SIZE / 2 + 1);
  return Array.from(g.getImageData(0, 0, SIZE, SIZE).data);
}

let shown = 0;

/** Puts the total on Cablebag's taskbar button (nothing when it is 0). */
export function showOnTaskbar(total: number) {
  if (total === shown) return;
  shown = total;
  invoke("window_badge", { rgba: total ? badgePixels(total) : null, size: SIZE }).catch(() => {});
}
