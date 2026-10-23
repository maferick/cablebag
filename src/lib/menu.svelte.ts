export interface MenuItem {
  label: string;
  icon?: any;
  action?: () => void;
  danger?: boolean;
  disabled?: boolean;
  hint?: string;
}

export type MenuEntry = MenuItem | "sep";

class MenuState {
  open = $state(false);
  x = $state(0);
  y = $state(0);
  items = $state<MenuEntry[]>([]);

  /** The element that had focus before the menu opened: it gets it back when the menu closes. */
  private before: HTMLElement | null = null;

  show(e: MouseEvent, items: MenuEntry[]) {
    e.preventDefault();
    e.stopPropagation();
    this.items = items;
    this.before = document.activeElement as HTMLElement | null;
    // The context menu key and Shift+F10 send the same event without a pointer position: open at the element.
    const el = (e.currentTarget ?? e.target) as HTMLElement | null;
    if (!e.clientX && !e.clientY && el?.getBoundingClientRect) {
      const r = el.getBoundingClientRect();
      this.x = r.left + 12;
      this.y = r.bottom;
    } else {
      this.x = e.clientX;
      this.y = e.clientY;
    }
    this.open = true;
  }

  showAt(el: HTMLElement, items: MenuEntry[]) {
    this.before = document.activeElement as HTMLElement | null;
    const r = el.getBoundingClientRect();
    this.items = items;
    this.x = r.left;
    this.y = r.bottom + 2;
    this.open = true;
  }

  close(restoreFocus = false) {
    this.open = false;
    if (restoreFocus && this.before?.isConnected) this.before.focus();
  }
}

export const menu = new MenuState();
