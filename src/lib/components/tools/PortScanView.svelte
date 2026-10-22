<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api } from "../../api";
  import { KNOWN_PORTS, COMMON_PORTS } from "../../tools";
  import { ScanSearch, Play, Square } from "@lucide/svelte";
  import "./tool.css";

  let host = $state("");
  let mode = $state<"common" | "range">("common");
  let from = $state(1);
  let to = $state(1024);
  let open = $state<number[]>([]);
  let scanId = $state<string | null>(null);
  let progress = $state({ done: 0, total: 0 });
  let error = $state("");
  let unlisten: (() => void)[] = [];

  onMount(async () => {
    unlisten.push(
      await listen<{ scan: string; port: number }>("scan-port", (e) => {
        if (e.payload.scan === scanId) open = [...open, e.payload.port].sort((a, b) => a - b);
      }),
      await listen<{ scan: string; done: number; total: number; finished: boolean }>("scan-progress", (e) => {
        if (e.payload.scan !== scanId) return;
        progress = { done: Math.max(progress.done, e.payload.done), total: e.payload.total };
        if (e.payload.finished) scanId = null;
      }),
    );
  });

  onDestroy(() => {
    unlisten.forEach((u) => u());
    if (scanId) api.stopScan(scanId);
  });

  function start() {
    error = "";
    if (!host.trim()) return (error = "Fill in a host name or address.");
    const lo = Math.max(1, Math.min(from, to));
    const hi = Math.min(65535, Math.max(from, to));
    const ports = mode === "common" ? COMMON_PORTS : Array.from({ length: hi - lo + 1 }, (_, i) => lo + i);
    open = [];
    progress = { done: 0, total: ports.length };
    scanId = crypto.randomUUID();
    api.portScan(scanId, host.trim(), ports).catch((e) => {
      error = String(e);
      scanId = null;
    });
  }
</script>

<div class="tool-view">
  <header>
    <ScanSearch size={20} />
    <div>
      <h2>Port scanner</h2>
      <p class="muted">Checks which TCP ports of one computer accept connections.</p>
    </div>
  </header>

  <div class="controls">
    <label class="field"><span>Host</span><input bind:value={host} placeholder="Name or address" style="width: 240px" onkeydown={(e) => e.key === "Enter" && !scanId && start()} /></label>
    <label class="field"><span>Which ports</span>
      <select bind:value={mode}>
        <option value="common">Well-known ports ({COMMON_PORTS.length})</option>
        <option value="range">A range</option>
      </select>
    </label>
    {#if mode === "range"}
      <label class="field"><span>From</span><input type="number" min="1" max="65535" bind:value={from} style="width: 90px" /></label>
      <label class="field"><span>To</span><input type="number" min="1" max="65535" bind:value={to} style="width: 90px" /></label>
    {/if}
    {#if scanId}
      <button class="btn" onclick={() => scanId && api.stopScan(scanId)}><Square size={14} /> Stop</button>
    {:else}
      <button class="btn primary" onclick={start}><Play size={14} /> Scan</button>
    {/if}
    {#if progress.total}
      <progress max={progress.total} value={progress.done} style="width: 180px"></progress>
      <span class="muted small">{progress.done} of {progress.total}</span>
    {/if}
  </div>
  {#if error}<p class="error">{error}</p>{/if}

  {#if open.length}
    <table class="grid" style="max-width: 520px">
      <thead><tr><th>Port</th><th>Usually</th></tr></thead>
      <tbody>
        {#each open as p}
          <tr><td class="mono">{p}</td><td>{KNOWN_PORTS[p] ?? "unknown"}</td></tr>
        {/each}
      </tbody>
    </table>
  {:else if !scanId && progress.total && progress.done}
    <p class="muted">No open ports found.</p>
  {/if}
</div>

<style>
  .error {
    color: var(--danger);
  }
</style>
