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

  show(e: MouseEvent, items: MenuEntry[]) {
    e.preventDefault();
    e.stopPropagation();
    this.items = items;
    this.x = e.clientX;
    this.y = e.clientY;
    this.open = true;
  }

  showAt(el: HTMLElement, items: MenuEntry[]) {
    const r = el.getBoundingClientRect();
    this.items = items;
    this.x = r.left;
    this.y = r.bottom + 2;
    this.open = true;
  }

  close() {
    this.open = false;
  }
}

export const menu = new MenuState();
