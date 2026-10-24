// Containers on SSH servers: which servers turned out to have Docker (so the search box can offer them), and the
// terminal tabs for a container's logs and a shell inside it, which are ordinary SSH tabs running docker.
import { app } from "./state.svelte";
import { api, type DockerContainer, type DockerListing, type Session } from "./api";

/**
 * A compose file of a Portainer stack: from this server's Portainer, or, for a server Portainer manages through
 * its agent (a NAS), from the Portainer on another open server. A file counts only when it names the stack's
 * services, so the stack with the same number in another Portainer is not taken for it.
 */
export async function portainerFile(tab: string, listing: DockerListing, sudo: boolean, path: string, services: string[]): Promise<string> {
  const fits = (text: string) => services.every((s) => !s || new RegExp(`^\\s+["']?${s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}["']?\\s*:`, "m").test(text));
  if (listing.portainer) {
    const text = await api.dockerPortainerFile(tab, sudo, listing.portainer.container, listing.portainer.data, path);
    if (fits(text)) return text;
  }
  for (const t of app.tabs.filter((x) => x.type === "docker" && x.id !== tab && x.status === "connected")) {
    const l = await api.dockerList(t.id).catch(() => null);
    if (!l?.portainer) continue;
    const text = await api.dockerPortainerFile(t.id, l.access === "sudo", l.portainer.container, l.portainer.data, path).catch(() => "");
    if (text && fits(text)) return text;
  }
  throw new Error("This stack was made in Portainer, but none of the open servers runs the Portainer that made it. Open the Containers of the server where that Portainer runs, then try again.");
}

const KEY = "docker.known";

function read(): string[] {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "[]");
  } catch {
    return [];
  }
}

class Containers {
  /** Saved SSH sessions that answered with Docker, on this PC. */
  known = $state<string[]>(read());

  remember(sessionId: string | undefined, hasDocker: boolean) {
    if (!sessionId || this.known.includes(sessionId) === hasDocker) return;
    this.known = hasDocker ? [...this.known, sessionId] : this.known.filter((x) => x !== sessionId);
    try {
      localStorage.setItem(KEY, JSON.stringify(this.known));
    } catch {
      /* only a convenience */
    }
  }

  /** The SSH sessions known to have Docker, for the search box. */
  get sessions(): Session[] {
    return app.sessions.filter((s) => s.kind === "ssh" && this.known.includes(s.id));
  }

  /** Containers tabs that use Docker through sudo with the login's password. */
  asRoot = $state<Record<string, true>>({});

  /** How docker is started in a terminal of the server: directly, sudo without a password, or sudo that asks. */
  prefix(tab: string, sudo: boolean): string {
    return this.asRoot[tab] ? "sudo " : sudo ? "sudo -n " : "";
  }

  /** A terminal tab on the same server running one docker command, closed when the command ends. */
  private run(server: Session, title: string, command: string) {
    app.openSession({
      ...server,
      id: "",
      name: title,
      data: { ...(server.data ?? {}), command, stay_after_command: false, files: "hide" },
    });
  }

  logs(server: Session, c: DockerContainer, prefix: string) {
    this.run(server, `${c.name} logs`, `${prefix}docker logs -f --tail 300 ${c.name}`);
  }

  shell(server: Session, c: DockerContainer, prefix: string) {
    this.run(server, `${c.name} shell`, `${prefix}docker exec -it ${c.name} sh -c 'command -v bash >/dev/null && exec bash || exec sh'`);
  }
}

export const containers = new Containers();

/** A compose file to start new stacks from, with {{blanks}} asked when used. Travels with Sync. */
export interface StackTemplate {
  id: string;
  name: string;
  description?: string;
  compose: string;
  env?: string;
}

/** Same rule as the Rust side (docker::secret_name): a part of the name says password, token, key or secret. */
const SECRET_PARTS = new Set(["PASSWORD", "PASS", "PASSWD", "PWD", "SECRET", "TOKEN", "KEY", "APIKEY", "PRIVATE", "CREDENTIALS"]);
export function secretName(name: string): boolean {
  return name.toUpperCase().split(/[^A-Z0-9]+/).some((p) => SECRET_PARTS.has(p));
}

/**
 * The text with the values of secret-looking settings replaced by a {{blank}} of their name, so a template never
 * keeps a password: "KEY: value", "KEY=value" and "- KEY=value" lines of a compose or .env file. Values that are
 * already a blank or a ${variable} stay.
 */
export function blankSecrets(text: string): string {
  return text.replace(/^([ \t]*(?:-[ \t]*)?["']?)([A-Za-z_][A-Za-z0-9_.-]*)(["']?[ \t]*[:=][ \t]*)(["']?)([^\n"']*?)(["']?)([ \t]*)$/gm, (line, lead, key, sep, q1, value, q2, tail) => {
    if (!secretName(key) || !value.trim() || /^\{\{.*\}\}$|^\$\{.*\}$/.test(value.trim())) return line;
    // An unquoted YAML value gets quotes: a filled-in password may hold ": " or " #".
    if (!q1 && sep.includes(":")) return `${lead}${key}${sep}"{{${key}}}"${tail}`;
    return `${lead}${key}${sep}${q1}{{${key}}}${q2}${tail}`;
  });
}

class Templates {
  get list(): StackTemplate[] {
    return (app.settings["docker.templates"] as StackTemplate[]) ?? [];
  }

  async save(t: StackTemplate) {
    const rest = this.list.filter((x) => x.id !== t.id);
    await app.setSetting("docker.templates", [...rest, t].sort((a, b) => a.name.localeCompare(b.name)));
  }

  async remove(id: string) {
    await app.setSetting("docker.templates", this.list.filter((x) => x.id !== id));
  }
}

export const templates = new Templates();

/** One stack moved from one server to another: who did what, kept on this PC (newest first, at most 200). */
export interface MoveRecord {
  at: number;
  project: string;
  from: string;
  to: string;
  /** moved, rolled back, failed */
  result: string;
  note?: string;
}

class Moves {
  get list(): MoveRecord[] {
    return (app.settings["docker.moves"] as MoveRecord[]) ?? [];
  }

  async add(m: MoveRecord) {
    await app.setSetting("docker.moves", [m, ...this.list].slice(0, 200));
  }
}

export const moves = new Moves();
