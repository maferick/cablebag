<script lang="ts">
  // The Proton Pass part of a credential source's form: where pass-cli is (a setting of this PC only, never synced)
  // and which vaults to use (the options are listed at the top of src-tauri/src/sources/protonpass.rs).
  import type { SourceConfig } from "../../api";
  import { app } from "../../state.svelte";
  import SettingsRow from "../SettingsRow.svelte";

  let { source = $bindable() }: { source: SourceConfig } = $props();

  // Missing options get their defaults in place, so the object the form edits stays the same one. The path comes
  // from this PC's setting; saving the source keeps it there again (it never goes into the synced options).
  $effect.pre(() => {
    if (!source.options || typeof source.options !== "object") source.options = {};
    if (typeof source.options.cli !== "string") source.options.cli = (source.id && (app.settings[`protonpass.cli.${source.id}`] as string)) || "";
    if (typeof source.options.vaults !== "string") source.options.vaults = "";
  });
</script>

<SettingsRow
  label="Proton Pass command-line tool"
  help="Cablebag reads Proton Pass through Proton's own pass-cli, which keeps its own sign-in (through your web browser). Leave this empty when pass-cli is on the PATH; otherwise fill in the full path of pass-cli (pass-cli.exe on Windows). This PC only: it does not go into the sync. Proton offers pass-cli at proton.me/pass/download."
>
  <input bind:value={source.options.cli} placeholder="pass-cli" />
</SettingsRow>

<SettingsRow label="Vaults" help="The names of the vaults to use, separated by commas, such as Work. Empty uses every vault.">
  <input bind:value={source.options.vaults} placeholder="All vaults" />
</SettingsRow>
