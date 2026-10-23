<script lang="ts">
  // The CyberArk part of a credential source's form: the address, the sign-in method, the user and the
  // options of this kind (their meaning is listed at the top of src-tauri/src/sources/cyberark/mod.rs).
  import type { SourceConfig } from "../../api";
  import SettingsRow from "../SettingsRow.svelte";

  let { source = $bindable() }: { source: SourceConfig } = $props();

  const METHODS = ["cyberark", "ldap", "radius", "windows", "saml", "identity"];

  /** Privilege Cloud on Shared Services signs in through CyberArk Identity; the rest with a password. */
  function defaultAuth(url: string): string {
    return /\.privilegecloud\.cyberark\.cloud|\.cyberark\.cloud\/privilegecloud/i.test(url || "") ? "identity" : "cyberark";
  }

  // Missing options get their defaults in place, so the object the form edits stays the same one.
  $effect.pre(() => {
    if (!source.options || typeof source.options !== "object") source.options = {};
    const o = source.options;
    if (!METHODS.includes(o.auth)) o.auth = defaultAuth(source.url);
    for (const k of ["sso_url", "identity_url", "ticketing_system", "psmp_host", "psmp_user", "psm_host", "psm_user"]) {
      if (typeof o[k] !== "string") o[k] = "";
    }
    for (const k of ["psmp_port", "psm_port"]) {
      if (typeof o[k] !== "string" && typeof o[k] !== "number") o[k] = "";
    }
    if (typeof o.psm_component !== "string" || !o.psm_component) o.psm_component = "PSM-RDP";
    if (!["auto", "connect", "client"].includes(o.psm_rdp_mode)) o.psm_rdp_mode = "auto";
    if (typeof o.ask_reason !== "boolean") o.ask_reason = false;
    if (typeof o.psmp_key !== "boolean") o.psmp_key = true;
  });

  const auth = $derived(source.options?.auth ?? "cyberark");
  const sso = $derived(auth === "saml" || auth === "identity");
</script>

<SettingsRow
  label="Address"
  help="Privilege Cloud, such as https://company.privilegecloud.cyberark.com or https://company.privilegecloud.cyberark.cloud, or your own PVWA, such as https://pvwa.example.com."
>
  <input bind:value={source.url} placeholder="https://company.privilegecloud.cyberark.com" />
</SettingsRow>

<SettingsRow
  label="Sign in with"
  help={sso
    ? "You sign in yourself in a Cablebag window, on your organisation's own sign-in page. Cablebag keeps the sign-in in memory only."
    : "Your password is asked for when signing in and never kept."}
>
  <select bind:value={source.options.auth}>
    <option value="cyberark">CyberArk password</option>
    <option value="ldap">LDAP (your directory account)</option>
    <option value="radius">RADIUS</option>
    <option value="windows">Windows</option>
    <option value="saml">Single sign-on (SAML)</option>
    <option value="identity">CyberArk Identity (Privilege Cloud on Shared Services)</option>
  </select>
</SettingsRow>

{#if !sso}
  <SettingsRow label="User name" help="Your CyberArk, LDAP, RADIUS or Windows user name. Leave it empty to type it each time.">
    <input bind:value={source.account} />
  </SettingsRow>
{/if}

{#if auth === "identity"}
  <SettingsRow
    label="CyberArk Identity address"
    help="Your Identity tenant, such as https://abc1234.id.cyberark.cloud. Leave it empty to start at the Privilege Cloud portal."
  >
    <input bind:value={source.options.identity_url} placeholder="https://abc1234.id.cyberark.cloud" />
  </SettingsRow>
{/if}

{#if sso}
  <SettingsRow
    label="Sign-in page"
    help={auth === "saml"
      ? "Optional: where the sign-in window starts, such as your identity provider's link to CyberArk. Empty: the PVWA's own sign-in page, where you choose single sign-on."
      : "Optional: where the sign-in window starts, instead of the Identity address."}
  >
    <input bind:value={source.options.sso_url} placeholder="Optional" />
  </SettingsRow>
  <SettingsRow label="User name" help="Optional: your CyberArk user name, shown while signed in and used for PSM when CyberArk does not say who you are.">
    <input bind:value={source.account} placeholder="Optional" />
  </SettingsRow>
{/if}

<SettingsRow label="Ticketing system" help="Optional: the name of the ticketing system to fill in when CyberArk asks for a ticket.">
  <input bind:value={source.options.ticketing_system} placeholder="Optional" />
</SettingsRow>

<SettingsRow label="Always ask for a reason" help="Asks why you need an account every time Cablebag fetches a password. Without it, Cablebag asks when CyberArk wants a reason.">
  <input type="checkbox" class="switch" bind:checked={source.options.ask_reason} />
</SettingsRow>

<div class="sub">Connecting through PSM</div>

<SettingsRow label="PSM for SSH server" help="The address of PSM for SSH, for SSH sessions that go through CyberArk.">
  <input bind:value={source.options.psmp_host} placeholder="psmp.example.com" />
</SettingsRow>
<SettingsRow label="PSM for SSH port">
  <input bind:value={source.options.psmp_port} inputmode="numeric" placeholder="22" />
</SettingsRow>
<SettingsRow label="Your user name for PSM for SSH" help="Optional: when it differs from the one you sign in with. On Privilege Cloud, your full login name, such as name@company.com.">
  <input bind:value={source.options.psmp_user} placeholder="Optional" />
</SettingsRow>
<SettingsRow label="Sign in to PSM for SSH with a key" help="After you sign in, Cablebag asks CyberArk for a short-lived SSH key, so PSM for SSH does not ask for your password. Needed with single sign-on.">
  <input type="checkbox" class="switch" bind:checked={source.options.psmp_key} />
</SettingsRow>

<SettingsRow label="Remote Desktop through PSM" help="Through CyberArk: CyberArk hands out the connection for the account. With an RDP client: Cablebag connects to the PSM server and PSM asks for your CyberArk sign-in.">
  <select bind:value={source.options.psm_rdp_mode}>
    <option value="auto">Choose for me</option>
    <option value="connect">Through CyberArk</option>
    <option value="client">With an RDP client</option>
  </select>
</SettingsRow>
<SettingsRow label="PSM server" help="The PSM server or its load balancer, for connecting with an RDP client.">
  <input bind:value={source.options.psm_host} placeholder="psm.example.com" />
</SettingsRow>
<SettingsRow label="PSM server port">
  <input bind:value={source.options.psm_port} inputmode="numeric" placeholder="3389" />
</SettingsRow>
<SettingsRow label="Connection component" help="The PSM connection component for Remote Desktop.">
  <input bind:value={source.options.psm_component} placeholder="PSM-RDP" />
</SettingsRow>
<SettingsRow label="Your user name for PSM" help="Optional: the CyberArk user name the PSM server gets, when it differs from the one you sign in with.">
  <input bind:value={source.options.psm_user} placeholder="Optional" />
</SettingsRow>

<style>
  .sub {
    padding: 14px 14px 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
    border-bottom: 1px solid var(--border);
  }
</style>
