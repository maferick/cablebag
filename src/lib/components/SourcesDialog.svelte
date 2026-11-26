<script lang="ts">
  // Credential sources in a dialog of its own, on top of whatever is open (such as a half-filled new session), so
  // setting one up never loses that. The same page as Settings, Credential sources.
  import Modal from "./Modal.svelte";
  import CredentialSources from "./CredentialSources.svelte";
  import { app } from "../state.svelte";

  const close = () => (app.sourcesOpen = false);
</script>

<Modal title="Credential sources" width={760} onclose={close}>
  <div class="sources">
    <CredentialSources />
  </div>
  {#snippet footer()}
    <button class="btn primary" onclick={close}>Done</button>
  {/snippet}
</Modal>

<style>
  .sources {
    max-height: 70vh;
    overflow-y: auto;
  }
</style>
