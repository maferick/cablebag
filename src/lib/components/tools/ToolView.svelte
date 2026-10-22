<script lang="ts">
  import type { Tab } from "../../state.svelte";
  import TunnelsView from "./TunnelsView.svelte";
  import NetScanView from "./NetScanView.svelte";
  import PortScanView from "./PortScanView.svelte";
  import OpenPortsView from "./OpenPortsView.svelte";
  import WakeView from "./WakeView.svelte";
  import KeyGenView from "./KeyGenView.svelte";
  import SettingsDialog from "../SettingsDialog.svelte";

  let { tab, visible }: { tab: Tab; visible: boolean } = $props();
</script>

<div class="wrap" style:display={visible ? "block" : "none"}>
  {#if tab.tool === "tunnels"}
    <TunnelsView />
  {:else if tab.tool === "netscan"}
    <NetScanView />
  {:else if tab.tool === "portscan"}
    <PortScanView />
  {:else if tab.tool === "openports"}
    <OpenPortsView />
  {:else if tab.tool === "wol"}
    <WakeView />
  {:else if tab.tool === "keygen"}
    <KeyGenView />
  {:else if tab.tool === "settings"}
    <SettingsDialog />
  {/if}
</div>

<style>
  .wrap {
    position: absolute;
    inset: 0;
  }
</style>
