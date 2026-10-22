<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, type HostResult } from "../../api";
  import { app, newSession } from "../../state.svelte";
  import { KNOWN_PORTS, PROBE_PORTS } from "../../tools";
  import { writeText } from "../../clipboard";
  import { Radar, Play, Square, Terminal, Monitor, Copy } from "@lucide/svelte";
  import "./tool.css";

  let range = $state("");
  let hosts = $state<HostResult[]>([]);
  let scanId = $state<string | null>(null);
  let progress = $state({ done: 0, total: 0 });
  let error = $state("");
  let unlisten: (() => void)[] = [];

  const sorted = $derived(
    [...hosts].sort((a, b) => {
      const n = (ip: string) => ip.split(".").reduce((acc, o) => acc * 256 + Number(o), 0);
      return n(a.ip) - n(b.ip);
    }),
  );

  onMount(async () => {
    const ranges = await api.localRanges();
    if (!range) range = ranges[0] ?? "192.168.1.0/24";
    unlisten.push(
      await listen<HostResult>("scan-host", (e) => {
        if (e.payload.scan !== scanId) return;
        // A host arrives first without name and hardware address, then again once those are known.
        const i = hosts.findIndex((h) => h.ip === e.payload.ip);
        if (i >= 0) hosts[i] = e.payload;
        else hosts = [...hosts, e.payload];
      }),
      await listen<{ scan: string; done: number; total: number; finished: boolean }>("scan-progress", (e) => {
        if (e.payload.scan !== scanId) return;
        // Updates can arrive out of order; the count only goes up.
        progress = { done: Math.max(progress.done, e.payload.done), total: e.payload.total };
        if (e.payload.finished) scanId = null;
      }),
    );
  });

  onDestroy(() => {
    unlisten.forEach((u) => u());
    if (scanId) api.stopScan(scanId);
  });

  async function start() {
    error = "";
    try {
      const total = await api.checkRange(range);
      hosts = [];
      progress = { done: 0, total };
      scanId = crypto.randomUUID();
      api.netScan(scanId, range, PROBE_PORTS).catch((e) => {
        error = String(e);
        scanId = null;
      });
    } catch (e) {
      error = String(e);
    }
  }

  function open(h: HostResult, kind: "ssh" | "rdp") {
    const s = newSession(kind);
    s.host = h.ip;
    s.name = h.name || h.ip;
    app.openSession(s);
  }
</script>

<div class="tool-view">
  <header>
    <Radar size={20} />
    <div>
      <h2>Network scanner</h2>
      <p class="muted">Finds computers that answer a ping or one of the common service ports.</p>
    </div>
  </header>

  <div class="controls">
    <label class="field"><span>Addresses</span>
      <input bind:value={range} placeholder="192.168.1.0/24 or 192.168.1.10-50" style="width: 280px" onkeydown={(e) => e.key === "Enter" && !scanId && start()} />
    </label>
    {#if scanId}
      <button class="btn" onclick={() => scanId && api.stopScan(scanId)}><Square size={14} /> Stop</button>
    {:else}
      <button class="btn primary" onclick={start}><Play size={14} /> Scan</button>
    {/if}
    {#if progress.total}
      <progress max={progress.total} value={progress.done} style="width: 200px"></progress>
      <span class="muted small">{progress.done} of {progress.total} checked, {hosts.length} found</span>
    {/if}
  </div>
  {#if error}<p class="error">{error}</p>{/if}

  {#if sorted.length}
    <table class="grid">
      <thead><tr><th>Address</th><th>Name</th><th>Hardware address</th><th>Ping</th><th>Open ports</th><th></th></tr></thead>
      <tbody>
        {#each sorted as h (h.ip)}
          <tr>
            <td class="mono">{h.ip}</td>
            <td>{h.name || "—"}</td>
            <td class="mono small">{h.mac || "—"}</td>
            <td>{h.rtt_ms !== null ? `${h.rtt_ms} ms` : "no answer"}</td>
            <td>
              {#each h.open_ports as p}<span class="port" title={String(p)}>{KNOWN_PORTS[p] ?? p}</span>{/each}
            </td>
            <td class="act">
              {#if h.open_ports.includes(22)}<button class="icon-btn" title="Open SSH" aria-label="Open SSH" onclick={() => open(h, "ssh")}><Terminal size={15} /></button>{/if}
              {#if h.open_ports.includes(3389)}<button class="icon-btn" title="Open Remote Desktop" aria-label="Open Remote Desktop" onclick={() => open(h, "rdp")}><Monitor size={15} /></button>{/if}
              <button class="icon-btn" title="Copy address" aria-label="Copy address" onclick={() => writeText(h.ip)}><Copy size={15} /></button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else if !scanId && progress.total}
    <p class="muted">Nothing answered in this range.</p>
  {/if}
</div>

<style>
  .port {
    display: inline-block;
    margin: 0 4px 3px 0;
    padding: 1px 7px;
    border-radius: 3px;
    background: var(--panel-2);
    font-size: 11px;
  }
  .act {
    white-space: nowrap;
    text-align: right;
  }
  .error {
    color: var(--danger);
  }
</style>
