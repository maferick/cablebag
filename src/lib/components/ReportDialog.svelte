<script lang="ts">
  // Report a problem: a few questions, then GitHub's bug form opens in the web browser with the answers filled in.
  // Nothing is sent from here; the user signs in, reads it over and submits it on GitHub. The texts pass
  // Cablebag's own secret filter first, and the preview shows exactly what the address carries.
  import Modal from "./Modal.svelte";
  import { app } from "../state.svelte";
  import { api } from "../api";
  import { writeText } from "../clipboard";
  import { SESSION_TYPES, issueUrl, fullText, type Report } from "../report";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { ShieldAlert, ExternalLink, Copy } from "@lucide/svelte";

  let { start }: { start: { what: string } } = $props();

  /** The bug form's choice for the tab in front, so the usual case needs no choosing. */
  function sessionOfActiveTab(): string {
    const t = app.active;
    if (!t || t.type === "home" || t.type === "tool") return "";
    const byProto: Record<string, string> = {
      ssh: t.type === "files" ? "SFTP" : "SSH", sftp: "SFTP", ftp: "FTP", telnet: "Telnet", serial: "Serial",
      local: "Local shell/WSL", rdp: "RDP", vnc: "VNC", browser: "Web",
    };
    return byProto[t.proto] ?? "";
  }

  // svelte-ignore state_referenced_locally
  let form = $state({ title: "", session: sessionOfActiveTab(), what: start.what, expected: "", steps: "", extra: "" });
  let sys = $state<{ version: string; windows: string; webview: string; dotnet: string } | null>(null);
  api.systemInfo().then((i) => (sys = i)).catch(() => {});

  /** The report as it leaves this PC: the typed texts after the secret filter. */
  let clean = $state<Report | null>(null);
  let hidden = $state(0);
  /** The form as it was when `clean` was made: sending waits until the filter has seen the last change. */
  let cleanOf = $state("");
  let filterFailed = $state(false);

  const HIDDEN = /\[[a-z ]*hidden\]/g;
  const count = (s: string) => s.match(HIDDEN)?.length ?? 0;

  $effect(() => {
    const raw = { ...form };
    const s = sys;
    if (!s) return;
    let gone = false;
    // Typing should not ask the backend at every key; a moment after it stops is soon enough.
    const timer = setTimeout(async () => {
      const keys = ["title", "what", "expected", "steps", "extra"] as const;
      const out = await Promise.all(keys.map((k) => (raw[k].trim() ? api.redact(raw[k]).catch(() => null) : Promise.resolve(""))));
      if (gone) return;
      // Without the filter nothing goes out: a text it could not check stays empty and blocks sending.
      if (out.some((o) => o === null)) {
        clean = null;
        filterFailed = true;
        return;
      }
      const r: Report = {
        title: "", version: `Cablebag ${s.version}, WebView2 ${s.webview}, .NET 8 Desktop Runtime ${s.dotnet || "not installed"}`,
        windows: s.windows, session: raw.session, what: "", expected: "", steps: "", extra: "",
      };
      let n = 0;
      keys.forEach((k, i) => {
        r[k] = out[i] as string;
        n += Math.max(0, count(r[k]) - count(raw[k]));
      });
      clean = r;
      hidden = n;
      cleanOf = JSON.stringify(raw);
      filterFailed = false;
    }, 300);
    return () => {
      gone = true;
      clearTimeout(timer);
    };
  });

  const built = $derived(clean ? issueUrl(clean) : null);

  /** The form's own labels, so the preview reads like the form on GitHub. */
  const LABELS: Record<string, string> = {
    title: "Title", version: "Cablebag version", windows: "Windows version", session: "Session type",
    "what-happened": "What happened", expected: "What you expected", steps: "Steps to make it happen again", extra: "Anything else",
  };
  const preview = $derived.by(() => {
    if (!built) return [];
    const q = new URL(built.url).searchParams;
    return Object.keys(LABELS).filter((k) => q.get(k)).map((k) => ({ label: LABELS[k], value: q.get(k)! }));
  });

  const fresh = $derived(!!clean && cleanOf === JSON.stringify(form));
  const ready = $derived(!!built && fresh && !!form.what.trim());

  async function copyFull() {
    if (!clean) return;
    await writeText(fullText(clean));
    app.notify("The full text is copied. Paste it under Anything else.");
  }

  async function send() {
    if (!built || !ready) return;
    if (built.shortened) await copyFull();
    try {
      await openUrl(built.url);
      app.notify(built.shortened ? "The form is open in your web browser. Paste the full text under Anything else, then submit it there." : "The form is open in your web browser. Sign in, read it over and submit it there.");
      app.reporting = null;
    } catch (e) {
      app.notify(String(e), "error");
    }
  }
</script>

<Modal title="Report a problem" width={720} onclose={() => (app.reporting = null)}>
  <div class="privacy">
    <ShieldAlert size={16} />
    <p>
      This becomes a public page on GitHub. Leave out passwords, keys, tokens and host names or addresses you want to keep
      private; write <code>server1</code> or <code>10.0.0.x</code> instead. Cablebag hides the passwords it knows and
      anything that looks like a key, but it cannot recognise everything.
    </p>
  </div>

  <div class="grid">
    <label class="field wide"><span>In a few words (the title)</span><input bind:value={form.title} placeholder="Left empty, the first line of What happened" /></label>
    <label class="field">
      <span>Where it happens</span>
      <select bind:value={form.session}>
        <option value="">Choose...</option>
        {#each SESSION_TYPES as t (t)}<option value={t}>{t}</option>{/each}
      </select>
    </label>
    <label class="field wide"><span>What happened</span><textarea rows="4" bind:value={form.what} placeholder="What you saw, with the exact text of any message."></textarea></label>
    <label class="field wide"><span>What you expected</span><textarea rows="2" bind:value={form.expected}></textarea></label>
    <label class="field wide"><span>Steps to make it happen again</span><textarea rows="3" bind:value={form.steps} placeholder={"1. Open ...\n2. Click ...\n3. ..."}></textarea></label>
    <label class="field wide"><span>Anything else (optional)</span><textarea rows="2" bind:value={form.extra} placeholder="Did it work in an earlier version? Through a WireGuard network?"></textarea></label>
  </div>

  <p class="muted small">The Cablebag version and the Windows version are added by themselves.</p>

  {#if hidden}
    <p class="note">Cablebag hid {hidden === 1 ? "one thing" : `${hidden} things`} that looked like a password or key. The preview shows {hidden === 1 ? "it" : "them"} as hidden.</p>
  {/if}
  {#if built?.shortened}
    <div class="note long">
      <p>This report is too long to fill in all at once. The longest parts are shortened in the form; copy the full text and paste it under Anything else on GitHub.</p>
      <button class="btn" onclick={copyFull}><Copy size={14} /> Copy the full text</button>
    </div>
  {/if}

  <h3>What GitHub gets</h3>
  <div class="preview" aria-label="Preview of the report">
    {#if filterFailed}
      <span class="error">Cablebag could not check the text for passwords, so it cannot be sent. Close this window and try again.</span>
    {:else if !fresh}
      <span class="muted">Getting it ready...</span>
    {:else}
      {#each preview as p (p.label)}
        <div class="pv-label">{p.label}</div>
        <div class="pv-value">{p.value}</div>
      {/each}
    {/if}
  </div>

  {#snippet footer()}
    <button class="btn" onclick={() => (app.reporting = null)}>Cancel</button>
    <button class="btn primary" disabled={!ready} title={form.what.trim() ? "Opens GitHub's form in your web browser; you submit it there" : "Write what happened first"} onclick={send}>
      <ExternalLink size={14} /> {built?.shortened ? "Copy the full text and open GitHub" : "Send to GitHub"}
    </button>
  {/snippet}
</Modal>

<style>
  .privacy {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 8px 12px;
    margin-bottom: 12px;
    border-left: 3px solid var(--warn);
    background: var(--warn-soft);
    color: var(--text);
    border-radius: 3px;
  }
  .privacy :global(svg) {
    flex-shrink: 0;
    margin-top: 2px;
    color: var(--warn-text);
  }
  .privacy p {
    margin: 0;
    line-height: 1.45;
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 200px;
    gap: 10px 12px;
  }
  .grid .wide:not(:first-child) {
    grid-column: 1 / -1;
  }
  textarea {
    resize: vertical;
  }
  .small {
    font-size: 12px;
    margin: 8px 0 0;
  }
  .note {
    margin: 8px 0 0;
    padding: 6px 10px;
    border-radius: 3px;
    background: var(--panel);
    border: 1px solid var(--border);
  }
  .note.long {
    display: flex;
    align-items: center;
    gap: 12px;
    border-color: var(--warn);
  }
  .note.long p {
    margin: 0;
    flex: 1;
  }
  .note.long .btn {
    flex-shrink: 0;
  }
  h3 {
    margin: 14px 0 6px;
    font-size: 13px;
    font-weight: 600;
  }
  .preview {
    max-height: 220px;
    overflow: auto;
    padding: 8px 12px;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--panel);
    font-size: 12px;
    user-select: text;
  }
  .error {
    color: var(--danger-text);
  }
  .pv-label {
    color: var(--muted);
    margin-top: 6px;
  }
  .pv-label:first-child {
    margin-top: 0;
  }
  .pv-value {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
