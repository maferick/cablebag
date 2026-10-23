<script lang="ts">
  // Settings, Browser profiles: the profiles web sessions can use, with how many sessions and folders use each.
  import { app } from "../state.svelte";
  import { ask } from "../ask.svelte";
  import { browserProfiles, type BrowserProfile } from "../browserProfiles.svelte";
  import BrowserProfileEditor from "./BrowserProfileEditor.svelte";
  import { Plus, Pencil, Trash2, LogOut, Globe } from "@lucide/svelte";

  /** The profile in the editor: undefined = closed, null = a new one. */
  let editing = $state<BrowserProfile | null | undefined>(undefined);

  function used(id: string): string {
    const sessions = app.sessions.filter((s) => s.data?.browser_profile === id).length;
    const folders = app.folders.filter((f) => f.data?.browser_profile === id).length;
    const parts = [sessions && `${sessions} session${sessions === 1 ? "" : "s"}`, folders && `${folders} folder${folders === 1 ? "" : "s"}`].filter(Boolean);
    return parts.length ? `Used by ${parts.join(" and ")}` : "Not used by any session or folder yet";
  }

  async function signOut(p: BrowserProfile) {
    if (!(await ask.confirm("Sign out everywhere", `Remove the cookies and logins of ${p.name}? Its open tabs close, and its web sessions ask you to sign in again.`, { okLabel: "Sign out" })))
      return;
    await browserProfiles.clear(p.id);
    app.notify(`Signed out of everything in ${p.name}`);
  }

  async function remove(p: BrowserProfile) {
    if (!(await ask.confirm("Delete browser profile", `Delete ${p.name} with its cookies and logins? Its open tabs close, and sessions and folders that use it go back to Cablebag's shared profile.`, { okLabel: "Delete", danger: true })))
      return;
    await browserProfiles.remove(p.id);
    app.notify(`Browser profile ${p.name} deleted`);
  }
</script>

<label class="check notify">
  <input type="checkbox" checked={app.settings["browser.notify"] !== false} onchange={(e) => app.setSetting("browser.notify", e.currentTarget.checked)} />
  Show notifications from web tabs as Windows notifications (switch off for do not disturb)
</label>
<label class="field sleep"><span>Web tabs not looked at for a while go to sleep (to save memory)</span>
  <select value={String(app.settings["browser.sleepMinutes"] ?? 0)} onchange={(e) => app.setSetting("browser.sleepMinutes", Number(e.currentTarget.value))}>
    <option value="0">Never</option>
    <option value="15">After 15 minutes</option>
    <option value="30">After 30 minutes</option>
    <option value="60">After an hour</option>
  </select>
  <span class="muted small">A sleeping tab opens again where it was when you show it. Sessions set to keep running and private tabs never sleep.</span>
</label>
<div class="list">
  {#each browserProfiles.list as p (p.id)}
    <div class="item">
      <span class="dot" style:background={p.color || "var(--faint)"} aria-hidden="true"></span>
      <div class="grow">
        <div class="name">{p.name}</div>
        <div class="sub">{used(p.id)}{p.muted ? "; notifications off" : ""}</div>
      </div>
      <button class="btn small" onclick={() => (editing = p)}><Pencil size={13} /> Edit</button>
      <button class="btn small" title="Remove its cookies and logins" onclick={() => signOut(p)}><LogOut size={13} /> Sign out everywhere</button>
      <button class="icon-btn" title="Delete {p.name}" aria-label="Delete {p.name}" onclick={() => remove(p)}><Trash2 size={14} /></button>
    </div>
  {:else}
    <div class="item empty">
      <Globe size={16} />
      <p class="sub">
        No browser profiles yet. Web sessions all share Cablebag's own cookies and logins, so you are signed in to one
        account per site. Make a profile for a second tenant, customer or Teams.
      </p>
    </div>
  {/each}
  <div><button class="btn" onclick={() => (editing = null)}><Plus size={14} /> New browser profile</button></div>
</div>

{#if editing !== undefined}
  <BrowserProfileEditor profile={editing} onclose={() => (editing = undefined)} />
{/if}

<style>
  .notify,
  .sleep {
    margin-bottom: 12px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    border: 1px solid var(--border);
    border-radius: 3px;
  }
  .item.empty :global(svg) {
    color: var(--muted);
    flex-shrink: 0;
  }
  .dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .name {
    font-weight: 600;
  }
  .sub {
    margin: 0;
    font-size: 12px;
    color: var(--muted);
  }
</style>
