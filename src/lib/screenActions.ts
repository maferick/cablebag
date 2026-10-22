// What the assistant can do on a Remote Desktop or VNC screen: click, scroll, press keys and type. Every action
// is described for its approval card first. The assistant points on a scale of 0 to 1000 across and down the
// picture, the way vision models such as Qwen give positions, whatever size the picture has.
import { invoke } from "@tauri-apps/api/core";
import type { Tab } from "./state.svelte";

const reason = { type: "string", description: "Why, in one short sentence the user reads before approving." };
const x = { type: "number", description: "Across the latest picture of the screen: 0 is the left edge, 1000 the right edge." };
const y = { type: "number", description: "Down the latest picture of the screen: 0 is the top edge, 1000 the bottom edge." };

export const SCREEN_TOOLS = [
  {
    type: "function",
    function: {
      name: "screen_click",
      description: "Click on the remote screen at a point of the latest picture. You get a new picture afterwards.",
      parameters: {
        type: "object",
        properties: {
          x,
          y,
          button: { type: "string", enum: ["left", "right", "middle"], description: "Default left." },
          double: { type: "boolean", description: "A double click." },
          reason,
        },
        required: ["x", "y", "reason"],
      },
    },
  },
  {
    type: "function",
    function: {
      name: "screen_scroll",
      description: "Scroll the mouse wheel over a point of the latest picture.",
      parameters: {
        type: "object",
        properties: { x, y, amount: { type: "number", description: "Notches: negative scrolls down, positive up. Default -3." }, reason },
        required: ["x", "y", "reason"],
      },
    },
  },
  {
    type: "function",
    function: {
      name: "screen_type",
      description: "Type text on the remote screen, where the keyboard focus is. A newline presses Enter.",
      parameters: { type: "object", properties: { text: { type: "string" }, reason }, required: ["text", "reason"] },
    },
  },
  {
    type: "function",
    function: {
      name: "screen_keys",
      description: 'Press a key or key combination, such as "enter", "esc", "tab", "ctrl+l", "alt+f4" or "win+r".',
      parameters: { type: "object", properties: { keys: { type: "string" }, reason }, required: ["keys", "reason"] },
    },
  },
];

export interface ScreenArgs {
  x?: number;
  y?: number;
  button?: string;
  double?: boolean;
  amount?: number;
  text?: string;
  keys?: string;
  reason?: string;
}

/** The action in words, for its card. */
export function describeAction(name: string, a: ScreenArgs): string {
  const at = `${Math.round((a.x ?? 0) / 10)}% across, ${Math.round((a.y ?? 0) / 10)}% down`;
  if (name === "screen_click") return `${a.double ? "double-click" : a.button === "right" ? "right-click" : a.button === "middle" ? "middle-click" : "click"} at ${at}`;
  if (name === "screen_scroll") return `scroll ${(a.amount ?? -3) < 0 ? "down" : "up"} ${Math.abs(a.amount ?? -3)} at ${at}`;
  if (name === "screen_type") return `type "${a.text ?? ""}"`;
  return `press ${a.keys ?? ""}`;
}

/** Does the action. Returns a note for the assistant, such as characters that could not be typed. */
export async function performAction(t: Tab, name: string, a: ScreenArgs): Promise<string> {
  if (t.proto === "rdp") {
    const action =
      name === "screen_click"
        ? { cmd: "mouse", x: a.x, y: a.y, button: a.button ?? "left", action: a.double ? "double" : "click" }
        : name === "screen_scroll"
          ? { cmd: "mouse", x: a.x, y: a.y, action: "scroll", amount: a.amount ?? -3 }
          : name === "screen_type"
            ? { cmd: "type", text: a.text ?? "" }
            : { cmd: "keys", keys: a.keys ?? "" };
    const r = await invoke<{ skipped?: string }>("rdp_input", { id: t.id, action });
    return r.skipped ? `These characters could not be typed with this keyboard layout: ${r.skipped}` : "";
  }
  if (t.proto === "vnc") return vnc(t, name, a);
  throw new Error("This tab has no remote screen.");
}

// ---- VNC: pointer events with a button mask, keys as X keysyms ----

const KEYSYMS: Record<string, number> = {
  ctrl: 0xffe3, control: 0xffe3, shift: 0xffe1, alt: 0xffe9, win: 0xffeb, windows: 0xffeb, super: 0xffeb,
  enter: 0xff0d, return: 0xff0d, esc: 0xff1b, escape: 0xff1b, tab: 0xff09, space: 0x20, backspace: 0xff08,
  delete: 0xffff, del: 0xffff, insert: 0xff63, home: 0xff50, end: 0xff57, pageup: 0xff55, pagedown: 0xff56,
  up: 0xff52, down: 0xff54, left: 0xff51, right: 0xff53,
};
for (let i = 1; i <= 12; i++) KEYSYMS[`f${i}`] = 0xffbe + i - 1;

const charSym = (ch: string) => {
  const cp = ch.codePointAt(0)!;
  return cp < 0x100 ? cp : 0x01000000 + cp;
};

async function vnc(t: Tab, name: string, a: ScreenArgs): Promise<string> {
  const send = (kind: string, p = 0, q = 0, r = 0) => invoke("vnc_input", { id: t.id, kind, a: p, b: q, c: r });
  const key = async (sym: number, down: boolean) => send("key", sym, down ? 1 : 0);
  if (name === "screen_click" || name === "screen_scroll") {
    const canvas = document.querySelector<HTMLCanvasElement>(`canvas[data-screen="${t.id}"]`);
    if (!canvas?.width) throw new Error("The remote screen is not showing.");
    const px = Math.min(canvas.width - 1, Math.max(0, Math.round(((a.x ?? 0) / 1000) * canvas.width)));
    const py = Math.min(canvas.height - 1, Math.max(0, Math.round(((a.y ?? 0) / 1000) * canvas.height)));
    await send("pointer", px, py, 0);
    if (name === "screen_scroll") {
      // Wheel up is button 4, down is button 5: one press and release per notch.
      const n = a.amount ?? -3;
      const mask = n > 0 ? 8 : 16;
      for (let i = 0; i < Math.abs(n); i++) {
        await send("pointer", px, py, mask);
        await send("pointer", px, py, 0);
      }
      return "";
    }
    const mask = a.button === "right" ? 4 : a.button === "middle" ? 2 : 1;
    for (let i = 0; i < (a.double ? 2 : 1); i++) {
      await send("pointer", px, py, mask);
      await send("pointer", px, py, 0);
    }
    return "";
  }
  if (name === "screen_type") {
    for (const ch of (a.text ?? "").replace(/\r\n/g, "\n")) {
      const sym = ch === "\n" ? KEYSYMS.enter : ch === "\t" ? KEYSYMS.tab : charSym(ch);
      await key(sym, true);
      await key(sym, false);
    }
    return "";
  }
  const syms = (a.keys ?? "")
    .toLowerCase()
    .split("+")
    .map((s) => s.trim())
    .filter(Boolean)
    .map((s) => {
      const sym = KEYSYMS[s] ?? (s.length === 1 ? charSym(s) : undefined);
      if (sym === undefined) throw new Error(`Unknown key "${s}".`);
      return sym;
    });
  for (const s of syms) await key(s, true);
  for (const s of [...syms].reverse()) await key(s, false);
  return "";
}
