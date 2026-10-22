<script lang="ts">
  import { ask } from "../ask.svelte";
  import { onMount, onDestroy } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { openPath } from "@tauri-apps/plugin-opener";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { api, type Entry } from "../api";
  import { app, type Tab } from "../state.svelte";
  import { menu } from "../menu.svelte";
  import { writeText } from "../clipboard";
  import {
    ArrowUp, RefreshCw, House, FolderPlus, FilePlus, Upload, Download, Trash2, Eye, EyeOff, Folder, File,
    FileSymlink, Pencil, Copy, TerminalSquare, Lock, ExternalLink,
  } from "@lucide/svelte";

  let { tab }: { tab: Tab } = $props();

  // Each SSH tab remembers its own folder.
  const memory: Record<string, string> = (globalThis as any).__tt_paths ??= {};

  let path = $state("");
  let pathInput = $state("");
  let entries = $state<Entry[]>([]);
  let loading = $state(false);
  let error = $state("");
  let showHidden = $state(false);
  let selected = $state<Set<string>>(new Set());
  let unlistenDrop: (() => void) | undefined;

  const visible = $derived(entries.filter((e) => showHidden || !e.name.startsWith(".")));

  async function load(p: string) {
    loading = true;
    error = "";
    try {
      const l = await api.sftpList(tab.id, p);
      path = l.path;
      pathInput = l.path;
      entries = l.entries;
      memory[tab.id] = l.path;
      selected = new Set();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function parent() {
    if (path === "/" || !path) return;
    const p = path.replace(/\/+$/, "");
    load(p.slice(0, p.lastIndexOf("/")) || "/");
  }

  function join(name: string) {
    return path.endsWith("/") ? path + name : `${path}/${name}`;
  }

  async function run(what: string, fn: () => Promise<void>, reload = true) {
    try {
      await fn();
      if (reload) await load(path);
    } catch (e) {
      app.notify(`${what} failed: ${e}`, "error");
    }
  }

  function chosen(): Entry[] {
    return visible.filter((e) => selected.has(e.path));
  }

  async function upload() {
    const files = await openDialog({ multiple: true, title: "Choose files to upload" });
    if (!files) return;
    const list = Array.isArray(files) ? files : [files];
    run("Upload", () => api.sftpUpload(tab.id, list as string[], path));
  }

  async function download(items = chosen()) {
    if (!items.length) return;
    const dir = await openDialog({ directory: true, title: "Download to which folder?" });
    if (!dir || Array.isArray(dir)) return;
    run("Download", () => api.sftpDownload(tab.id, items.map((e) => e.path), dir), false).then(() =>
      app.notify(`Downloaded ${items.length === 1 ? items[0].name : items.length + " items"} to ${dir}`),
    );
  }

  async function remove(items = chosen()) {
    if (!items.length) return;
    const what = items.length === 1 ? `"${items[0].name}"` : `${items.length} items`;
    if (!(await ask.confirm("Delete on the server", `Delete ${what}? Folders are deleted with everything inside them.`, { okLabel: "Delete", danger: true }))) return;
    run("Delete", () => api.sftpRemove(tab.id, items.map((e) => e.path)));
  }

  async function mkdir() {
    const name = await ask.text("New folder", "Name of the new folder", "", "Create");
    if (name?.trim()) run("Create folder", () => api.sftpMkdir(tab.id, join(name.trim())));
  }

  async function newFile() {
    const name = await ask.text("New file", "Name of the new file", "", "Create");
    if (name?.trim()) run("Create file", () => api.sftpNewFile(tab.id, join(name.trim())));
  }

  async function rename(e: Entry) {
    const name = await ask.text("Rename", "New name", e.name, "Rename");
    if (name?.trim() && name !== e.name) run("Rename", () => api.sftpRename(tab.id, e.path, join(name.trim())));
  }

  async function chmod(e: Entry) {
    const cur = (e.mode & 0o777).toString(8).padStart(3, "0");
    const v = await ask.text("Permissions", `Permissions for ${e.name} as a number, for example 644 or 755`, cur, "Apply");
    if (!v || !/^[0-7]{3,4}$/.test(v.trim())) return;
    const recursive = e.is_dir && (await ask.confirm("Permissions", "Apply to everything inside this folder as well?", { okLabel: "Yes, everything" }));
    run("Change permissions", () => api.sftpChmod(tab.id, e.path, parseInt(v.trim(), 8), recursive));
  }

  // Open a remote file with its Windows program and offer to upload it again after it is saved.
  const watches: { local: string; remote: string; mtime: number; tab: string }[] = (globalThis as any).__tt_watch ??= [];
  async function openFile(e: Entry) {
    try {
      const local = await api.sftpFetchTemp(tab.id, e.path);
      const mtime = await api.fileModified(local);
      watches.push({ local, remote: e.path, mtime, tab: tab.id });
      await openPath(local);
    } catch (err) {
      app.notify(`Could not open ${e.name}: ${err}`, "error");
    }
  }

  let timer: ReturnType<typeof setInterval>;
  onMount(() => {
    load(memory[tab.id] ?? "");
    timer = setInterval(async () => {
      for (const w of watches.filter((x) => x.tab === tab.id)) {
        const m = await api.fileModified(w.local);
        if (m && m !== w.mtime) {
          w.mtime = m;
          if (await ask.confirm("File changed", `"${w.remote}" was changed on this PC. Upload it back to the server?`, { okLabel: "Upload" })) {
            run("Upload", () => api.sftpPushBack(w.tab, w.local, w.remote)).then(() => app.notify(`Saved ${w.remote}`));
          }
        }
      }
    }, 2000);
    getCurrentWebview()
      .onDragDropEvent((ev) => {
        const p = ev.payload;
        if (p.type === "drop" && app.activeId === tab.id && (tab.type === "files" || app.filesOpen) && p.paths.length) {
          run("Upload", () => api.sftpUpload(tab.id, p.paths, path));
        }
      })
      .then((u) => (unlistenDrop = u));
  });

  onDestroy(() => {
    clearInterval(timer);
    unlistenDrop?.();
  });

  function activate(e: Entry) {
    if (e.is_dir) load(e.path);
    else openFile(e);
  }

  function pastePath(e: Entry) {
    api.input(tab.id, e.path.includes(" ") ? `'${e.path}'` : e.path);
  }

  function rowClick(ev: MouseEvent, e: Entry) {
    const s = new Set(ev.ctrlKey ? selected : []);
    if (ev.ctrlKey && s.has(e.path)) s.delete(e.path);
    else s.add(e.path);
    selected = s;
  }

  function rowMenu(ev: MouseEvent, e: Entry) {
    if (!selected.has(e.path)) selected = new Set([e.path]);
    menu.show(ev, [
      { label: e.is_dir ? "Open folder" : "Open", icon: ExternalLink, action: () => activate(e) },
      { label: "Download", icon: Download, action: () => download() },
      "sep",
      { label: "Rename", icon: Pencil, action: () => rename(e) },
      { label: "Permissions", icon: Lock, action: () => chmod(e) },
      { label: "Copy path", icon: Copy, action: () => writeText(e.path) },
      { label: "Paste path into terminal", icon: TerminalSquare, action: () => pastePath(e), hint: "Middle click" },
      "sep",
      { label: "Delete", icon: Trash2, danger: true, action: () => remove() },
    ]);
  }

  function size(n: number, dir: boolean) {
    if (dir) return "";
    if (n < 1024) return `${n} B`;
    if (n < 1024 ** 2) return `${(n / 1024).toFixed(1)} KB`;
    if (n < 1024 ** 3) return `${(n / 1024 ** 2).toFixed(1)} MB`;
    return `${(n / 1024 ** 3).toFixed(2)} GB`;
  }

  function date(t: number) {
    if (!t) return "";
    const d = new Date(t * 1000);
    return d.toLocaleDateString(undefined, { day: "2-digit", month: "short", year: "numeric" }) + " " +
      d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  }

  const transfer = $derived(Object.values(app.transfers).find((t) => t.tab === tab.id && !t.finished));
</script>

<div class="fb">
  <div class="tools">
    <button class="icon-btn" title="Parent folder" onclick={parent}><ArrowUp size={15} /></button>
    <button class="icon-btn" title="Home folder" onclick={() => load("")}><House size={15} /></button>
    <button class="icon-btn" title="Refresh" onclick={() => load(path)}><RefreshCw size={15} class={loading ? "spin" : ""} /></button>
    <span class="gap"></span>
    <button class="icon-btn" title="Upload files" onclick={upload}><Upload size={15} /></button>
    <button class="icon-btn" title="Download selected" onclick={() => download()} disabled={!selected.size}><Download size={15} /></button>
    <button class="icon-btn" title="New folder" onclick={mkdir}><FolderPlus size={15} /></button>
    <button class="icon-btn" title="New file" onclick={newFile}><FilePlus size={15} /></button>
    <button class="icon-btn" title="Delete selected" onclick={() => remove()} disabled={!selected.size}><Trash2 size={15} /></button>
    <button class="icon-btn" class:active={showHidden} title={showHidden ? "Hide dot files" : "Show dot files"} onclick={() => (showHidden = !showHidden)}>
      {#if showHidden}<Eye size={15} />{:else}<EyeOff size={15} />{/if}
    </button>
  </div>
  <input
    class="path mono"
    bind:value={pathInput}
    onkeydown={(e) => e.key === "Enter" && load(pathInput)}
    title="Type a path and press Enter"
  />
  {#if error}
    <div class="error">{error}</div>
  {/if}
  <div class="list" role="listbox" tabindex="-1" aria-label="Remote files">
    {#each visible as e (e.path)}
      <div
        class="row"
        class:sel={selected.has(e.path)}
        role="option"
        tabindex="0"
        aria-selected={selected.has(e.path)}
        title={`${e.name}\n${e.permissions}  ${e.owner}:${e.group}\n${date(e.modified)}`}
        onclick={(ev) => rowClick(ev, e)}
        ondblclick={() => activate(e)}
        onkeydown={(ev) => ev.key === "Enter" && activate(e)}
        onauxclick={(ev) => ev.button === 1 && pastePath(e)}
        oncontextmenu={(ev) => rowMenu(ev, e)}
      >
        <span class="ic" class:dir={e.is_dir}>
          {#if e.is_link}<FileSymlink size={14} />{:else if e.is_dir}<Folder size={14} />{:else}<File size={14} />{/if}
        </span>
        <span class="nm">{e.name}</span>
        <span class="sz">{size(e.size, e.is_dir)}</span>
      </div>
    {/each}
    {#if !loading && !error && !visible.length}
      <p class="muted empty">This folder is empty.</p>
    {/if}
  </div>
  <div class="foot">
    {#if transfer}
      <span class="tname">{transfer.name}</span>
      <progress max={transfer.total || 1} value={transfer.done}></progress>
    {:else}
      <span class="muted">{visible.length} items{selected.size ? `, ${selected.size} selected` : ""}. Drop files here to upload.</span>
    {/if}
  </div>
</div>

<style>
  .fb {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
  }
  .tools {
    display: flex;
    align-items: center;
    gap: 1px;
    padding: 6px 6px 4px;
  }
  .gap {
    flex: 1;
  }
  .tools :global(.spin) {
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .path {
    margin: 0 8px 6px;
    font-size: 12px;
  }
  .error {
    margin: 0 8px 6px;
    padding: 6px 8px;
    border-radius: 3px;
    background: var(--danger-soft);
    color: var(--danger);
    font-size: 12px;
  }
  .list {
    flex: 1;
    overflow: auto;
    padding: 0 4px;
    outline: none;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 24px;
    padding: 0 6px;
    border-radius: 3px;
    cursor: default;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.sel {
    background: var(--selected);
  }
  .ic {
    display: inline-flex;
    color: var(--faint);
  }
  .ic.dir {
    color: var(--muted);
  }
  .nm {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sz {
    color: var(--faint);
    font-size: 11px;
    white-space: nowrap;
  }
  .empty {
    padding: 8px;
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-top: 1px solid var(--border);
    font-size: 11px;
    min-height: 30px;
  }
  .tname {
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  progress {
    flex: 1;
    accent-color: var(--accent);
  }
</style>
