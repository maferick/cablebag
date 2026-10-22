// Tiling: every open tab gets a pane, arranged as a tree of splits. A new tab splits the pane that had
// the focus along its longer side (the way bspwm and Hyprland's dwindle layout do it).

export type TileNode =
  | { kind: "leaf"; tab: string }
  | { kind: "split"; dir: "row" | "col"; ratio: number; a: TileNode; b: TileNode };

/** Position and size as fractions of the tab area. */
export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Divider {
  node: Extract<TileNode, { kind: "split" }>;
  /** The area the split divides. */
  area: Rect;
}

const leaf = (tab: string): TileNode => ({ kind: "leaf", tab });

function chain(items: TileNode[], dir: "row" | "col"): TileNode {
  if (items.length === 1) return items[0];
  return { kind: "split", dir, ratio: 1 / items.length, a: items[0], b: chain(items.slice(1), dir) };
}

function without(n: TileNode, tab: string): TileNode | null {
  if (n.kind === "leaf") return n.tab === tab ? null : n;
  const a = without(n.a, tab);
  const b = without(n.b, tab);
  if (!a) return b;
  if (!b) return a;
  return { ...n, a, b };
}

function leaves(n: TileNode | null, out: string[] = []): string[] {
  if (!n) return out;
  if (n.kind === "leaf") out.push(n.tab);
  else {
    leaves(n.a, out);
    leaves(n.b, out);
  }
  return out;
}

function findLeaf(n: TileNode, tab: string): Extract<TileNode, { kind: "leaf" }> | null {
  if (n.kind === "leaf") return n.tab === tab ? n : null;
  return findLeaf(n.a, tab) ?? findLeaf(n.b, tab);
}

/** The leaf for a tab and the splits above it, nearest first, each with the side the tab is on. */
function pathTo(n: TileNode, tab: string, up: { node: Extract<TileNode, { kind: "split" }>; side: "a" | "b" }[] = []): typeof up | null {
  if (n.kind === "leaf") return n.tab === tab ? up : null;
  return pathTo(n.a, tab, [{ node: n, side: "a" }, ...up]) ?? pathTo(n.b, tab, [{ node: n, side: "b" }, ...up]);
}

/** How many panes sit next to each other along a direction, to share the space out evenly. */
function weight(n: TileNode, dir: "row" | "col"): number {
  if (n.kind === "leaf") return 1;
  const a = weight(n.a, dir);
  const b = weight(n.b, dir);
  return n.dir === dir ? a + b : Math.max(a, b);
}

const clamp = (r: number) => Math.min(0.9, Math.max(0.1, r));

function place(n: TileNode, r: Rect, rects: Map<string, Rect>, dividers: Divider[]) {
  if (n.kind === "leaf") {
    rects.set(n.tab, r);
    return;
  }
  if (n.dir === "row") {
    const w = r.w * n.ratio;
    place(n.a, { x: r.x, y: r.y, w, h: r.h }, rects, dividers);
    place(n.b, { x: r.x + w, y: r.y, w: r.w - w, h: r.h }, rects, dividers);
  } else {
    const h = r.h * n.ratio;
    place(n.a, { x: r.x, y: r.y, w: r.w, h }, rects, dividers);
    place(n.b, { x: r.x, y: r.y + h, w: r.w, h: r.h - h }, rects, dividers);
  }
  dividers.push({ node: n, area: r });
}

class Tiles {
  root = $state<TileNode | null>(null);
  /** The pane shown alone for a moment, filling the whole area. */
  zoomed = $state<string | null>(null);
  /** Size of the tab area in pixels, to know which side of a pane is longer. */
  stage = $state({ w: 1600, h: 900 });
  /** Raised after every change, for native windows (RDP) that follow their pane. */
  version = $state(0);
  /** Where the next new tab goes: beside or below the focused pane, or along its longer side (null). */
  next = $state<"row" | "col" | null>(null);
  private lastFocus = "";

  get layout(): { rects: Map<string, Rect>; dividers: Divider[] } {
    const rects = new Map<string, Rect>();
    const dividers: Divider[] = [];
    if (this.root) place(this.root, { x: 0, y: 0, w: 1, h: 1 }, rects, dividers);
    return { rects, dividers };
  }

  private changed() {
    this.version++;
  }

  /** Keeps the panes in step with the open tabs: closed tabs leave, new ones split the focused pane. */
  sync(tabs: string[], focused: string) {
    const open = new Set(tabs);
    let root = this.root;
    for (const t of leaves(root)) if (!open.has(t)) root = root && without(root, t);
    const placed = new Set(leaves(root));
    for (const t of tabs) {
      if (placed.has(t)) continue;
      if (!root) {
        root = leaf(t);
      } else {
        const target = placed.has(this.lastFocus) ? this.lastFocus : this.largest(root);
        const rects = new Map<string, Rect>();
        place(root, { x: 0, y: 0, w: 1, h: 1 }, rects, []);
        const r = rects.get(target)!;
        const dir = this.next ?? (r.w * this.stage.w >= r.h * this.stage.h ? "row" : "col");
        this.next = null;
        root = this.replace(root, target, { kind: "split", dir, ratio: 0.5, a: leaf(target), b: leaf(t) });
      }
      placed.add(t);
      this.lastFocus = t;
    }
    if (this.zoomed && !open.has(this.zoomed)) this.zoomed = null;
    if (placed.has(focused)) this.lastFocus = focused;
    // A pane that fills the area hands that over to the tab you switch to; otherwise the switch shows nothing.
    if (this.zoomed && placed.has(focused) && focused !== this.zoomed) {
      this.zoomed = focused;
      this.changed();
    }
    if (root !== this.root) {
      this.root = root;
      this.changed();
    }
  }

  private largest(root: TileNode): string {
    const rects = new Map<string, Rect>();
    place(root, { x: 0, y: 0, w: 1, h: 1 }, rects, []);
    let best = "";
    let area = -1;
    // Panes of the same size: the first one, top left.
    for (const [t, r] of rects) if (r.w * r.h > area + 1e-6) [best, area] = [t, r.w * r.h];
    return best;
  }

  private replace(n: TileNode, tab: string, by: TileNode): TileNode {
    if (n.kind === "leaf") return n.tab === tab ? by : n;
    return { ...n, a: this.replace(n.a, tab, by), b: this.replace(n.b, tab, by) };
  }

  setRatio(node: Divider["node"], ratio: number) {
    node.ratio = clamp(ratio);
    this.changed();
  }

  /**
   * Makes a pane bigger (positive) or smaller (negative) by a share of the area. With an axis only
   * its width ("row") or height ("col") changes, otherwise both.
   */
  grow(tab: string, by: number, axis?: "row" | "col") {
    const path = this.root && pathTo(this.root, tab);
    if (!path) return;
    let done = false;
    for (const dir of axis ? [axis] : (["row", "col"] as const)) {
      const up = path.find((p) => p.node.dir === dir);
      if (!up) continue;
      up.node.ratio = clamp(up.node.ratio + (up.side === "a" ? by : -by));
      done = true;
    }
    if (done) this.changed();
  }

  /** Gives every pane the same share of the space, or only the panes under one split. */
  equalize(from?: Divider["node"]) {
    const even = (n: TileNode) => {
      if (n.kind === "leaf") return;
      const a = weight(n.a, n.dir);
      n.ratio = a / (a + weight(n.b, n.dir));
      even(n.a);
      even(n.b);
    };
    const start = from ?? this.root;
    if (!start) return;
    even(start);
    this.changed();
  }

  /** Turns the split a pane is in: side by side becomes above each other, and back. */
  rotate(tab: string) {
    const up = this.root && pathTo(this.root, tab)?.[0];
    if (!up) return;
    up.node.dir = up.node.dir === "row" ? "col" : "row";
    this.changed();
  }

  /** Puts a pane in the place of the largest one, swapping the two. */
  makeMain(tab: string) {
    if (this.root) this.swap(tab, this.largest(this.root));
  }

  swap(a: string, b: string) {
    if (!this.root || a === b) return;
    const la = findLeaf(this.root, a);
    const lb = findLeaf(this.root, b);
    if (!la || !lb) return;
    la.tab = b;
    lb.tab = a;
    this.changed();
  }

  toggleZoom(tab: string) {
    this.zoomed = this.zoomed === tab ? null : tab;
    this.changed();
  }

  /** The nearest pane in a direction from this one, or null at the edge. */
  neighbour(tab: string, dir: "left" | "right" | "up" | "down"): string | null {
    const { rects } = this.layout;
    const r = rects.get(tab);
    if (!r) return null;
    const cx = r.x + r.w / 2;
    const cy = r.y + r.h / 2;
    const e = 1e-6;
    let best: string | null = null;
    let bestScore = Infinity;
    for (const [t, o] of rects) {
      if (t === tab) continue;
      const ok =
        dir === "left" ? o.x + o.w <= r.x + e : dir === "right" ? o.x >= r.x + r.w - e : dir === "up" ? o.y + o.h <= r.y + e : o.y >= r.y + r.h - e;
      // Panes beside each other must overlap on the other axis.
      const overlap = dir === "left" || dir === "right" ? o.y < r.y + r.h - e && o.y + o.h > r.y + e : o.x < r.x + r.w - e && o.x + o.w > r.x + e;
      if (!ok || !overlap) continue;
      const score = Math.hypot(o.x + o.w / 2 - cx, o.y + o.h / 2 - cy);
      if (score < bestScore) [best, bestScore] = [t, score];
    }
    return best;
  }

  /**
   * Rebuilds the layout from scratch: a grid, all side by side, all above each other, or one main pane
   * on the left (the first tab) with the others stacked beside it.
   */
  arrange(how: "grid" | "columns" | "rows" | "main", tabs: string[]) {
    if (!tabs.length) return;
    const items = tabs.map(leaf);
    if (how === "main") this.root = items.length === 1 ? items[0] : { kind: "split", dir: "row", ratio: 0.6, a: items[0], b: chain(items.slice(1), "col") };
    else if (how === "columns") this.root = chain(items, "row");
    else if (how === "rows") this.root = chain(items, "col");
    else {
      const cols = Math.ceil(Math.sqrt(tabs.length));
      const rows: TileNode[] = [];
      for (let i = 0; i < items.length; i += cols) rows.push(chain(items.slice(i, i + cols), "row"));
      this.root = chain(rows, "col");
    }
    this.zoomed = null;
    this.changed();
  }

  reset() {
    this.root = null;
    this.zoomed = null;
    this.changed();
  }
}

export const tiles = new Tiles();
