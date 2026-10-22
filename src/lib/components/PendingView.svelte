<script lang="ts">
  import { app, type Tab } from "../state.svelte";
  import { kindInfo } from "../kinds";
  import { Pencil } from "@lucide/svelte";

  let { tab }: { tab: Tab } = $props();
  const s = $derived(tab.session!);
  const k = $derived(kindInfo(s.kind));
</script>

<div class="pending">
  <div class="card">
    <span class="ic"><k.icon size={28} /></span>
    <h2>{s.name || s.host}</h2>
    <p>
      Opening <strong>{k.label}</strong> sessions is part of the <em>{k.phase}</em> phase of Cablebag, which is not built yet.
      The session and all its settings are saved, so it will open here as soon as that phase lands.
    </p>
    <dl>
      <dt>Type</dt><dd>{k.label}</dd>
      {#if s.host}<dt>Host</dt><dd class="mono">{s.host}{s.port ? `:${s.port}` : ""}</dd>{/if}
      {#if s.username}<dt>User</dt><dd class="mono">{s.username}</dd>{/if}
      {#if s.folder_id}<dt>Folder</dt><dd>{app.folderPath(s.folder_id)}</dd>{/if}
    </dl>
    {#if s.id}
      <button class="btn" onclick={() => app.editSession(s)}><Pencil size={14} /> Edit session</button>
    {/if}
  </div>
</div>

<style>
  .pending {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--bg);
  }
  .card {
    max-width: 480px;
    padding: 28px;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--raised);
  }
  .ic {
    color: var(--muted);
  }
  h2 {
    margin: 8px 0;
    font-size: 18px;
  }
  p {
    line-height: 1.55;
    color: var(--muted);
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 4px 14px;
    margin: 14px 0 18px;
  }
  dt {
    color: var(--faint);
  }
  dd {
    margin: 0;
    user-select: text;
  }
</style>
