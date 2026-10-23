<script lang="ts">
  import { isAppShortcut } from "../shortcuts";
  import { ask } from "../ask.svelte";
  import { onMount, onDestroy } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { WebglAddon } from "@xterm/addon-webgl";
  import { SearchAddon } from "@xterm/addon-search";
  import { WebLinksAddon } from "@xterm/addon-web-links";
  import { Unicode11Addon } from "@xterm/addon-unicode11";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { readText, writeText } from "../clipboard";
  import "@xterm/xterm/css/xterm.css";
  import { api, Channel } from "../api";
  import { app, type Tab } from "../state.svelte";
  import { schemeById } from "../schemes";
  import { terminals, emitOutput } from "../terms";
  import { Highlighter, BUILTIN_SYNTAX, compile, type SyntaxSet } from "../highlight";
  import { macros } from "../macros.svelte";
  import { Search, X, ChevronUp, ChevronDown } from "@lucide/svelte";

  let { tab, visible }: { tab: Tab; visible: boolean } = $props();

  let host: HTMLDivElement;
  let term: Terminal | undefined;
  let fit: FitAddon | undefined;
  let search: SearchAddon | undefined;
  let ro: ResizeObserver | undefined;
  let findOpen = $state(false);
  let findText = $state("");
  let findInput = $state<HTMLInputElement>();
  let lastSize = "";

  function sendResize() {
    if (!term || !fit || !visible || !host?.offsetWidth) return;
    try {
      fit.fit();
    } catch {
      return;
    }
    const key = `${term.cols}x${term.rows}`;
    if (key !== lastSize) {
      lastSize = key;
      api.resize(tab.id, term.cols, term.rows).catch(() => {});
    }
  }

  async function paste() {
    const text = await readText();
    if (!text || !term) return;
    if (app.settings["term.pasteWarning"] && text.includes("\n") && text.trim().includes("\n")) {
      const lines = text.trim().split("\n").length;
      if (!(await ask.confirm("Paste several lines?", `You are about to paste ${lines} lines. Each line may run as a command.`, { okLabel: "Paste" }))) return;
    }
    term.paste(text);
  }

  /** The cursor blinks when the setting says so, unless Windows asks for less motion. */
  function blinks(): boolean {
    return !!app.settings["term.cursorBlink"] && !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }

  export function focus() {
    term?.focus();
  }

  onMount(() => {
    const scheme = schemeById(app.settings["term.scheme"]);
    term = new Terminal({
      fontFamily: `"${app.settings["term.font"]}", "Cascadia Mono", Consolas, monospace`,
      fontSize: Number(app.settings["term.fontSize"]) || 14,
      scrollback: Number(app.settings["term.scrollback"]) || 20000,
      cursorStyle: app.settings["term.cursor"],
      cursorBlink: blinks(),
      screenReaderMode: !!app.settings["term.screenReader"],
      theme: scheme.theme,
      allowProposedApi: true,
      macOptionIsMeta: true,
      rightClickSelectsWord: false,
    });
    fit = new FitAddon();
    search = new SearchAddon();
    term.loadAddon(fit);
    term.loadAddon(search);
    term.loadAddon(new WebLinksAddon((_e, uri) => openUrl(uri)));
    const uni = new Unicode11Addon();
    term.loadAddon(uni);
    term.unicode.activeVersion = "11";
    term.open(host);
    try {
      term.loadAddon(new WebglAddon());
    } catch {
      /* falls back to the DOM renderer */
    }
    fit.fit();

    term.onData((d) => {
      macros.record(tab.id, d);
      if (app.multiExec) {
        for (const t of app.tabs) if (t.type === "terminal" && t.status === "connected") api.input(t.id, d).catch(() => {});
      } else {
        api.input(tab.id, d).catch(() => {});
      }
    });
    term.onSelectionChange(() => {
      if (app.settings["term.copyOnSelect"] && term?.hasSelection()) writeText(term.getSelection());
    });
    term.attachCustomKeyEventHandler((e) => {
      if (e.type !== "keydown") return true;
      if (e.ctrlKey && e.shiftKey && e.code === "KeyC") {
        if (term?.hasSelection()) writeText(term.getSelection());
        return false;
      }
      if (e.ctrlKey && e.shiftKey && e.code === "KeyV") {
        paste();
        return false;
      }
      if (e.ctrlKey && e.shiftKey && e.code === "KeyF") {
        openFind();
        return false;
      }
      // Cablebag's own shortcuts (close tab, new terminal, palette, macros...) go to the window, not the shell.
      if (isAppShortcut(e)) return false;
      if (retry && e.type === "keydown" && e.key === "Escape") {
        stopRetry("Not connecting again. Press R to connect again.");
        return false;
      }
      if ((tab.status === "closed" || tab.status === "failed") && !e.ctrlKey && !e.altKey && e.key.toLowerCase() === "r") {
        stopRetry();
        app.restart(tab.id);
        return false;
      }
      return true;
    });
    host.addEventListener("contextmenu", (e) => {
      if (app.settings["term.rightClickPaste"] && !e.ctrlKey && !e.shiftKey) {
        e.preventDefault();
        paste();
      }
    });

    terminals.set(tab.id, term);
    connect();

    ro = new ResizeObserver(() => sendResize());
    ro.observe(host);
    if (visible) setTimeout(() => term?.focus(), 30);
  });

  /** Opens the connection (again) into this terminal; what it showed so far stays. */
  function connect() {
    if (!term) return;
    const syntaxId = tab.session?.data?.syntax || app.settings["term.syntax"] || "keywords";
    const set: SyntaxSet | undefined =
      syntaxId === "custom" ? (app.settings["term.customSyntax"] as SyntaxSet) : BUILTIN_SYNTAX.find((x) => x.id === syntaxId);
    const hl = new Highlighter(set ? compile(set) : null);
    const decoder = new TextDecoder("utf-8");
    const ch = new Channel<ArrayBuffer>();
    ch.onmessage = (buf) => {
      const text = decoder.decode(new Uint8Array(buf), { stream: true });
      emitOutput(tab.id, text);
      term?.write(hl.process(text));
    };
    const cols = term.cols;
    const rows = term.rows;
    lastSize = `${cols}x${rows}`;
    const log = logSpec();
    const opened =
      tab.proto === "ssh" && tab.session
        ? api.openSsh(tab.id, $state.snapshot(tab.session), cols, rows, log, ch)
        : (tab.proto === "telnet" || tab.proto === "serial" || tab.proto === "winrm") && tab.session
          ? api.openStream(tab.id, $state.snapshot(tab.session), cols, rows, log, ch)
          : api.openLocal(tab.id, $state.snapshot(tab.local!), cols, rows, log, ch);
    opened.catch((e) => {
      term?.write(`\r\n\x1b[31m${String(e)}\x1b[0m\r\n`);
      tab.status = "failed";
    });
  }

  // Connect again in place (R, Reconnect, or by itself after a lost connection).
  let seenReconnect = 0;
  $effect(() => {
    const n = tab.reconnect ?? 0;
    if (n === seenReconnect) return;
    seenReconnect = n;
    api.close(tab.id).catch(() => {});
    setTimeout(() => connect(), 300);
  });

  // A lost SSH connection connects again by itself: after 2, 5, 10, 20 and 30 seconds, then every 30 seconds,
  // for at most ten minutes. Esc stops it.
  const DELAYS = [2, 5, 10, 20, 30];
  let retry: { tries: number; since: number; timer?: ReturnType<typeof setTimeout> } | null = null;
  function stopRetry(note = "") {
    if (retry?.timer) clearTimeout(retry.timer);
    retry = null;
    if (note) term?.write(`\x1b[2m\r\n${note}\x1b[0m\r\n`);
  }
  $effect(() => {
    const status = tab.status;
    if (status === "connected") {
      if (retry) stopRetry();
      return;
    }
    if ((status !== "closed" && status !== "failed") || app.settings["ssh.autoReconnect"] === false) return;
    if (!retry && !(status === "closed" && tab.lost)) return;
    if (retry?.timer) return;
    retry ??= { tries: 0, since: Date.now() };
    if (Date.now() - retry.since > 10 * 60_000) return stopRetry("Stopped trying after ten minutes. Press R to connect again.");
    const wait = DELAYS[Math.min(retry.tries, DELAYS.length - 1)];
    term?.write(`\x1b[2m\r\n${retry.tries ? "Still no connection." : `${tab.lost ?? "The connection was lost."}`} Connecting again in ${wait} s; Esc stops this.\x1b[0m\r\n`);
    retry.timer = setTimeout(() => {
      if (!retry) return;
      retry.timer = undefined;
      retry.tries += 1;
      app.restart(tab.id);
    }, wait * 1000);
  });

  onDestroy(() => {
    if (retry?.timer) clearTimeout(retry.timer);
    ro?.disconnect();
    terminals.delete(tab.id);
    term?.dispose();
  });

  /** Where this terminal is logged, from the logging settings; undefined when logging is off. */
  function logSpec() {
    if (!app.settings["log.enabled"] || !app.settings["log.dir"]) return undefined;
    const d = new Date();
    const p2 = (n: number) => String(n).padStart(2, "0");
    const s = tab.session;
    const vars: Record<string, string> = {
      S: tab.proto === "local" ? (s?.kind ?? "local") : tab.proto,
      H: s?.host || tab.title,
      U: s?.username || "",
      P: s?.port ? String(s.port) : "",
      Y: String(d.getFullYear()),
      M: p2(d.getMonth() + 1),
      D: p2(d.getDate()),
      T: `${p2(d.getHours())}${p2(d.getMinutes())}${p2(d.getSeconds())}`,
    };
    const name = String(app.settings["log.name"] || "&S_&H_&Y-&M-&D_&T.log")
      .replace(/&([SHUPYMDT])/g, (_, k) => vars[k] ?? "")
      .replace(/[<>:"/\\|?*]+/g, "_");
    const dir = String(app.settings["log.dir"]).replace(/[\\/]+$/, "");
    return { path: `${dir}\\${name}`, kind: String(app.settings["log.kind"] || "text") };
  }

  $effect(() => {
    if (visible && term) {
      requestAnimationFrame(() => {
        sendResize();
        if (!findOpen && app.activeId === tab.id) term?.focus();
      });
    }
  });

  // Live settings: font size, scheme.
  $effect(() => {
    if (!term) return;
    term.options.fontSize = Number(app.settings["term.fontSize"]) || 14;
    term.options.theme = schemeById(app.settings["term.scheme"]).theme;
    term.options.fontFamily = `"${app.settings["term.font"]}", "Cascadia Mono", Consolas, monospace`;
    term.options.cursorStyle = app.settings["term.cursor"];
    term.options.cursorBlink = blinks();
    term.options.screenReaderMode = !!app.settings["term.screenReader"];
    requestAnimationFrame(() => sendResize());
  });

  let announced = false;
  $effect(() => {
    if ((tab.status === "closed" || tab.status === "failed") && term && !announced && !retry && !(tab.lost && app.settings["ssh.autoReconnect"] !== false)) {
      announced = true;
      term.write("\x1b[2m\r\nPress R to reconnect, or close this tab.\x1b[0m\r\n");
    }
    if (tab.status === "connected") announced = false;
  });

  function openFind() {
    findOpen = true;
    if (term?.hasSelection()) findText = term.getSelection();
    setTimeout(() => findInput?.select(), 10);
  }

  function find(next = true) {
    if (!findText) return;
    const opts = { decorations: { matchOverviewRuler: "#e07a43", activeMatchColorOverviewRuler: "#e07a43", matchBackground: "#7a4a2e", activeMatchBackground: "#e07a43" } };
    if (next) search?.findNext(findText, opts);
    else search?.findPrevious(findText, opts);
  }
</script>

<div class="wrap" style:display={visible ? "flex" : "none"} style:background={schemeById(app.settings["term.scheme"]).theme.background}>
  {#if findOpen}
    <div class="find">
      <Search size={14} />
      <input
        bind:this={findInput}
        bind:value={findText}
        placeholder="Find in terminal"
        onkeydown={(e) => {
          if (e.key === "Enter") find(!e.shiftKey);
          if (e.key === "Escape") {
            findOpen = false;
            search?.clearDecorations();
            term?.focus();
          }
        }}
      />
      <button class="icon-btn" title="Previous match" aria-label="Previous match" onclick={() => find(false)}><ChevronUp size={15} /></button>
      <button class="icon-btn" title="Next match" aria-label="Next match" onclick={() => find(true)}><ChevronDown size={15} /></button>
      <button
        class="icon-btn"
        title="Close" aria-label="Close"
        onclick={() => {
          findOpen = false;
          search?.clearDecorations();
          term?.focus();
        }}><X size={15} /></button
      >
    </div>
  {/if}
  <div class="term" bind:this={host}></div>
</div>

<style>
  .wrap {
    position: absolute;
    inset: 0;
    flex-direction: column;
    padding: 6px 0 0 8px;
  }
  .term {
    flex: 1;
    min-height: 0;
  }
  .term :global(.xterm-viewport) {
    background: transparent !important;
  }
  .find {
    position: absolute;
    top: 8px;
    right: 18px;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 6px 4px 10px;
    background: var(--raised);
    border: 1px solid var(--border);
    border-radius: 3px;
    box-shadow: var(--shadow);
    color: var(--muted);
  }
  .find input {
    width: 220px;
  }
</style>
