<script lang="ts">
  // Moving a stack to another Docker server. First a check: where every folder it uses is on the other server
  // (through the NAS both mount, whatever the mount's name or depth; on the NAS itself the folder is local),
  // what is only on this server, ports, names, networks, images and logins there. Then the move: stop here
  // (never both at once on the same data), start there, wait until it is healthy, and go back when it is not.
  import { onMount } from "svelte";
  import Modal from "./Modal.svelte";
  import { api, type DockerContainer, type DockerListing, type DockerTarget, type DockerTranslation } from "../api";
  import { app } from "../state.svelte";
  import { ask } from "../ask.svelte";
  import { moves, portainerFile } from "../docker.svelte";
  import { CircleCheck, CircleAlert, TriangleAlert, Info, ArrowRight, RefreshCw } from "@lucide/svelte";

  let {
    tab,
    listing,
    project,
    items,
    onclose,
    ondone,
  }: { tab: string; listing: DockerListing; project: string; items: DockerContainer[]; onclose: () => void; ondone: () => void } = $props();

  // svelte-ignore state_referenced_locally
  const source = app.tabs.find((t) => t.id === tab)!;
  const serverName = (t: { session?: { name: string; host: string } }) => t.session?.name || t.session?.host || "";
  // svelte-ignore state_referenced_locally
  const sourceSudo = listing.access === "sudo";
  // svelte-ignore state_referenced_locally
  const first = items[0];
  const portainer = first.portainer;

  /** The other Containers tabs that are connected: the servers a stack can move to. */
  const targets = $derived(app.tabs.filter((t) => t.type === "docker" && t.id !== tab && t.status === "connected"));
  const servers = $derived(app.sessions.filter((s) => s.kind === "ssh" && !app.tabs.some((t) => t.type === "docker" && t.session?.id === s.id)));
  let targetId = $state("");
  let openOther = $state("");
  // svelte-ignore state_referenced_locally
  let name = $state(project.toLowerCase().replace(/[^a-z0-9_-]+/g, "-"));
  let pin = $state(true);

  interface Row {
    level: "ok" | "warn" | "bad" | "info";
    text: string;
    detail?: string;
  }
  let checking = $state(false);
  let rows = $state<Row[] | null>(null);
  let file = $state("");
  let env = $state("");
  let error = $state("");
  let phase = $state<"" | "stopping" | "starting" | "waiting" | "rolling back">("");
  let anyway = $state(false);
  let targetInfo: { listing: DockerListing; facts: DockerTarget } | null = null;

  const bad = $derived((rows ?? []).filter((r) => r.level === "bad").length);

  onMount(() => {
    if (targets.length === 1) targetId = targets[0].id;
  });

  /** Opens the other server's Containers in the background, so its connection is there for the move. */
  function open() {
    const s = app.sessions.find((x) => x.id === openOther);
    if (!s) return;
    const here = app.activeId;
    app.openContainers(s);
    app.activeId = here;
    openOther = "";
  }

  const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

  async function check() {
    error = "";
    rows = null;
    anyway = false;
    const target = app.tabs.find((t) => t.id === targetId);
    if (!target) return (error = "Choose the server to move it to.");
    checking = true;
    try {
      const out: Row[] = [];
      const tListing = await api.dockerList(target.id);
      if (tListing.access !== "direct" && tListing.access !== "sudo") throw new Error(`${serverName(target)} cannot use Docker with this login (open its Containers tab to see why).`);
      if (tListing.compose === "none") throw new Error(`${serverName(target)} has no Docker Compose.`);
      const images = [...new Set(items.map((c) => c.image))];
      const [fromM, toM, facts, here, digests] = await Promise.all([
        api.dockerMounts(tab),
        api.dockerMounts(target.id),
        api.dockerTarget(target.id, tListing.access === "sudo", images, name),
        api.dockerTarget(tab, sourceSudo, [], "-"),
        pin ? api.dockerDigests(tab, sourceSudo, images) : Promise.resolve({} as Record<string, string>),
      ]);
      targetInfo = { listing: tListing, facts };

      // The compose file (and .env) as it is now.
      let text = "";
      let envText = "";
      if (portainer) {
        text = await portainerFile(tab, listing, sourceSudo, first.config_files[0], [...new Set(items.map((c) => c.service))]);
        out.push({ level: "warn", text: "Made in Portainer: variables set in Portainer are not in the file.", detail: "Add them under .env below. Remove the stack in Portainer after the move, not here." });
      } else {
        text = (await api.readText(tab, first.config_files[0])).text;
        envText = await api.readText(tab, `${first.working_dir}/.env`).then((r) => r.text, () => "");
      }
      if (first.config_files.length > 1) out.push({ level: "warn", text: `It uses ${first.config_files.length} compose files; only ${first.config_files[0].split("/").pop()} goes along.` });

      // Folders through the shared storage.
      const binds = [...new Set(items.flatMap((c) => c.mounts.filter((m) => m.kind === "bind").map((m) => m.source)))];
      const tr: DockerTranslation[] = binds.length ? await api.dockerTranslate(binds, fromM, toM) : [];
      for (const t of [...tr].sort((a, b) => b.path.length - a.path.length)) {
        if (t.kind === "nas" && t.to) {
          out.push({ level: "ok", text: t.path === t.to ? `${t.path} is the same there` : `${t.path}  ->  ${t.to}`, detail: `On the NAS: ${t.nas}` });
          if (t.to !== t.path) {
            // Also inside ${VAR:-/path} defaults.
            const re = new RegExp(escape(t.path) + "(?=[/:\"'}\\s]|$)", "gm");
            text = text.replace(re, t.to);
            envText = envText.replace(re, t.to);
          }
        } else if (t.kind === "nas") {
          out.push({ level: "bad", text: `${t.path}: ${serverName(target)} has no mount of ${t.nas}`, detail: "Mount that NAS folder there first (and check the NAS lets that server in)." });
        } else if (t.kind === "host") {
          out.push({ level: "info", text: `${t.path}: a file of the server itself; the same path there` });
        } else {
          out.push({ level: "warn", text: `${t.path} is only on this server`, detail: "Copy it to the other server first (to the same path), or the stack starts without it." });
        }
      }
      // Paths written differently in the file (./data, ${VAR}) cannot be changed here.
      for (const t of tr) if (t.kind === "nas" && t.to && t.to !== t.path && !text.includes(t.to) && !envText.includes(t.to)) out.push({ level: "warn", text: `${t.path} is written differently in the compose file`, detail: `Change it by hand to ${t.to}.` });
      if (/^\s*-\s*\.\.?\//m.test(text)) out.push({ level: "warn", text: "The compose file uses folders next to it (./...)", detail: "Their contents stay here; copy them along to ~/stacks/" + name + " on the other server." });

      for (const v of new Set(items.flatMap((c) => c.mounts.filter((m) => m.kind === "volume").map((m) => m.source)))) {
        out.push({ level: "warn", text: `Docker volume ${v} stays on this server`, detail: "Its data does not move; the stack starts with an empty volume there." });
      }

      // Ports, names, networks, host things.
      const ports = [...new Set(items.flatMap((c) => c.ports.filter((p) => p.public).map((p) => p.public!)))];
      const used = ports.filter((p) => facts.ports.includes(p));
      if (used.length) out.push({ level: "bad", text: `Port${used.length > 1 ? "s" : ""} ${used.join(", ")} already in use on ${serverName(target)}` });
      else if (ports.length) out.push({ level: "ok", text: ports.length === 1 ? `Port ${ports[0]} is free there` : `Ports ${ports.join(", ")} are free there` });
      // A name Compose made (project-service-1) follows the stack's new name; only a fixed container_name stays.
      for (const c of items) {
        const made = new RegExp(`^${escape(project)}[-_]${escape(c.service)}[-_]\\d+$`).exec(c.name);
        const there = made ? c.name.replace(project, name) : c.name;
        if (facts.names.includes(there)) out.push({ level: "bad", text: `A container named ${there} already exists there` });
      }
      const own = new Set(["bridge", "host", "none", `${project}_default`]);
      for (const n of new Set(items.flatMap((c) => c.networks))) {
        if (own.has(n) || n.startsWith(`${project}_`)) continue;
        if (!facts.networks.includes(n)) out.push({ level: "bad", text: `Network ${n} does not exist there`, detail: "Create it there first (docker network create " + n + "), or remove it from the compose file." });
      }
      for (const c of items) {
        if (c.network_mode === "host") out.push({ level: "warn", text: `${c.name} uses the server's own network`, detail: "Addresses and firewall rules of the other server apply." });
        if (c.privileged) out.push({ level: "warn", text: `${c.name} runs privileged` });
        for (const d of c.devices) out.push({ level: "warn", text: `${c.name} uses ${d} of this server`, detail: "Check that the other server has it." });
      }
      if (facts.stack_exists) out.push({ level: "bad", text: `~/stacks/${name} already exists on ${serverName(target)}`, detail: "Pick another name below." });
      if (here.arch && facts.arch && here.arch !== facts.arch) out.push({ level: "bad", text: `Different processor: ${here.arch} here, ${facts.arch} there`, detail: "The images must exist for both." });

      // Images: the version running now, private registries.
      const warned = new Set<string>();
      for (const img of images) {
        const registry = img.includes("/") && /[.:]/.test(img.split("/")[0]) ? img.split("/")[0] : "";
        if (registry && !warned.has(registry) && !facts.logins.includes(registry) && !facts.images.includes(img)) {
          warned.add(registry);
          out.push({ level: "bad", text: `${serverName(target)} is not logged in to ${registry}`, detail: `Run docker login ${registry} there first.` });
        }
        const digest = digests[img];
        if (pin && img.includes("@")) {
          out.push({ level: "ok", text: `${img.split("@")[0]} is already pinned to one version` });
        } else if (pin && digest) {
          const re = new RegExp(`^(\\s*image:\\s*["']?)${escape(img)}(["']?\\s*)$`, "m");
          if (re.test(text)) {
            text = text.replace(re, `$1${img}@${digest}$2`);
            out.push({ level: "ok", text: `${img} pinned to the version running now` });
          }
        }
      }
      if (!out.some((r) => r.level === "bad" || r.level === "warn")) out.unshift({ level: "ok", text: "Nothing stands in the way." });
      rows = out;
      file = text;
      env = envText;
    } catch (e) {
      error = String(e);
    } finally {
      checking = false;
    }
  }

  async function healthy(targetTab: string): Promise<boolean> {
    // Two looks in a row where every container of the stack runs and none is unhealthy or restarting.
    let good = 0;
    for (let i = 0; i < 12; i++) {
      await new Promise((r) => setTimeout(r, 5000));
      const l = await api.dockerList(targetTab).catch(() => null);
      const mine = l?.containers.filter((c) => c.project === name) ?? [];
      const ok = mine.length > 0 && mine.every((c) => c.state === "running" && c.health !== "unhealthy" && c.health !== "starting");
      good = ok ? good + 1 : 0;
      if (good >= 2) return true;
    }
    return false;
  }

  async function startHere() {
    if (portainer) for (const c of items) await api.dockerContainer(tab, sourceSudo, c.id, "start").catch(() => {});
    else await api.dockerCompose(tab, listing.compose, sourceSudo, project, first.working_dir, first.config_files, "start").catch(() => {});
  }

  async function move() {
    const target = app.tabs.find((t) => t.id === targetId);
    if (!target || !targetInfo) return;
    const to = serverName(target);
    const from = serverName(source);
    if (!(await ask.confirm("Move the stack", `Stop ${project} on ${from} and start it on ${to}? If it does not come up healthy there, Cablebag stops it there and starts it here again.`, { okLabel: "Move", danger: true }))) return;
    error = "";
    const tSudo = targetInfo.listing.access === "sudo";
    let dir = "";
    try {
      phase = "stopping";
      // Never both at once on the same data: stop here first (stopped, not removed, to come back to).
      if (portainer) for (const c of items) await api.dockerContainer(tab, sourceSudo, c.id, "stop");
      else {
        const r = await api.dockerCompose(tab, listing.compose, sourceSudo, project, first.working_dir, first.config_files, "stop");
        if (r.exit !== 0) throw new Error(`Could not stop it here: ${r.output}`);
      }
      phase = "starting";
      const made = await api.dockerCreateStack(target.id, targetInfo.listing.compose, tSudo, name, file, env);
      dir = made.dir;
      phase = "waiting";
      if (made.exit !== 0 || !(await healthy(target.id))) {
        phase = "rolling back";
        await api.dockerCompose(target.id, targetInfo.listing.compose, tSudo, name, dir, [`${dir}/compose.yml`], "down").catch(() => {});
        await startHere();
        await moves.add({ at: Date.now(), project, from, to, result: "rolled back", note: made.output.slice(-500) });
        error = `It did not come up healthy on ${to}, so it was stopped there and started here again. Its files stay in ${dir} on ${to}.\n\n${made.output.trim()}`;
        return;
      }
      await moves.add({ at: Date.now(), project, from, to, result: "moved", note: `Files in ${dir}` });
      app.notify(`${project} runs on ${to} now. It is stopped (not removed) on ${from}.`);
      ondone();
    } catch (e) {
      // Something broke halfway: whatever started there stays, this one starts again here.
      if (phase !== "stopping") await startHere();
      await moves.add({ at: Date.now(), project, from, to, result: "failed", note: String(e) });
      error = `${e}\n\nIt was started here again.`;
    } finally {
      phase = "";
    }
  }

  const icon = { ok: CircleCheck, warn: TriangleAlert, bad: CircleAlert, info: Info };
  const recent = $derived(moves.list.filter((m) => m.project === project).slice(0, 3));
</script>

<Modal title={`Move ${project} to another server`} width={920} {onclose}>
  <div class="top">
    <label class="field grow"><span>To</span>
      <select bind:value={targetId} onchange={() => (rows = null)}>
        <option value="">Choose a server...</option>
        {#each targets as t (t.id)}<option value={t.id}>{serverName(t)}</option>{/each}
      </select>
    </label>
    <label class="field"><span>Stack name there</span><input bind:value={name} onchange={() => (rows = null)} spellcheck="false" /></label>
    <button class="btn" disabled={!targetId || checking} onclick={check}><RefreshCw size={14} class={checking ? "spin" : ""} /> {rows ? "Check again" : "Check the move"}</button>
  </div>
  {#if !targets.length}
    <p class="muted small">The other server's Containers must be open (Cablebag uses that connection). Open one here:</p>
  {/if}
  <div class="top">
    <label class="field grow"><span>{targets.length ? "Or open another server" : "Server"}</span>
      <select bind:value={openOther} onchange={open}>
        <option value="">Choose an SSH session...</option>
        {#each servers as s (s.id)}<option value={s.id}>{s.name || s.host}</option>{/each}
      </select>
    </label>
    <label class="check pin"><input type="checkbox" bind:checked={pin} onchange={() => (rows = null)} /> Pin the images to the versions running now</label>
  </div>

  {#if rows}
    <ul class="report">
      {#each rows as r}
        {@const I = icon[r.level]}
        <li class={r.level}><I size={15} /><div><span class="mono">{r.text}</span>{#if r.detail}<span class="muted small">{r.detail}</span>{/if}</div></li>
      {/each}
    </ul>
    <label class="field"><span>Compose file as it goes to the other server (you can still change it)</span><textarea class="mono" rows="10" bind:value={file} spellcheck="false"></textarea></label>
    <label class="field"><span>.env</span><textarea class="mono" rows="3" bind:value={env} spellcheck="false"></textarea></label>
    {#if bad}<label class="check"><input type="checkbox" bind:checked={anyway} /> Move anyway ({bad} {bad === 1 ? "problem" : "problems"} above)</label>{/if}
  {/if}
  {#if phase}<p class="phase"><RefreshCw size={14} class="spin" /> {phase === "stopping" ? "Stopping it here..." : phase === "starting" ? "Starting it there..." : phase === "waiting" ? "Waiting until it is healthy there..." : "Going back..."}</p>{/if}
  {#if error}<pre class="bad-text">{error}</pre>{/if}
  {#if recent.length}
    <p class="muted small">Earlier: {#each recent as m, i}{i ? "; " : ""}{new Date(m.at).toLocaleString()} {m.from} <ArrowRight size={11} /> {m.to}, {m.result}{/each}</p>
  {/if}
  {#snippet footer()}
    <button class="btn" onclick={onclose}>Close</button>
    <button class="btn danger" disabled={!rows || !!phase || (bad > 0 && !anyway)} onclick={move}>Move</button>
  {/snippet}
</Modal>

<style>
  .top {
    display: flex;
    align-items: flex-end;
    gap: 10px;
    margin-bottom: 10px;
  }
  .grow {
    flex: 1;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .field > span {
    font-size: 12px;
    color: var(--muted);
  }
  .pin {
    padding-bottom: 6px;
  }
  .report {
    list-style: none;
    margin: 6px 0 12px;
    padding: 0;
    max-height: 260px;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 3px;
  }
  .report li {
    display: flex;
    gap: 8px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--border);
  }
  .report li:last-child {
    border-bottom: none;
  }
  .report li > div {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    word-break: break-all;
  }
  .report :global(svg) {
    flex-shrink: 0;
    margin-top: 2px;
  }
  .ok :global(svg) {
    color: var(--ok);
  }
  .warn :global(svg) {
    color: var(--warn);
  }
  .bad :global(svg) {
    color: var(--danger);
  }
  .info :global(svg) {
    color: var(--muted);
  }
  textarea {
    resize: vertical;
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .small {
    font-size: 12px;
  }
  .phase {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .bad-text {
    color: var(--danger-text);
    white-space: pre-wrap;
    font-family: inherit;
    margin: 8px 0 0;
  }
</style>
