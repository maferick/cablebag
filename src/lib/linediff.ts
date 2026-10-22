// The differences between two texts, line by line (Myers' algorithm), for the "what changes" view before saving
// a remote file.

export interface DiffLine {
  kind: "same" | "added" | "removed";
  text: string;
  /** Line number in the old text (removed, same) or the new text (added). */
  n: number;
}

export function diffLines(oldText: string, newText: string): DiffLine[] {
  const a = oldText.split("\n");
  const b = newText.split("\n");
  const n = a.length;
  const m = b.length;
  const max = n + m;
  const v = new Map<number, number>([[1, 0]]);
  const trace: Map<number, number>[] = [];
  outer: for (let d = 0; d <= max; d++) {
    trace.push(new Map(v));
    for (let k = -d; k <= d; k += 2) {
      let x = k === -d || (k !== d && (v.get(k - 1) ?? -1) < (v.get(k + 1) ?? -1)) ? (v.get(k + 1) ?? 0) : (v.get(k - 1) ?? 0) + 1;
      let y = x - k;
      while (x < n && y < m && a[x] === b[y]) {
        x++;
        y++;
      }
      v.set(k, x);
      if (x >= n && y >= m) break outer;
    }
  }
  // Walk back through the trace to find the path.
  const out: DiffLine[] = [];
  let x = n;
  let y = m;
  for (let d = trace.length - 1; d >= 0 && (x > 0 || y > 0); d--) {
    const vd = trace[d];
    const k = x - y;
    const down = k === -d || (k !== d && (vd.get(k - 1) ?? -1) < (vd.get(k + 1) ?? -1));
    const pk = down ? k + 1 : k - 1;
    const px = vd.get(pk) ?? 0;
    const py = px - pk;
    while (x > px && y > py) {
      out.push({ kind: "same", text: a[x - 1], n: x });
      x--;
      y--;
    }
    if (d > 0) {
      if (down) out.push({ kind: "added", text: b[y - 1], n: y });
      else out.push({ kind: "removed", text: a[x - 1], n: x });
    }
    x = px;
    y = py;
  }
  while (x > 0 && y > 0) {
    out.push({ kind: "same", text: a[x - 1], n: x });
    x--;
    y--;
  }
  return out.reverse();
}

/** Only the changed lines with a few lines around them; gaps become null. */
export function hunks(lines: DiffLine[], around = 3): (DiffLine | null)[] {
  const keep = new Set<number>();
  lines.forEach((l, i) => {
    if (l.kind !== "same") for (let j = Math.max(0, i - around); j <= Math.min(lines.length - 1, i + around); j++) keep.add(j);
  });
  const out: (DiffLine | null)[] = [];
  let last = -2;
  lines.forEach((l, i) => {
    if (!keep.has(i)) return;
    if (i !== last + 1 && out.length) out.push(null);
    out.push(l);
    last = i;
  });
  return out;
}
