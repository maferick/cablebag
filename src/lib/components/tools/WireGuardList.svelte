<script lang="ts">
  // WireGuard networks that Cablebag runs itself: one per profile, only for the sessions that choose it, so two
  // networks with the same addresses can be used at the same time. No adapter or routes in Windows.
  import { onMount } from "svelte";
  import { api, type WireGuardInfo } from "../../api";
  import { wireguard } from "../../wireguard.svelte";
  import { app } from "../../state.svelte";
  import { ask } from "../../ask.svelte";
  import WireGuardEditor from "../WireGuardEditor.svelte";
  import { Plus, Pencil, Trash2, FileUp, Activity } from "@lucide/svelte";

  /** The network in the editor: undefined = closed, null = a new one. */
  let editing = $state<WireGuardInfo | null | undefined>(undefined);
  let testing = $state<Record<string, string>>({});

  onMount(() => {
    wireguard.load();
    // Handshakes, data and open connections follow while the page is open.
    const t = setInterval(() => wireguard.load(), 2000);
    return () => clearInterval(t);
  });

  async function remove(p: WireGuardInfo) {
    const using = app.sessions.filter((s) => s.data?.wireguard === p.id).length + app.folders.filter((f) => f.data?.wireguard === p.id).length;
    const also = using ? ` ${using} session${using === 1 ? "" : "s"} or folder${using === 1 ? "" : "s"} use it and will connect directly again.` : "";
    if (await ask.confirm("Delete WireGuard network", `Delete ${p.name} and its keys?${also}`, { okLabel: "Delete", danger: true })) {
      await api.wireguardDelete(p.id);
      await wireguard.load();
    }
  }

  async function test(p: WireGuardInfo) {
    testing[p.id] = "Shaking hands...";
    try {
      await api.wireguardTest(p.id);
      testing[p.id] = "The server answered.";
    } catch (e) {
      testing[p.id] = String(e);
    }
    await wireguard.load();
  }

  function ago(s: number | null | undefined): string {
    if (s === null || s === undefined) return "no handshake yet";
    if (s < 5) return "handshake just now";
    if (s < 120) return `handshake ${s} seconds ago`;
    return `handshake ${Math.round(s / 60)} minutes ago`;
  }

  function size(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
    return `${(n / 1024 / 1024).toFixed(1)} MB`;
  }

  const users = (id: string) => app.sessions.filter((s) => s.data?.wireguard === id).length;
</script>

<section class="wg">
  <div class="sechead">
    <div>
      <h3>WireGuard networks</h3>
      <p class="muted">
        Cablebag runs these itself, only for the sessions that choose one, so two networks with the same addresses
        work at the same time. Nothing is changed in Windows.
      </p>
    </div>
    <span class="grow"></span>
    <button class="btn" onclick={() => wireguard.importFile()}><FileUp size={14} /> Import .conf</button>
    <button class="btn" onclick={() => (editing = null)}><Plus size={14} /> New WireGuard network</button>
  </div>

  {#if !wireguard.list.length}
    <div class="empty">
      <p class="muted">
        No WireGuard networks yet. The easiest place to add one is where you use it: under Network when you edit a
        session or a folder.
      </p>
    </div>
  {:else}
    <div class="tlist">
      {#each wireguard.list as p (p.id)}
        {@const st = p.status}
        <div class="tcard">
          <div class="thead">
            <span class="smark {st ? (st.handshake !== null ? 'connected' : 'connecting') : 'closed'}"></span>
            <strong class="name">{p.name}</strong>
            {#if users(p.id)}<span class="tag">{users(p.id)} session{users(p.id) === 1 ? "" : "s"}</span>{/if}
            {#if !p.has_key && !p.vault_item}<span class="tag warn">no private key</span>{/if}
            <span class="grow"></span>
            <span class="state" class:running={st?.handshake !== null && !!st}>
              {#if st}{st.connections} open, {ago(st.handshake)}, {size(st.sent)} sent, {size(st.received)} received{:else}Starts when a session needs it{/if}
            </span>
            <button class="btn" onclick={() => test(p)} title="Start the tunnel and wait for the server's answer"><Activity size={14} /> Test</button>
            <button class="icon-btn" title="Edit" aria-label="Edit" onclick={() => (editing = p)}><Pencil size={15} /></button>
            <button class="icon-btn" title="Delete" aria-label="Delete" onclick={() => remove(p)}><Trash2 size={15} /></button>
          </div>
          <div class="route">
            <span class="stop"><em>This side in the tunnel</em><span class="mono">{p.address}</span></span>
            <span class="arrow" aria-hidden="true"></span>
            <span class="stop"><em>Server</em><span class="mono">{p.endpoint}</span></span>
            {#if p.allowed_ips}
              <span class="arrow" aria-hidden="true"></span>
              <span class="stop"><em>Reaches</em><span class="mono">{p.allowed_ips}</span></span>
            {/if}
          </div>
          {#if st?.warning}<div class="msg">{st.warning}</div>{/if}
          {#if testing[p.id]}<div class="msg" class:ok={testing[p.id] === "The server answered."}>{testing[p.id]}</div>{/if}
        </div>
      {/each}
    </div>
  {/if}
</section>

{#if editing !== undefined}
  <WireGuardEditor profile={editing} onclose={() => (editing = undefined)} />
{/if}

<style>
  .wg {
    margin-top: 28px;
    max-width: 1100px;
  }
  .sechead {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin-bottom: 10px;
  }
  .sechead h3 {
    margin: 0 0 4px;
    font-size: 14px;
  }
  .sechead p {
    margin: 0;
    max-width: 640px;
  }
  .grow {
    flex: 1;
  }
  .empty p {
    margin: 0;
  }
  .tlist {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .tcard {
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--chrome);
  }
  .thead {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 8px 8px 12px;
    border-bottom: 1px solid var(--border);
  }
  .name {
    font-weight: 600;
  }
  .tag {
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 3px;
    border: 1px solid var(--border);
    color: var(--muted);
  }
  .tag.warn {
    color: var(--danger-text);
  }
  .state {
    font-size: 12px;
    color: var(--muted);
    margin-right: 4px;
  }
  .state.running {
    color: var(--ok-text);
  }
  .route {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 10px;
    padding: 10px 12px;
    background: var(--bg);
  }
  .stop {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 5px 10px;
    border: 1px solid var(--border);
    border-radius: 3px;
  }
  .stop em {
    font-style: normal;
    font-size: 11px;
    color: var(--muted);
  }
  .stop .mono {
    font-size: 12px;
  }
  .arrow {
    width: 22px;
    height: 1px;
    background: var(--border-strong);
    position: relative;
  }
  .arrow::after {
    content: "";
    position: absolute;
    right: 0;
    top: -3px;
    border: 3.5px solid transparent;
    border-left: 5px solid var(--border-strong);
    border-right: none;
  }
  .msg {
    padding: 0 12px 10px;
    font-size: 12px;
    color: var(--danger-text);
    background: var(--bg);
  }
  .msg.ok {
    color: var(--ok-text);
  }
</style>
