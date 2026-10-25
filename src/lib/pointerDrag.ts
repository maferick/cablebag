// Dragging with the pointer itself: press, move, let go. Cablebag keeps Windows' own drag and drop on, so files can
// be dragged in from Explorer; while that is on, the page in the window gets no HTML drag events at all. A small
// label follows the pointer; Escape cancels.

export interface PointerDrag {
  /** The pointer moved far enough: the drag starts. */
  start: () => void;
  /** Every move while dragging, with the element under the pointer that takes drops (closest [data-dt]). */
  move: (target: HTMLElement | null, x: number, y: number) => void;
  /** Let go over the page. */
  drop: () => void;
  /** Always last: after a drop, Escape or letting go. */
  end: () => void;
  /** What the label next to the pointer says. */
  label?: string;
}

/** Starts watching a press; nothing happens until the pointer moves a few pixels with the button down. */
export function pressToDrag(e: PointerEvent, d: PointerDrag) {
  if (e.button !== 0) return;
  // A button or field inside the thing being dragged (a pane's close button) keeps its own press.
  const control = (e.target as HTMLElement | null)?.closest("button, input, select, textarea, a");
  if (control && control !== e.currentTarget) return;
  const sx = e.clientX;
  const sy = e.clientY;
  let started = false;
  let ghost: HTMLDivElement | null = null;

  const move = (ev: PointerEvent) => {
    if (!started) {
      if (Math.hypot(ev.clientX - sx, ev.clientY - sy) < 6) return;
      started = true;
      d.start();
      document.body.classList.add("pointer-dragging");
      if (d.label) {
        ghost = document.createElement("div");
        ghost.className = "drag-ghost";
        ghost.textContent = d.label;
        document.body.appendChild(ghost);
      }
    }
    ev.preventDefault();
    if (ghost) {
      ghost.style.left = `${ev.clientX + 14}px`;
      ghost.style.top = `${ev.clientY + 10}px`;
    }
    const under = document.elementFromPoint(ev.clientX, ev.clientY) as HTMLElement | null;
    d.move(under?.closest<HTMLElement>("[data-dt]") ?? null, ev.clientX, ev.clientY);
  };
  const finish = (dropIt: boolean) => {
    window.removeEventListener("pointermove", move, true);
    window.removeEventListener("pointerup", up, true);
    window.removeEventListener("keydown", key, true);
    ghost?.remove();
    document.body.classList.remove("pointer-dragging");
    if (!started) return;
    if (dropIt) d.drop();
    d.end();
    // The click that ends a drag must not open or fold what is under the pointer.
    const eat = (c: MouseEvent) => {
      c.stopPropagation();
      c.preventDefault();
    };
    window.addEventListener("click", eat, { capture: true, once: true });
    setTimeout(() => window.removeEventListener("click", eat, true), 0);
  };
  const up = () => finish(true);
  const key = (k: KeyboardEvent) => {
    if (k.key !== "Escape") return;
    k.preventDefault();
    k.stopPropagation();
    finish(false);
  };
  window.addEventListener("pointermove", move, true);
  window.addEventListener("pointerup", up, true);
  window.addEventListener("keydown", key, true);
}
