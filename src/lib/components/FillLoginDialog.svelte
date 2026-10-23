<script lang="ts">
  // Choose any login from the credential sources to fill into a web tab's page (when none is saved for its site).
  import { onMount } from "svelte";
  import Modal from "./Modal.svelte";
  import VaultPicker from "./VaultPicker.svelte";
  import { app } from "../state.svelte";
  import { vault } from "../vault.svelte";

  let { host, onclose, onpick }: { host: string; onclose: () => void; onpick: (id: string, name: string) => void } = $props();
  let itemId = $state<string | undefined>(undefined);
  // A login the server found while searching is not among the loaded ones: its name is then not known here.
  const name = $derived(vault.item(itemId)?.name ?? "the login");

  // The page is a view of its own over the window: it steps aside while this dialog is open.
  onMount(() => {
    app.dialogs++;
    return () => app.dialogs--;
  });
</script>

<Modal title="Fill a login" width={520} {onclose}>
  <p class="muted small">Choose a login to fill into {host}. It is not saved for this site, so check that it is the right one.</p>
  <VaultPicker bind:itemId hint={host} />
  {#snippet footer()}
    <button class="btn" onclick={onclose}>Cancel</button>
    <button class="btn primary" disabled={!itemId} onclick={() => itemId && onpick(itemId, name)}>Fill into {host}</button>
  {/snippet}
</Modal>
