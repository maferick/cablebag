// Containers on SSH servers: which servers turned out to have Docker (so the search box can offer them), and the
// terminal tabs for a container's logs and a shell inside it, which are ordinary SSH tabs running docker.
import { app } from "./state.svelte";
import type { DockerContainer, Session } from "./api";

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

  /** A terminal tab on the same server running one docker command, closed when the command ends. */
  private run(server: Session, title: string, command: string) {
    app.openSession({
      ...server,
      id: "",
      name: title,
      data: { ...(server.data ?? {}), command, stay_after_command: false, files: "hide" },
    });
  }

  logs(server: Session, c: DockerContainer, sudo: boolean) {
    this.run(server, `${c.name} logs`, `${sudo ? "sudo -n " : ""}docker logs -f --tail 300 ${c.name}`);
  }

  shell(server: Session, c: DockerContainer, sudo: boolean) {
    this.run(server, `${c.name} shell`, `${sudo ? "sudo -n " : ""}docker exec -it ${c.name} sh -c 'command -v bash >/dev/null && exec bash || exec sh'`);
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
