<script lang="ts">
  // A remote text file in Cablebag: edit it, see what changes before saving, keep the server's version as .bak.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { app, type Tab } from "../state.svelte";
  import { ask } from "../ask.svelte";
  import { diffLines, hunks } from "../linediff";
  import Modal from "./Modal.svelte";
  import { Save, RotateCcw, FileText } from "@lucide/svelte";

  let { tab, visible }: { tab: Tab; visible: boolean } = $props();
  const edit = $derived(tab.edit!);

  let original = $state("");
  let text = $state("");
  let mtime = 0;
  let loading = $state(true);
  let error = $state("");
  let saving = $state(false);
  let review = $state(false);
  let backup = $state(true);
  let area = $state<HTMLTextAreaElement>();
  let gutter = $state<HTMLDivElement>();

  const dirty = $derived(!loading && text !== original);
  const lines = $derived(text.split("\n").length);
  const changes = $derived(review ? hunks(diffLines(original, text)) : []);

  // The tab shows a dot and closing asks while there are unsaved changes.
  $effect(() => {
    if (dirty) app.unsaved.add(tab.id);
    else app.unsaved.delete(tab.id);
  });

  onMount(() => {
    load();
    return () => app.unsaved.delete(tab.id);
  });

  async function load() {
    loading = true;
    error = "";
    try {
      const f = await invoke<{ text: string; mtime: number }>("sftp_read_text", { tab: edit.tab, path: edit.path });
      original = f.text;
      text = f.text;
      mtime = f.mtime;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function reload() {
    if (dirty && !(await ask.confirm("Open it again", "Throw away your changes and open the version on the server?", { okLabel: "Throw away", danger: true }))) return;
    load();
  }

  async function save() {
    saving = true;
    try {
      mtime = await invoke<number>("sftp_write_text", { tab: edit.tab, path: edit.path, text, mtime, backup });
      original = text;
      review = false;
      app.notify(`Saved ${edit.path}${backup ? ` (the previous version is ${edit.path.split("/").pop()}.bak)` : ""}`);
    } catch (e) {
      app.notify(String(e), "error");
    } finally {
      saving = false;
    }
  }

  function keydown(e: KeyboardEvent) {
    if (e.ctrlKey && !e.shiftKey && e.key.toLowerCase() === "s") {
      e.preventDefault();
      if (dirty) review = true;
    } else if (e.key === "Tab" && !e.ctrlKey) {
      // A tab character, not leaving the field.
      e.preventDefault();
      const el = e.target as HTMLTextAreaElement;
      const s = el.selectionStart;
      text = text.slice(0, s) + "\t" + text.slice(el.selectionEnd);
      requestAnimationFrame(() => el.setSelectionRange(s + 1, s + 1));
    }
  }
</script>

<div class="editor" style:display={visible ? "flex" : "none"}>
  <div class="bar">
    <FileText size={14} />
    <span class="mono path" title={edit.path}>{edit.path}</span>
    {#if dirty}<span class="muted small">changed</span>{/if}
    <span class="grow"></span>
    <label class="check small" title="Keeps the version on the server as name.bak before saving"><input type="checkbox" bind:checked={backup} /> Keep a .bak</label>
    <button class="btn small" onclick={reload}><RotateCcw size={13} /> Open again</button>
    <button class="btn small primary" disabled={!dirty || saving} onclick={() => (review = true)}><Save size={13} /> Save... <span class="muted">Ctrl+S</span></button>
  </div>
  {#if loading}
    <p class="muted msg">Opening {edit.path}...</p>
  {:else if error}
    <p class="err msg">{error}</p>
  {:else}
    <div class="body">
      <div class="gutter mono" bind:this={gutter} aria-hidden="true">
        {#each { length: lines } as _, i}<div>{i + 1}</div>{/each}
      </div>
      <textarea
        class="mono"
        bind:this={area}
        bind:value={text}
        spellcheck="false"
        wrap="off"
        onkeydown={keydown}
        onscroll={() => gutter && area && (gutter.scrollTop = area.scrollTop)}
      ></textarea>
    </div>
  {/if}
</div>

{#if review}
  <Modal title={`Save ${edit.path.split("/").pop()}`} width={820} onclose={() => (review = false)}>
    <p class="muted small">What changes on the server. Removed lines in red, new lines in green.</p>
    <div class="diff mono">
      {#each changes as l}
        {#if l === null}
          <div class="gap">...</div>
        {:else}
          <div class="dl {l.kind}"><span class="ln">{l.kind === "added" ? "" : l.n}</span><span class="mk">{l.kind === "added" ? "+" : l.kind === "removed" ? "-" : " "}</span>{l.text || " "}</div>
        {/if}
      {/each}
    </div>
    {#snippet footer()}
      <label class="check small"><input type="checkbox" bind:checked={backup} /> Keep the server's version as .bak</label>
      <span class="grow"></span>
      <button class="btn" onclick={() => (review = false)}>Keep editing</button>
      <button class="btn primary" disabled={saving} onclick={save}>{saving ? "Saving..." : "Save to the server"}</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .editor {
    position: absolute;
    inset: 0;
    flex-direction: column;
    background: var(--bg);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .grow {
    flex: 1;
  }
  .small {
    font-size: 12px;
  }
  .msg {
    padding: 16px;
  }
  .err {
    color: var(--danger-text);
  }
  .body {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .gutter {
    overflow: hidden;
    padding: 8px 8px 8px 12px;
    text-align: right;
    color: var(--faint);
    background: var(--panel);
    border-right: 1px solid var(--border);
    user-select: none;
    font-size: 13px;
    line-height: 1.5;
    min-width: 44px;
  }
  textarea {
    flex: 1;
    resize: none;
    border: none;
    outline: none;
    padding: 8px 12px;
    background: var(--field);
    color: var(--text);
    font-size: 13px;
    line-height: 1.5;
    tab-size: 4;
    white-space: pre;
    overflow: auto;
  }
  .diff {
    max-height: 60vh;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 3px;
    font-size: 12px;
    line-height: 1.5;
  }
  .dl {
    white-space: pre;
    padding: 0 8px 0 0;
  }
  .dl.added {
    background: color-mix(in srgb, var(--ok) 16%, transparent);
  }
  .dl.removed {
    background: color-mix(in srgb, var(--danger) 16%, transparent);
  }
  .ln {
    display: inline-block;
    width: 44px;
    padding-right: 8px;
    text-align: right;
    color: var(--faint);
  }
  .mk {
    display: inline-block;
    width: 14px;
    color: var(--muted);
  }
  .gap {
    padding-left: 52px;
    color: var(--faint);
    background: var(--hover);
  }
</style>
