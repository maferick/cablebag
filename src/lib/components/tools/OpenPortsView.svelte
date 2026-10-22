<script lang="ts">
  import { onMount } from "svelte";
  import { api, type SocketInfo } from "../../api";
  import { KNOWN_PORTS } from "../../tools";
  import { Network, RefreshCw, Search } from "@lucide/svelte";
  import "./tool.css";

  let rows = $state<SocketInfo[]>([]);
  let loading = $state(false);
  let filter = $state("");
  let onlyListening = $state(true);
  let error = $state("");

  const shown = $derived(
    rows
      .filter((r) => !onlyListening || r.state === "Listen" || r.protocol === "UDP")
      .filter((r) => {
        const q = filter.trim().toLowerCase();
        return !q || [r.process, String(r.local_port), r.remote_address, String(r.remote_port), KNOWN_PORTS[r.local_port] ?? ""].some((v) => v.toLowerCase().includes(q));
      }),
  );

  async function load() {
    loading = true;
    error = "";
    try {
      rows = await api.openPorts();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  const STATE: Record<string, string> = {
    Listen: "Listening", Established: "Connected", TimeWait: "Closing", CloseWait: "Closing", SynSent: "Connecting",
    FinWait1: "Closing", FinWait2: "Closing", LastAck: "Closing", Closing: "Closing", SynReceived: "Connecting",
  };
</script>

<div class="tool-view">
  <header>
    <Network size={20} />
    <div>
      <h2>Open ports on this PC</h2>
      <p class="muted">Which programs listen for connections, or are connected right now.</p>
    </div>
  </header>
  <div class="controls">
    <label class="field"><span>Find</span>
      <div class="search"><Search size={14} /><input bind:value={filter} placeholder="Program or port" /></div>
    </label>
    <label class="check"><input type="checkbox" bind:checked={onlyListening} /> Only listening ports</label>
    <button class="btn" onclick={load} disabled={loading}><RefreshCw size={14} /> Refresh</button>
    <span class="muted small">{shown.length} shown</span>
  </div>
  {#if error}<p class="error">{error}</p>{/if}
  <table class="grid">
    <thead><tr><th>Program</th><th>Protocol</th><th>Address</th><th>Port</th><th>Connected to</th><th>State</th></tr></thead>
    <tbody>
      {#each shown as r, i (i)}
        <tr>
          <td>{r.process || "—"} <span class="muted small">{r.pid || ""}</span></td>
          <td>{r.protocol}</td>
          <td class="mono small">{r.local_address}</td>
          <td class="mono">{r.local_port}{#if KNOWN_PORTS[r.local_port]}<span class="muted small portname">{KNOWN_PORTS[r.local_port]}</span>{/if}</td>
          <td class="mono small">{r.remote_port ? `${r.remote_address}:${r.remote_port}` : ""}</td>
          <td>{STATE[r.state] ?? r.state}</td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--faint);
  }
  .search input {
    width: 220px;
  }
  .error {
    color: var(--danger);
  }
  .portname {
    margin-left: 8px;
    font-family: var(--font-ui);
  }
</style>
