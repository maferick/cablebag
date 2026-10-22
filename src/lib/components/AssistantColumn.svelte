<script lang="ts">
  // The Assistant column at the far right: a conversation about the active tab. Folds to a strip, so it is
  // always there to find (Ctrl+Shift+A).
  import { tick } from "svelte";
  import { app } from "../state.svelte";
  import { api } from "../api";
  import { assistant, parts, SCREEN_LINES } from "../assistant.svelte";
  import { Sparkles, X, Square, Send, ClipboardPaste, Copy, Eraser } from "@lucide/svelte";
  import { writeText } from "../clipboard";

  const tab = $derived(app.active && app.active.type !== "home" ? app.active : undefined);
  const conv = $derived(tab ? assistant.convs[tab.id] : undefined);

  // A tab gets its conversation when the column shows it (not while drawing: that may not change state).
  $effect(() => {
    if (!tab || !assistant.hosts.length) return;
    const c = assistant.conv(tab.id);
    if (!c.hostId) {
      c.hostId = assistant.hosts[0].id;
      c.model = assistant.hosts[0].model;
    }
  });
  const host = $derived(assistant.hosts.find((h) => h.id === conv?.hostId));
  const isTerminal = $derived(tab?.type === "terminal");

  let width = $state(Number(localStorage.getItem("assistant.width")) || 400);
  let question = $state("");
  let list = $state<HTMLDivElement>();
  let input = $state<HTMLTextAreaElement>();

  $effect(() => {
    if (assistant.open) tick().then(() => input?.focus());
  });

  // Keep the newest answer in view while it streams.
  $effect(() => {
    void conv?.msgs.at(-1)?.text;
    tick().then(() => list && (list.scrollTop = list.scrollHeight));
  });

  function drag(e: PointerEvent) {
    const startX = e.clientX;
    const start = width;
    const move = (ev: PointerEvent) => (width = Math.max(300, Math.min(900, start - (ev.clientX - startX))));
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      localStorage.setItem("assistant.width", String(width));
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  function send() {
    if (!tab || !question.trim()) return;
    assistant.ask(tab.id, question);
    question = "";
  }

  /** Puts a one-line command at the prompt without pressing Enter: you read it and run it yourself. Commands
   *  of several lines only get Copy, since a line break would run them. */
  function paste(code: string) {
    if (!tab || !isTerminal) return;
    api.input(tab.id, code.trim());
    app.notify("Pasted into the terminal; press Enter there to run it.");
  }

  function pick(v: string) {
    if (!conv) return;
    const [h, ...m] = v.split("|");
    conv.hostId = h;
    conv.model = m.join("|");
  }
</script>

{#if assistant.open}
  <div class="splitter" role="separator" aria-orientation="vertical" onpointerdown={drag}></div>
  <aside class="assistant" style:width="{width}px">
    <header>
      <Sparkles size={13} />
      <span class="t">Assistant</span>
      {#if conv && assistant.hosts.length}
        <select class="model" value={`${conv.hostId}|${conv.model}`} onchange={(e) => pick((e.target as HTMLSelectElement).value)} title="AI host and model">
          {#each assistant.hosts as h (h.id)}
            {#each [...new Set([h.model, h.vision_model].filter(Boolean))] as m}
              <option value={`${h.id}|${m}`}>{h.name} · {m}</option>
            {/each}
          {/each}
        </select>
      {/if}
      <button class="icon-btn" title="Start over" disabled={!conv?.msgs.length} onclick={() => tab && assistant.clear(tab.id)}><Eraser size={13} /></button>
      <button class="icon-btn" title="Hide the assistant (Ctrl+Shift+A)" onclick={() => (assistant.open = false)}><X size={13} /></button>
    </header>

    {#if !assistant.hosts.length}
      <div class="empty">
        <p>The assistant needs an AI host: any service with an OpenAI-compatible API, such as LiteLLM, vLLM, Ollama or a cloud API.</p>
        <button class="btn primary" onclick={() => app.openSettings("assistant")}>Add an AI host</button>
      </div>
    {:else if !tab}
      <div class="empty"><p class="muted">Open a tab; the assistant talks about the tab you are in.</p></div>
    {:else}
      <div class="about muted small">About: {assistant.describe(tab)}</div>
      <div class="msgs" bind:this={list}>
        {#if !conv?.msgs.length}
          <p class="muted hint">
            Ask about what you see, for example "why does this service not start?" or "what does this error mean?".
            {#if isTerminal}The last {SCREEN_LINES} lines of this terminal go with your question; passwords and other secrets are hidden first.{/if}
            Commands in an answer can be pasted at the prompt; nothing runs until you press Enter there.
          </p>
        {/if}
        {#each conv?.msgs ?? [] as m, i (i)}
          {#if m.role === "user"}
            <div class="q">
              {m.text}
              {#if m.screen}<details class="screen"><summary>Screen sent with it</summary><pre>{m.screen}</pre></details>{/if}
            </div>
          {:else}
            <div class="a">
              {#if m.thinking && !m.text}<p class="muted small thinking">Thinking...</p>{/if}
              {#each parts(m.text) as p}
                {#if p.code}
                  <div class="code">
                    <pre>{p.text}</pre>
                    <div class="codebar">
                      {#if isTerminal && !p.text.trim().includes("\n")}<button class="btn small" onclick={() => paste(p.text)} title="Put it at the prompt without running it"><ClipboardPaste size={13} /> Paste at the prompt</button>{/if}
                      <button class="icon-btn" title="Copy" onclick={() => writeText(p.text)}><Copy size={13} /></button>
                    </div>
                  </div>
                {:else}
                  <p class="text">{p.text.trim()}</p>
                {/if}
              {/each}
              {#if m.error}<p class="err">{m.error}</p>{/if}
            </div>
          {/if}
        {/each}
      </div>
      <div class="ask">
        <textarea
          bind:this={input}
          bind:value={question}
          rows="3"
          placeholder={host ? `Ask ${host.name} about this tab` : "Ask about this tab"}
          onkeydown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              send();
            }
          }}
        ></textarea>
        <div class="askbar">
          {#if isTerminal}
            <label class="check small"><input type="checkbox" bind:checked={assistant.includeScreen} /> Send the screen with it</label>
          {/if}
          <span class="grow"></span>
          {#if conv?.busy}
            <button class="btn" onclick={() => tab && assistant.stop(tab.id)}><Square size={13} /> Stop</button>
          {:else}
            <button class="btn primary" disabled={!question.trim()} onclick={send}><Send size={13} /> Ask</button>
          {/if}
        </div>
      </div>
    {/if}
  </aside>
{:else}
  <button class="strip" title="Show the assistant (Ctrl+Shift+A)" onclick={() => (assistant.open = true)}>
    <Sparkles size={13} />
    <span>Assistant</span>
  </button>
{/if}

<style>
  .splitter {
    width: 4px;
    margin-right: -2px;
    cursor: col-resize;
    z-index: 2;
  }
  .splitter:hover {
    background: var(--border-strong);
  }
  .assistant {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--chrome);
    border-left: 1px solid var(--border);
  }
  header {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 4px 0 10px;
    border-bottom: 1px solid var(--border);
    color: var(--muted);
    font-size: 12px;
  }
  .t {
    font-weight: 600;
  }
  .model {
    flex: 1;
    min-width: 0;
    padding: 2px 6px;
    font-size: 12px;
  }
  .about {
    padding: 6px 12px;
    border-bottom: 1px solid var(--border);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .empty {
    padding: 14px 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    align-items: flex-start;
    line-height: 1.5;
  }
  .empty p {
    margin: 0;
  }
  .msgs {
    flex: 1;
    overflow: auto;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    user-select: text;
  }
  .hint {
    margin: 0;
    line-height: 1.5;
    font-size: 12px;
  }
  .q {
    align-self: flex-end;
    max-width: 92%;
    padding: 7px 10px;
    border-radius: 3px;
    background: var(--chrome-2);
    white-space: pre-wrap;
  }
  .screen summary {
    margin-top: 4px;
    font-size: 11px;
    color: var(--muted);
    cursor: pointer;
  }
  .screen pre {
    max-height: 200px;
    overflow: auto;
    font-size: 11px;
  }
  .a {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .text {
    margin: 0;
    white-space: pre-wrap;
    line-height: 1.5;
  }
  .text:empty {
    display: none;
  }
  .code {
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--bg);
  }
  .code pre,
  .screen pre {
    margin: 0;
    padding: 8px 10px;
    font-family: var(--font-mono);
    font-size: 12px;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .codebar {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
    padding: 3px 4px;
    border-top: 1px solid var(--border);
  }
  .codebar :global(.btn.small) {
    padding: 2px 8px;
    font-size: 12px;
  }
  .err {
    margin: 0;
    color: var(--danger-text);
  }
  .thinking {
    margin: 0;
  }
  .ask {
    border-top: 1px solid var(--border);
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .ask textarea {
    resize: vertical;
    min-height: 54px;
  }
  .askbar {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .grow {
    flex: 1;
  }
  .small {
    font-size: 12px;
  }
  .strip {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    width: 24px;
    padding: 10px 0;
    border: none;
    border-left: 1px solid var(--border);
    border-radius: 0;
    background: var(--chrome);
    color: var(--muted);
    writing-mode: vertical-rl;
    font-size: 12px;
  }
  .strip:hover {
    color: var(--text);
    background: var(--hover);
  }
</style>
