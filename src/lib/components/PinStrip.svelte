<script lang="ts">
  // The strip at the left edge: pinned sites, each with its browser profile's colour and the unread count its page
  // shows. A click goes to its tab (or opens it); drag to reorder; right-click for the other choices.
  import { app } from "../state.svelte";
  import { menu } from "../menu.svelte";
  import { browserProfiles, SHARED } from "../browserProfiles.svelte";
  import { pins, type Pin } from "../pins.svelte";
  import { openInMenu } from "../webMenu.svelte";
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
</script>

<nav class="strip" aria-label="Pinned sites">
  {#each pins.list as p (p.id)}
    {@const t = pins.tab(p)}
    {@const name = label(p)}
    {@const color = p.profile === SHARED ? undefined : browserProfiles.color(p.profile)}
    {@const who = p.profile === SHARED ? "" : ` (${browserProfiles.name(p.profile)})`}
    <button
      class="pin"
      class:active={t && app.activeId === t.id}
      class:asleep={!t}
      class:dragging={dragging === p.id}
      title="{name}{who}{t?.unread ? `, ${t.unread} unread` : ''}"
      aria-label="{name}{who}{t?.unread ? `, ${t.unread} unread` : ''}"
      draggable="true"
      onclick={() => pins.show(p)}
      oncontextmenu={(e) => pinMenu(e, p)}
      ondragstart={(e) => {
        dragging = p.id;
        e.dataTransfer?.setData("text/plain", p.id);
      }}
      ondragend={() => (dragging = null)}
      ondragover={(e) => dragging && e.preventDefault()}
      ondrop={(e) => {
        e.preventDefault();
        if (dragging) pins.move(dragging, p.id);
        dragging = null;
      }}
    >
      {#if color}<span class="bar" style:background={color} aria-hidden="true"></span>{/if}
      <span class="letter" aria-hidden="true">{name.trim().charAt(0).toUpperCase()}</span>
      {#if t?.unread}<span class="badge" aria-hidden="true">{t.unread > 99 ? "99+" : t.unread}</span>{/if}
    </button>
  {/each}
  <!-- Dropping below the last pin moves a pin to the end. -->
  <div
    class="end"
    role="presentation"
    ondragover={(e) => dragging && e.preventDefault()}
    ondrop={(e) => {
      e.preventDefault();
      if (dragging) pins.move(dragging, null);
      dragging = null;
    }}
  ></div>
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
  .pin.asleep .letter {
    opacity: 0.6;
  }
  .pin.dragging {
    opacity: 0.4;
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
