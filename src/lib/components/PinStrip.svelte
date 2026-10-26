<script lang="ts">
  // The strip at the left edge: pinned sites, each with its browser profile's colour and the unread count its page
  // shows. A click goes to its tab (or opens it); drag to reorder; right-click for the other choices.
  import { app } from "../state.svelte";
  import { menu } from "../menu.svelte";
  import { browserProfiles, SHARED } from "../browserProfiles.svelte";
  import { pins, type Pin } from "../pins.svelte";
  import { openInMenu } from "../webMenu.svelte";
  import { pressToDrag } from "../pointerDrag";
  import { Pencil, PinOff, Users } from "@lucide/svelte";

  function label(p: Pin): string {
    const s = pins.session(p);
    return s?.name || s?.host || "Site";
  }

  function pinMenu(e: MouseEvent, p: Pin) {
    const s = pins.session(p);
    if (!s) return;
    menu.show(e, [
      { label: "Open in...", icon: Users, action: () => menu.again(openInMenu(s, p.profile)) },
      { label: "Unpin", icon: PinOff, action: () => pins.remove(p.id) },
      { label: "Edit session", icon: Pencil, action: () => app.editSession(s) },
    ]);
  }

  let dragging = $state<string | null>(null);
  /** The pin the dragged one goes in front of ("end": after the last one). */
  let before = $state<string | null>(null);

  function press(e: PointerEvent, p: Pin) {
    pressToDrag(e, {
      label: label(p),
      start: () => (dragging = p.id),
      move: (el) => {
        const dt = el?.dataset.dt ?? "";
        before = dt === "pin-end" ? "end" : dt.startsWith("pin:") ? dt.slice(4) : null;
      },
      drop: () => {
        if (dragging && before) pins.move(dragging, before === "end" ? null : before);
      },
      end: () => {
        dragging = null;
        before = null;
      },
    });
  }
</script>

<nav class="strip" aria-label="Pinned sites">
  {#each pins.list as p (p.id)}
    {@const t = pins.tab(p)}
    {@const name = label(p)}
    {@const color = p.profile === SHARED ? undefined : browserProfiles.color(p.profile)}
    {@const who = p.profile === SHARED ? "" : ` (${browserProfiles.name(p.profile)})`}
    {@const icon = pins.icon(p)}
    <button
      class="pin"
      class:active={t && app.activeId === t.id}
      class:asleep={!t}
      class:dragging={dragging === p.id}
      class:before={dragging && before === p.id && dragging !== p.id}
      data-dt="pin:{p.id}"
      title="{name}{who}{t?.unread ? `, ${t.unread} unread` : ''}"
      aria-label="{name}{who}{t?.unread ? `, ${t.unread} unread` : ''}"
      onclick={() => pins.show(p)}
      oncontextmenu={(e) => pinMenu(e, p)}
      onpointerdown={(e) => press(e, p)}
    >
      {#if color}<span class="bar" style:background={color} aria-hidden="true"></span>{/if}
      {#if icon}
        <img class="icon" src={icon} alt="" draggable="false" />
      {:else}
        <span class="letter" aria-hidden="true">{name.trim().charAt(0).toUpperCase()}</span>
      {/if}
      {#if t?.unread}<span class="badge" aria-hidden="true">{t.unread > 99 ? "99+" : t.unread}</span>{/if}
    </button>
  {/each}
  <!-- Dropping below the last pin moves a pin to the end. -->
  <div class="end" class:before={dragging && before === "end"} role="presentation" data-dt="pin-end"></div>
</nav>

<style>
  .strip {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    width: 42px;
    flex-shrink: 0;
    padding: 10px 0;
    background: var(--panel);
    border-right: 1px solid var(--border);
    overflow-y: auto;
    overflow-x: hidden;
  }
  .pin {
    position: relative;
    flex-shrink: 0;
    width: 30px;
    height: 30px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--raised);
    color: var(--muted);
    font-size: 13px;
    font-weight: 600;
  }
  .pin:hover {
    color: var(--text);
    border-color: var(--faint);
  }
  .pin.active {
    color: var(--text);
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .pin.asleep .letter,
  .pin.asleep .icon {
    opacity: 0.6;
  }
  .icon {
    display: block;
    width: 18px;
    height: 18px;
    margin: auto;
    object-fit: contain;
  }
  .pin.dragging {
    opacity: 0.4;
  }
  /* Where a dragged pin goes: a line above the pin it lands in front of, or above the empty end. */
  .pin.before,
  .end.before {
    box-shadow: 0 -5px 0 -3px var(--accent);
  }
  .bar {
    position: absolute;
    left: -7px;
    top: 8px;
    width: 3px;
    height: 12px;
  }
  .badge {
    position: absolute;
    right: -7px;
    top: -6px;
    min-width: 16px;
    padding: 0 4px;
    border-radius: 8px;
    background: var(--danger);
    color: #fff;
    font-size: 10px;
    font-weight: 600;
    line-height: 15px;
  }
  .end {
    flex: 1;
    width: 100%;
    min-height: 20px;
  }
</style>
