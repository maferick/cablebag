// Browser keyboard events to X11 keysyms, the key codes VNC uses.

const SPECIAL: Record<string, number> = {
  Backspace: 0xff08, Tab: 0xff09, Enter: 0xff0d, Escape: 0xff1b, Delete: 0xffff, Insert: 0xff63,
  Home: 0xff50, End: 0xff57, PageUp: 0xff55, PageDown: 0xff56,
  ArrowLeft: 0xff51, ArrowUp: 0xff52, ArrowRight: 0xff53, ArrowDown: 0xff54,
  ShiftLeft: 0xffe1, ShiftRight: 0xffe2, ControlLeft: 0xffe3, ControlRight: 0xffe4,
  AltLeft: 0xffe9, AltRight: 0xfe03, MetaLeft: 0xffeb, MetaRight: 0xffec, CapsLock: 0xffe5,
  ContextMenu: 0xff67, PrintScreen: 0xff61, ScrollLock: 0xff14, Pause: 0xff13, NumLock: 0xff7f,
};

export function keysym(e: KeyboardEvent): number | null {
  if (SPECIAL[e.code] !== undefined) return SPECIAL[e.code];
  if (SPECIAL[e.key] !== undefined) return SPECIAL[e.key];
  const f = /^F(\d{1,2})$/.exec(e.key);
  if (f) return 0xffbe + Number(f[1]) - 1;
  if ([...e.key].length === 1) {
    const cp = e.key.codePointAt(0)!;
    if (cp < 0x100) return cp; // Latin-1 keysyms equal the character code
    return 0x01000000 + cp; // Unicode keysyms
  }
  return null;
}
