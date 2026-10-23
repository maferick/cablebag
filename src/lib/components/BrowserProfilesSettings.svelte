<script lang="ts">
  // Settings, Web tabs: how web tabs behave, and the browser profiles web sessions can use (with how many sessions
  // and folders use each).
  import { app } from "../state.svelte";
  import { ask } from "../ask.svelte";
  import { browserProfiles, type BrowserProfile } from "../browserProfiles.svelte";
  import BrowserProfileEditor from "./BrowserProfileEditor.svelte";
  import SettingsSection from "./SettingsSection.svelte";
  import SettingsRow from "./SettingsRow.svelte";
  import SettingsItem from "./SettingsItem.svelte";
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

<SettingsSection title="Behaviour" find="notifications do not disturb sleep memory">
  <SettingsRow label="Notifications from web tabs" help="As Windows notifications; switch off for do not disturb.">
    <input type="checkbox" class="switch" checked={app.settings["browser.notify"] !== false} onchange={(e) => app.setSetting("browser.notify", e.currentTarget.checked)} />
  </SettingsRow>
  <SettingsRow label="Tabs not looked at go to sleep" info="To save memory. A sleeping tab opens again where it was when you show it. Sessions set to keep running and private tabs never sleep.">
    <select value={String(app.settings["browser.sleepMinutes"] ?? 0)} onchange={(e) => app.setSetting("browser.sleepMinutes", Number(e.currentTarget.value))}>
      <option value="0">Never</option>
      <option value="15">After 15 minutes</option>
      <option value="30">After 30 minutes</option>
      <option value="60">After an hour</option>
    </select>
  </SettingsRow>
</SettingsSection>

<SettingsSection title={`Browser profiles (${browserProfiles.list.length})`} desc="Each keeps its own cookies and logins; choose one per web session or per folder." find="profile cookies tenant private sign out">
  {#each browserProfiles.list as p (p.id)}
    <SettingsItem name={p.name} detail={`${used(p.id)}${p.muted ? "; notifications off" : ""}`}>
      {#snippet mark()}<span class="dot" style:background={p.color || "var(--faint)"} aria-hidden="true"></span>{/snippet}
      {#snippet actions()}
        <button class="btn small" onclick={() => (editing = p)}><Pencil size={13} /> Edit</button>
        <button class="btn small" title="Remove its cookies and logins" onclick={() => signOut(p)}><LogOut size={13} /> Sign out everywhere</button>
        <button class="icon-btn" title="Delete {p.name}" aria-label="Delete {p.name}" onclick={() => remove(p)}><Trash2 size={14} /></button>
      {/snippet}
    </SettingsItem>
  {:else}
    <div class="empty">
      <Globe size={16} />
      <p>
        No browser profiles yet. Web sessions all share Cablebag's own cookies and logins, so you are signed in to one
        account per site. Make a profile for a second tenant, customer or Teams.
      </p>
    </div>
  {/each}
  <div class="add"><button class="btn" onclick={() => (editing = null)}><Plus size={14} /> New browser profile</button></div>
</SettingsSection>

{#if editing !== undefined}
  <BrowserProfileEditor profile={editing} onclose={() => (editing = undefined)} />
{/if}

<style>
  .dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
  }
  .empty {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--border);
    color: var(--muted);
  }
  .empty p {
    margin: 0;
    font-size: 12px;
  }
  .add {
    padding: 10px 14px;
  }
</style>
