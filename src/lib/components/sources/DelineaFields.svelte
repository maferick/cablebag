<script lang="ts">
  // The Delinea part of a credential source's form: the address, the account and the options of this kind.
  // shortcut: a stub until the Delinea client is written.
  import type { SourceConfig } from "../../api";
  import SettingsRow from "../SettingsRow.svelte";

  let { source = $bindable() }: { source: SourceConfig } = $props();
</script>

<SettingsRow label="Server">
  <input bind:value={source.url} />
</SettingsRow>
