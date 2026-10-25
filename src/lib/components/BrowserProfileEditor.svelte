<script lang="ts">
  // A browser profile's name and colour; new or existing.
  import Modal from "./Modal.svelte";
  import { app } from "../state.svelte";
  import { browserProfiles, type BrowserProfile } from "../browserProfiles.svelte";
  import { TAB_COLORS } from "../tabColors";

  let { profile, onclose, onsaved }: { profile: BrowserProfile | null; onclose: () => void; onsaved?: (id: string) => void } = $props();
  // svelte-ignore state_referenced_locally
  let editing = $state<BrowserProfile>(profile ? { ...profile } : { id: "", name: "", color: browserProfiles.freeColor() });

  async function save() {
    if (!editing.name.trim()) return app.notify("Give the profile a name, such as the customer or tenant.", "error");
    const id = await browserProfiles.save(editing);
    app.notify(`Browser profile ${editing.name.trim()} saved`);
    onsaved?.(id);
    onclose();
  }
</script>

<Modal title={profile ? `Browser profile ${profile.name}` : "New browser profile"} width={480} {onclose}>
  <div class="form">
    <p class="muted small">
      A browser profile keeps its own cookies and logins. Web sessions with different profiles can be signed in to
      different accounts at the same time, such as two Azure tenants or two Teams.
    </p>
    <label class="field"><span>Name</span><input bind:value={editing.name} placeholder="For example: Customer A tenant" /></label>
    <label class="field"><span>Colour of its tabs</span>
      <select bind:value={editing.color}>
        {#each TAB_COLORS as c}<option value={c.value}>{c.name}</option>{/each}
      </select>
    </label>
    <label class="check"><input type="checkbox" checked={!editing.muted} onchange={(e) => (editing.muted = !e.currentTarget.checked)} /> Show notifications from its web tabs (such as Teams messages)</label>
  </div>
  {#snippet footer()}
    <button class="btn" onclick={onclose}>Cancel</button>
    <button class="btn primary" onclick={save}>Save</button>
  {/snippet}
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
</style>
