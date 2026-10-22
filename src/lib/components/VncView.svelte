<script lang="ts">
  import type { Snippet } from "svelte";
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { api, Channel } from "../api";
  import { app, type Tab } from "../state.svelte";
  import { keysym } from "../keysym";
  import { isAppShortcut } from "../shortcuts";
  import { readText, writeText } from "../clipboard";
  import { MonitorSmartphone, Keyboard, ClipboardPaste, RotateCw, Maximize, Minimize } from "@lucide/svelte";

  /** In tiling the bar doubles as the pane header: pane is its name part, paneEnd its buttons. */
  let { tab, visible, pane, paneEnd }: { tab: Tab; visible: boolean; pane?: Snippet<[Tab]>; paneEnd?: Snippet<[Tab]> } = $props();
  let canvas: HTMLCanvasElement;
  let ctx: CanvasRenderingContext2D | null = null;
  let size = $state({ w: 0, h: 0 });
  let fit = $state(true);
  let message = $state("");
  let buttons = 0;
  let area: HTMLDivElement;
  let room = $state({ w: 0, h: 0 });
  let dpr = $state(window.devicePixelRatio || 1);

  // Fit: as large as the tab allows, up or down, keeping the shape. Real size: one remote pixel on one
  // screen pixel, so with Windows display scaling the canvas is smaller in CSS pixels.
  const view = $derived.by(() => {
    if (!size.w || !size.h) return { w: 0, h: 0 };
    if (!fit) return { w: size.w / dpr, h: size.h / dpr };
    const k = Math.min(room.w / size.w, room.h / size.h);
    return k > 0 ? { w: Math.floor(size.w * k), h: Math.floor(size.h * k) } : { w: size.w / dpr, h: size.h / dpr };
  });

  onMount(() => {
    const ro = new ResizeObserver(([e]) => {
      room = { w: e.contentRect.width, h: e.contentRect.height };
      dpr = window.devicePixelRatio || 1;
    });
    ro.observe(area);
    return () => ro.disconnect();
  });

  // Input goes out strictly in order; separate invokes could otherwise overtake each other.
  let queue: Promise<unknown> = Promise.resolve();
  const input = (kind: string, a = 0, b = 0, c = 0, text?: string) => {
    queue = queue.then(() => invoke("vnc_input", { id: tab.id, kind, a, b, c, text }).catch(() => {}));
  };

  // A key is released with the same key code it was pressed with ("H" pressed with Shift is
  // released as "h" by the browser once Shift is up), otherwise the remote side keeps it down.
  const pressed = new Map<string, number>();
  function keyDown(e: KeyboardEvent) {
    if (isAppShortcut(e)) return; // Cablebag's own shortcuts are not sent to the remote screen
    const k = keysym(e);
    if (k === null) return;
    e.preventDefault();
    pressed.set(e.code, k);
    input("key", k, 1);
  }
  function keyUp(e: KeyboardEvent) {
    const k = pressed.get(e.code) ?? keysym(e);
    pressed.delete(e.code);
    if (k === null) return;
    e.preventDefault();
    input("key", k, 0);
  }
  function releaseAll() {
    for (const k of pressed.values()) input("key", k, 0);
    pressed.clear();
  }

  function handle(buf: ArrayBuffer) {
    const v = new DataView(buf);
    const t = v.getUint8(0);
    const u = (i: number) => v.getUint16(1 + i * 2, true);
    if (t === 1) {
      size = { w: u(0), h: u(1) };
      canvas.width = size.w;
      canvas.height = size.h;
      ctx = canvas.getContext("2d");
    } else if (!ctx) {
      return;
    } else if (t === 2) {
      const [x, y, w, h] = [u(0), u(1), u(2), u(3)];
      if (!w || !h) return;
      const px = new Uint8ClampedArray(buf, 9, w * h * 4);
      for (let i = 3; i < px.length; i += 4) px[i] = 255;
      ctx.putImageData(new ImageData(px, w, h), x, y);
    } else if (t === 3) {
      const [dx, dy, w, h, sx, sy] = [u(0), u(1), u(2), u(3), u(4), u(5)];
      ctx.drawImage(canvas, sx, sy, w, h, dx, dy, w, h);
    } else if (t === 4) {
      const [x, y] = [u(0), u(1)];
      createImageBitmap(new Blob([new Uint8Array(buf, 9)], { type: "image/jpeg" })).then((img) => ctx?.drawImage(img, x, y));
    } else if (t === 6) {
      if (tab.session?.data?.shared_clipboard !== false) writeText(new TextDecoder().decode(new Uint8Array(buf, 1)));
    }
  }

  function pos(e: MouseEvent): [number, number] {
    const r = canvas.getBoundingClientRect();
    const x = Math.round(((e.clientX - r.left) / r.width) * size.w);
    const y = Math.round(((e.clientY - r.top) / r.height) * size.h);
    return [Math.max(0, Math.min(size.w - 1, x)), Math.max(0, Math.min(size.h - 1, y))];
  }

  const bit = (b: number) => (b === 0 ? 1 : b === 1 ? 2 : b === 2 ? 4 : 0);

  onMount(() => {
    const un = listen<{ id: string; state: string; message: string }>("term-status", (e) => {
      if (e.payload.id === tab.id) message = e.payload.message;
    });
    const ch = new Channel<ArrayBuffer>();
    ch.onmessage = handle;
    invoke("vnc_open", { id: tab.id, session: $state.snapshot(tab.session!), onData: ch }).catch((e) => {
      message = String(e);
      tab.status = "failed";
    });
    return () => {
      un.then((f) => f());
    };
  });

  onDestroy(() => {
    invoke("vnc_close", { id: tab.id }).catch(() => {});
  });

  $effect(() => {
    if (visible && tab.status === "connected" && app.activeId === tab.id) setTimeout(() => canvas?.focus(), 30);
  });

  const s = $derived(tab.session!);
</script>

<div class="vnc" style:display={visible ? "flex" : "none"}>
  <div class="bar">
    {#if pane}
      {@render pane(tab)}
    {:else}
      <MonitorSmartphone size={14} />
      <span class="where">{s.name || s.host}</span>
      {#if size.w}<span class="muted small size">{size.w} × {size.h}</span>{/if}
      <span class="spacer"></span>
    {/if}
    <button class="btn small" title={fit ? "Show at real size" : "Fit to the tab"} onclick={() => (fit = !fit)}>
      {#if fit}<Maximize size={14} /><span class="lbl">Real size</span>{:else}<Minimize size={14} /><span class="lbl">Fit</span>{/if}
    </button>
    <button class="btn small" disabled={tab.status !== "connected"} onclick={async () => input("clipboard", 0, 0, 0, await readText())} title="Send this PC's clipboard to the remote clipboard">
      <ClipboardPaste size={14} /><span class="lbl">Send clipboard</span>
    </button>
    <button class="btn small" disabled={tab.status !== "connected"} onclick={() => input("cad")} title="Send Ctrl+Alt+Del to the remote computer"><Keyboard size={14} /><span class="lbl">Ctrl+Alt+Del</span></button>
    <button class="btn small" onclick={() => app.restart(tab.id)} title="Connect again"><RotateCw size={14} /><span class="lbl">Reconnect</span></button>
    {#if paneEnd}{@render paneEnd(tab)}{/if}
  </div>
  <div class="area" class:fit bind:this={area}>
    <canvas
      bind:this={canvas}
      data-screen={tab.id}
      style:width="{view.w}px"
      style:height="{view.h}px"
      tabindex="0"
      style:display={size.w && tab.status === "connected" ? "block" : "none"}
      onkeydown={keyDown}
      onkeyup={keyUp}
      onblur={releaseAll}
      onmousemove={(e) => {
        const [x, y] = pos(e);
        input("pointer", x, y, buttons);
      }}
      onmousedown={(e) => {
        canvas.focus();
        buttons |= bit(e.button);
        const [x, y] = pos(e);
        input("pointer", x, y, buttons);
      }}
      onmouseup={(e) => {
        buttons &= ~bit(e.button);
        const [x, y] = pos(e);
        input("pointer", x, y, buttons);
      }}
      oncontextmenu={(e) => e.preventDefault()}
      onwheel={(e) => {
        e.preventDefault();
        const [x, y] = pos(e);
        const w = e.deltaY < 0 ? 8 : 16;
        input("pointer", x, y, buttons | w);
        input("pointer", x, y, buttons);
      }}
    ></canvas>
    {#if tab.status !== "connected"}
      <div class="state">
        {#if tab.status === "connecting"}
          <p>Connecting to <strong>{s.host}</strong>...</p>
          {#if message}<p class="small">{message}</p>{/if}
        {:else}
          <p>{message || "Not connected."}</p>
          <button class="btn primary" onclick={() => app.restart(tab.id)}><RotateCw size={14} /> Connect again</button>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .vnc {
    position: absolute;
    inset: 0;
    container-type: inline-size;
    flex-direction: column;
    background: var(--bg);
  }
  .bar {
    white-space: nowrap;
    overflow: hidden;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 10px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
    color: var(--muted);
  }
  .where {
    color: var(--text);
  }
  .spacer {
    flex: 1;
  }
  .small {
    padding: 3px 9px;
    font-size: 12px;
  }
  .btn.small {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    flex-shrink: 0;
  }
  /* In a narrow pane only the icons stay; the pane header already names the tab. */
  @container (max-width: 640px) {
    .lbl,
    .where,
    .size {
      display: none;
    }
  }

  .area {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    overflow: auto;
    background: #1d1c1a;
  }
  .area.fit {
    overflow: hidden;
  }
  canvas {
    /* Centred while it fits, scrollable from the top left when it does not. */
    margin: auto;
    flex-shrink: 0;
    outline: none;
    cursor: default;
    image-rendering: auto;
  }
  .state {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: #ebe7df;
  }
</style>
