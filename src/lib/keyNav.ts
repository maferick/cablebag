// Arrow keys between the items of a list that the keyboard otherwise reaches one Tab at a time: the tabs
// (left and right) and the session tree (up and down; right opens a folder, left closes it).

function move(items: HTMLElement[], from: number, to: number) {
  if (!items.length) return;
  items[Math.max(0, Math.min(items.length - 1, to))]?.focus();
}

/** For a container with role="tablist": ArrowLeft/ArrowRight, Home and End move focus between its tabs. */
export function tabKeys(e: KeyboardEvent) {
  const box = e.currentTarget as HTMLElement;
  const items = [...box.querySelectorAll<HTMLElement>('[role="tab"]')];
  const i = items.indexOf(document.activeElement as HTMLElement);
  if (i < 0) return;
  const to = { ArrowLeft: i - 1, ArrowRight: i + 1, Home: 0, End: items.length - 1 }[e.key];
  if (to === undefined) return;
  e.preventDefault();
  move(items, i, to);
}

/** The tree's rows that are on screen (the rows of closed folders are not). */
const rows = (box: HTMLElement) => [...box.querySelectorAll<HTMLElement>('[role="treeitem"]')].filter((el) => el.offsetParent !== null);

/** For a container with role="tree": ArrowUp/ArrowDown, Home and End move focus between its visible rows;
 *  ArrowRight opens a closed folder and ArrowLeft closes an open one (through the row's own Enter). */
export function treeKeys(e: KeyboardEvent) {
  const box = e.currentTarget as HTMLElement;
  const items = rows(box);
  const row = document.activeElement as HTMLElement;
  const i = items.indexOf(row);
  if (i < 0) return;
  const expanded = row.getAttribute("aria-expanded");
  if ((e.key === "ArrowRight" && expanded === "false") || (e.key === "ArrowLeft" && expanded === "true")) {
    e.preventDefault();
    // Bubbling, because Svelte handles keyboard events at the root of the window.
    row.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    // The tree draws the folder's row anew; the focus stays on it (it keeps its place, its rows come below).
    requestAnimationFrame(() => rows(box)[i]?.focus());
    return;
  }
  const to = { ArrowUp: i - 1, ArrowDown: i + 1, Home: 0, End: items.length - 1 }[e.key];
  if (to === undefined) return;
  e.preventDefault();
  move(items, i, to);
}
