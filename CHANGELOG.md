# What's new in Cablebag

Cablebag updates itself: when a new version is out, the status line shows **Update to ...**. One click
installs it and starts Cablebag again. Every version is also on the
[releases page](https://github.com/maferick/cablebag/releases).

---

## 0.5.2 · 22 October 2026

- **You can see when the assistant waits for you.** The conversation now follows every new command and card,
  and while a command waits for your OK, a bar above the question box says so, with **Show it**.
- `systemctl` with options before the subcommand (such as `systemctl --no-pager --failed`) counts as reading
  again, instead of as a change.

## 0.5.1 · 22 October 2026

- **Import from MobaXterm is back under its own name.** It finds `MobaXterm.ini` by itself and shows how many
  sessions it holds; you can also choose an `.mxtsessions` export.

## 0.5.0 · 22 October 2026

**The assistant remembers, and Settings you can read**

- **The assistant has a memory, inside Cablebag.** It remembers what it learns per connection (how a server is set
  up, what fixed a problem), per folder and in general, and starts each conversation with what it knows about
  that server. New memories show in the conversation with an **Undo**; see and edit them in a session's editor
  or in Settings › Assistant. The memory travels with Sync (Git), encrypted, and secrets are hidden before
  anything is stored.
- **Settings rebuilt.** Each page has a title, settings are grouped in cards with a short explanation next to
  each one, on/off settings are switches, and **Find a setting** filters the pages. The page no longer gets cut
  off halfway.
- The assistant's answers show **bold** and `code` properly.
- More commands count as only reading (`command -v`, `test`), and the password question for sudo says
  *Continue* instead of *Connect*.
- A shorter README with a screenshot.

## 0.4.0 · 22 October 2026

**The assistant can now work on your SSH servers**

- In an SSH tab the assistant **investigates by itself**: it runs commands that only read (status, logs, disk
  space, container lists) and uses what it finds.
- **Every change waits for you.** A card shows the command, why, how to undo it and what kind of change it is,
  with **Run it**, **Always** for that kind of command on that server, or **Refuse**. Cablebag decides what a
  command does, not the AI: commands that write files, run commands inside commands or cannot be checked
  always count as changes.
- **Dangerous commands** (deleting folders, formatting disks, rebooting, stopping SSH, firewall flushes,
  destroying machines) always get a red card and never an "Always".
- **Protected folders:** right-click a folder in the tree, *Protect from the assistant*. There every command
  asks, dangerous ones are refused, and only AI hosts you allowed are used.
- **Root without sharing your password:** after one click per conversation Cablebag gives sudo the password
  from the vault, a saved password or a one-time question; the AI never sees it.
- The commands run on their own channel of your SSH login, with exact output and exit codes; your terminal
  shows each one as a dimmed line. Limits of 25 commands and 15 minutes per question, and **Stop** works at
  any moment.

## 0.3.0 · 22 October 2026

**The assistant, first step: ask about what is on your screen**

- A new **Assistant** column at the right (<kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>A</kbd>). Ask about the tab
  you are in, for example *"why does this service not start?"*. The last lines of the terminal go with the
  question.
- Works with any AI host that speaks the OpenAI API: LiteLLM, vLLM, Ollama or a cloud service. Add one in
  Settings › Assistant; the key comes from your vault or is kept on this PC, and **Test** shows the models
  and what each can do.
- **Secrets stay home.** Before anything is sent, Cablebag hides the passwords it knows and text that looks
  like a secret (private keys, tokens, `password=` values, password hashes).
- **Nothing runs by itself.** A command in an answer can be put at the prompt with one click; it only runs
  when you press Enter.
- Settings › Assistant lists everything that was sent and what came back.

## 0.2.3 · 22 October 2026

**Remote files your way**

- Choose whether the remote files show when an SSH session connects: **Show** or **Don't show**, in
  Settings › SSH.
- Each session can choose differently in its editor, for example closed for network switches and open for
  servers.
- The choice now belongs to each tab: hiding the files in one tab no longer hides them in another, and
  <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>E</kbd> works on the tab you are in.

## 0.2.2 · 22 October 2026

**Smoother updates**

- An update no longer stops with *"Error opening file for writing: cablebag-rdp.exe"* when a Remote Desktop
  tab was open. Cablebag closes its Remote Desktop helpers before it installs, a helper also quits by itself
  once Cablebag is gone, and the installer stops any helper that is left.

## 0.2.1 · 22 October 2026

**A look of its own, and imports from other programs**

- **New colours.** Warm graphite and paper, with one rust accent that only marks what is yours: the focused
  pane, the selected row, the main button. Connection states have a shape as well as a colour (a filled
  circle when connected, a ring while connecting, a diamond when it failed), so they read for colour-blind
  eyes too.
- **Import sessions** from PuTTY and KiTTY saved sessions, WinSCP sites, your OpenSSH config file, Remote
  Desktop (`.rdp`) files and `.ini` or `.mxtsessions` session files. Each source gets its own folder;
  importing again updates those sessions instead of adding copies.
- **A new New session window:** the session types in a list with a short explanation each, next to one
  form, with the extra options folded underneath.
- **A new Tunnels page:** every tunnel is a card with its route drawn step by step, from where it listens to
  where it arrives.
- The logo now shows on GitHub in both light and dark mode.

## 0.2.0 · 22 October 2026

**The first public release**

- Tabs for SSH, Remote Desktop, VNC, Telnet, serial ports, SFTP and FTP, local shells and WSL, web pages,
  and the consoles of Proxmox machines.
- Remote files beside every SSH tab, tiling of all tabs with resizing and moving from the keyboard, a split
  view for terminals, and a search box that finds any session, tab or command.
- Logins from Vaultwarden, unlocked with Windows Hello; sessions synced through git, encrypted.
- Proxmox servers as live folders of their machines; tunnels, a network and port scanner, Wake on LAN and an
  SSH key generator.
- Updates from this page only, checked against the Cablebag signature before anything is installed.
