<script lang="ts">
  // The Delinea Secret Server part of a credential source's form: the address, the sign-in method, the
  // user and the options of this kind (their meaning is listed at the top of src-tauri/src/sources/delinea/mod.rs).
  import type { SourceConfig } from "../../api";
  import SettingsRow from "../SettingsRow.svelte";

  let { source = $bindable() }: { source: SourceConfig } = $props();

  // Missing options get their defaults in place, so the object the form edits stays the same one.
  $effect.pre(() => {
    if (!source.options || typeof source.options !== "object") source.options = {};
    const o = source.options;
    if (o.auth !== "password" && o.auth !== "platform") o.auth = "password";
    if (typeof o.domain !== "string") o.domain = "";
    if (typeof o.otp !== "boolean") o.otp = false;
    if (typeof o.platformUrl !== "string") o.platformUrl = "";
    if (o.ticketSystem === undefined || o.ticketSystem === null) o.ticketSystem = "";
  });

  const platform = $derived(source.options?.auth === "platform");
</script>

<SettingsRow
  label="Secret Server address"
  help={platform
    ? "The address of your Secret Server Cloud, as the Delinea Platform shows it under Settings, Secret Server Connection."
    : "Secret Server on your own server, such as https://server/SecretServer, or Secret Server Cloud, such as https://tenant.secretservercloud.com."}
>
  <input bind:value={source.url} placeholder={platform ? "https://tenant.secretservercloud.com" : "https://server/SecretServer"} />
</SettingsRow>

<SettingsRow label="Sign in with" help="Which account Cablebag signs in with. Your password is asked for when signing in and never kept.">
  <select bind:value={source.options.auth}>
    <option value="password">A Secret Server user name and password</option>
    <option value="platform">A Delinea Platform service user</option>
  </select>
</SettingsRow>

{#if platform}
  <SettingsRow label="Delinea Platform address" help="Where the service user signs in, such as https://tenant.delinea.app.">
    <input bind:value={source.options.platformUrl} placeholder="https://tenant.delinea.app" />
  </SettingsRow>
  <SettingsRow label="Service user" help="The name of the platform service user that has access to the secrets in Secret Server.">
    <input bind:value={source.account} />
  </SettingsRow>
{:else}
  <SettingsRow label="User name" help="A Secret Server user, or your Active Directory user name.">
    <input bind:value={source.account} />
  </SettingsRow>
  <SettingsRow label="Domain" help="Only for an Active Directory account: its domain, such as CORP. Leave empty for a Secret Server user.">
    <input bind:value={source.options.domain} placeholder="Optional" />
  </SettingsRow>
  <SettingsRow label="Ask for a two-factor code" help="Asks for the code from your authenticator with the password every time. Without it, Cablebag asks for a code when Secret Server wants one.">
    <input type="checkbox" class="switch" bind:checked={source.options.otp} />
  </SettingsRow>
{/if}

<SettingsRow label="Ticket system number" help="Only when Secret Server checks ticket numbers against more than one ticket system: the number of the one to use. Leave empty otherwise.">
  <input bind:value={source.options.ticketSystem} inputmode="numeric" placeholder="Optional" />
</SettingsRow>
