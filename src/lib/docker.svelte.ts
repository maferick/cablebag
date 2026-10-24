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
