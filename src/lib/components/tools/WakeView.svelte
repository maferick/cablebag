<script lang="ts">
  import { api } from "../../api";
  import { app } from "../../state.svelte";
  import { Power, Trash2 } from "@lucide/svelte";
  import "./tool.css";

  interface Saved {
    name: string;
    mac: string;
    broadcast: string;
  }

  let name = $state("");
  let mac = $state("");
  let broadcast = $state("");
  let port = $state(9);
  const saved = $derived((app.settings["wol.saved"] as Saved[]) ?? []);

  async function wake(m = mac, b = broadcast, label = name || m) {
    try {
      await api.wake(m, b, port);
      app.notify(`Wake-up sent to ${label}`);
    } catch (e) {
      app.notify(String(e), "error");
    }
  }

  async function remember() {
    if (!mac.trim()) return;
    const list = [...saved.filter((s) => s.mac !== mac.trim()), { name: name.trim() || mac.trim(), mac: mac.trim(), broadcast: broadcast.trim() }];
    await app.setSetting("wol.saved", list);
  }
</script>

<div class="tool-view">
  <header>
    <Power size={20} />
    <div>
      <h2>Wake on LAN</h2>
      <p class="muted">Sends the wake-up packet a sleeping or switched-off computer listens for, if its network card allows it.</p>
    </div>
  </header>
  <div class="controls">
    <label class="field"><span>Name</span><input bind:value={name} placeholder="For example: Media server" style="width: 200px" /></label>
    <label class="field"><span>Hardware (MAC) address</span><input bind:value={mac} placeholder="00:11:22:AA:BB:CC" style="width: 190px" /></label>
    <label class="field"><span>Broadcast address</span><input bind:value={broadcast} placeholder="255.255.255.255" style="width: 160px" /></label>
    <label class="field"><span>Port</span><input type="number" bind:value={port} style="width: 70px" /></label>
    <button class="btn primary" onclick={() => wake()} disabled={!mac.trim()}><Power size={14} /> Wake</button>
    <button class="btn" onclick={remember} disabled={!mac.trim()}>Remember</button>
  </div>
  <p class="muted small">Tip: the network scanner shows the hardware address of computers on your own network.</p>

  {#if saved.length}
    <table class="grid" style="max-width: 640px">
      <thead><tr><th>Name</th><th>Hardware address</th><th></th></tr></thead>
      <tbody>
        {#each saved as s (s.mac)}
          <tr>
            <td>{s.name}</td>
            <td class="mono">{s.mac}</td>
            <td style="text-align: right; white-space: nowrap">
              <button class="btn" onclick={() => wake(s.mac, s.broadcast, s.name)}><Power size={14} /> Wake</button>
              <button class="icon-btn" title="Forget" onclick={() => app.setSetting("wol.saved", saved.filter((x) => x.mac !== s.mac))}><Trash2 size={14} /></button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</div>
