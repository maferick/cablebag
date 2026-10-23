<script lang="ts">
  // Settings, Assistant log: every question with the screen text that went with it, every answer and every command
  // that ran. The newest 2000 are kept.
  import { invoke } from "@tauri-apps/api/core";
  import type { AiLogEntry } from "../assistant.svelte";
  import { ask } from "../ask.svelte";
  import { app } from "../state.svelte";
  import SettingsSection from "./SettingsSection.svelte";
  import { ChevronRight } from "@lucide/svelte";

  let log = $state<AiLogEntry[]>([]);
  let openLog = $state<number | null>(null);
  let logFilter = $state("");
  let logLimit = $state(100);

  $effect(() => {
    loadLog();
  });

  async function loadLog() {
    log = await invoke<AiLogEntry[]>("ai_log", { limit: logLimit, filter: logFilter }).catch(() => []);
  }

  async function clearLog() {
    if (!(await ask.confirm("Clear the log", "Remove everything in the assistant's log? The conversations themselves are not affected.", { okLabel: "Clear", danger: true }))) return;
    await invoke("ai_log_clear").catch((e) => app.notify(String(e), "error"));
    loadLog();
  }

  const time = (t: number) => new Date(t * 1000).toLocaleString();
</script>

<SettingsSection title="What was sent and what ran" desc="The newest 2000 are kept; secrets were hidden before anything went out.">
  <div class="inner">
    <div class="logbar">
      <input placeholder="Show only a server, host or word" bind:value={logFilter} oninput={() => loadLog()} />
      <button class="btn" disabled={!log.length} onclick={clearLog}>Clear the log</button>
    </div>
    {#if !log.length}
      <p class="muted small">{logFilter ? `Nothing matches "${logFilter}".` : "Nothing yet. When you ask the assistant something, the question, the screen text that went with it and the answer show here."}</p>
    {:else}
      <div class="log">
        {#each log as l, i}
          <button class="lrow" onclick={() => (openLog = openLog === i ? null : i)}>
            <span class="chev" class:open={openLog === i}><ChevronRight size={12} /></span>
            <span class="lt">{time(l.at)}</span>
            <span class="ld {l.direction}">{({ sent: "Sent", received: "Answer", error: "Error", ran: "Ran" } as Record<string, string>)[l.direction] ?? l.direction}</span>
            <span class="la">{l.host} · {l.about}</span>
          </button>
          {#if openLog === i}<pre class="ltext">{l.text}</pre>{/if}
        {/each}
      </div>
      {#if log.length >= logLimit}
        <button class="btn more" onclick={() => { logLimit += 200; loadLog(); }}>Show more</button>
      {/if}
    {/if}
  </div>
</SettingsSection>

<style>
  .inner {
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .logbar {
    display: flex;
    gap: 8px;
  }
  .logbar input {
    flex: 1;
  }
  .more {
    align-self: flex-start;
  }
  .small {
    font-size: 12px;
    margin: 0;
  }
  .log {
    border: 1px solid var(--border);
    border-radius: 3px;
    max-height: 560px;
    overflow: auto;
  }
  .lrow {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 8px;
    border: none;
    border-bottom: 1px solid var(--border);
    background: none;
    text-align: left;
    font-size: 12px;
  }
  .lrow:hover {
    background: var(--hover);
  }
  .chev {
    display: inline-flex;
    color: var(--muted);
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .lt {
    color: var(--muted);
    white-space: nowrap;
  }
  .ld {
    width: 52px;
  }
  .ld.error {
    color: var(--danger-text);
  }
  .la {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ltext {
    margin: 0;
    padding: 8px 10px;
    max-height: 260px;
    overflow: auto;
    background: var(--bg);
    border-bottom: 1px solid var(--border);
    font-family: var(--font-mono);
    font-size: 11px;
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
  }
</style>
