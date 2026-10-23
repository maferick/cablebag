# What's new in Cablebag

Cablebag updates itself: when a new version is out, the status line shows **Update to ...**. One click
installs it and starts Cablebag again. Every version is also on the
[releases page](https://github.com/maferick/cablebag/releases).

---

## 0.17.0

**Web sessions for more than one tenant or account**

- **Browser profiles** (Settings, Browser profiles): each profile keeps its own cookies and logins, so web sessions
  can be signed in to two Azure tenants, two Teams or two of anything side by side. Choose a profile per web
  session or once for a folder, as with the WireGuard network. **Private** keeps nothing after the tab closes.
  "Sign out everywhere" clears a profile; the list of profiles goes along with Sync (cookies never do).
- **Colour per identity**: a web tab shows its profile's name and colour on the tab and on the pane header, its
  address bar and page get a frame in that colour, and the session tree shows the profile next to each web
  session and folder. Two tabs of the same site in different tenants never look alike.
- **Open this page in another profile**: click the profile's name in the address bar to open the same page in
  another profile, privately, or in the shared one.
- **Unread counts**: chat and mail pages that show a count in their title, such as Teams, get a badge on their
  tab, and the total shows on Cablebag's taskbar button.
- **Notifications** from web tabs (a Teams message, for example) show as Windows notifications with the
  profile's name; clicking one opens the tab. Switch them off in Settings, Browser profiles (do not disturb), or
  per profile.
- **Keep running**: a web session can open when Cablebag starts and keep running in the background, so its
  notifications keep coming. Other web tabs can go to sleep after a while to save memory (off unless you
  choose it) and open again where they were.
- Links that open a new tab now open in a Cablebag tab with the same profile and network. Sign-in pop-ups open
  as Cablebag windows in the same profile, titled with the profile and page, and close with their tab. Finished
  downloads are announced.
- **Fill logins** in web tabs from your credential sources: the key button in the address bar offers the logins
  saved for the page's site (on Microsoft's and Google's sign-in pages, those saved for their sites), and fills the
  one you pick. Nothing is filled by itself; a login saved for another site is only filled after you confirm it.
- **Proton Pass** as a credential source, through Proton's own command-line tool pass-cli (it keeps its own
  sign-in through your web browser). Where pass-cli is stays a setting of this PC and never goes into the sync.
- **Import bookmarks** from Chrome, Edge and Firefox (Import sessions, in Settings, General or the command palette) as web sessions in the same folders.
  Importing again updates them instead of adding copies.

## 0.16.1 · 23 October 2026

**Security fixes from a code review**

- **Downloads stay in the folder you chose.** A server can name a file `..\..\something` or `C:\something`
  (ordinary names on Linux); Windows reads those as paths, so downloading such a file over SFTP or FTP, or
  opening it with a program on this PC, could write it elsewhere. Such characters now become `_`, and names
  Windows reserves (such as CON or COM1) get a `_` in front.
- **The assistant asks before more commands.** Some commands it treated as only reading can write files or
  start other programs: sed's w and e commands, awk with pipes or `print > file`, `env some-program`,
  `ip netns exec`, `sort -o`, `uniq in out`, `curl -o file` and cookie jars, `openssl ... -out`,
  `git branch name`, `nginx -s`, and on Windows `route delete`, `arp -d`, `ipconfig /release` and method calls
  such as `.Terminate()`. These now wait for your approval like any other change.
- **The remote editor notices every change on the server** before it saves: it compares the content, not the
  modification time (which counts in whole seconds).
- An answer from an AI endpoint is stopped when it grows past 16 MB, so a broken endpoint cannot fill memory.

## 0.16.0 · 23 October 2026

**Credential sources: several vaults, CyberArk and Delinea**

- **Settings, Credential sources** (was "Vault (Vaultwarden)"): as many sources as you like, side by side, such
  as a Vaultwarden for work and one for home. Your existing vault becomes the first source by itself; nothing
  to set up again, and every login already linked keeps working.
- **CyberArk** (Privilege Cloud or your own PVWA) and **Delinea Secret Server** as new kinds of source:
  sign in (single sign-on included), look up logins, give a reason or ticket when a safe asks for one, and
  check logins out and in. Built from the official documentation; please tell us how it works with yours.
- **Through CyberArk PSM**: SSH and SFTP through PSM for SSH, Remote Desktop through PSM, per session or for a
  whole folder.
- The login picker shows the logins of every open source, grouped by source.

**New session types and options**

- **PowerShell remoting (WinRM)**: a remote PowerShell on a Windows server in a terminal tab, with the login
  from your vault. Windows' own PowerShell does the connecting; the password never shows on screen or on a
  command line.
- **Linux windows on this PC (X11)**: an SSH option that shows the windows of Linux programs on your desktop.
  The X server (VcXsrv) now comes with Cablebag and starts by itself when needed, which makes the installer
  larger (about 47 MB).

**Easier to use**

- **Report a problem**, in Settings, About Cablebag, the search box and error messages: describe what happened
  and GitHub's bug form opens filled in, after Cablebag has hidden anything that looks like a password or key.
  You check and submit it on GitHub yourself.
- **Keyboard and screen readers**: dialogs close with Escape and keep the focus inside, menus open with the
  context menu key or Shift+F10 and work with the arrow keys, the tabs and the session tree follow the arrow
  keys, keyboard focus is always visible, the sidebar width can be changed by keyboard, and the terminal has
  a "Screen reader support" setting. Fewer animations when Windows asks for that. See
  [ACCESSIBILITY.md](ACCESSIBILITY.md).

**Safer**

- The WireGuard ports on this PC (the web proxy and the forwards for Remote Desktop, VNC and telnet) now only
  let in Cablebag's own programs; other programs on your PC are refused.

## 0.15.3 · 23 October 2026

- **Send to all ends by itself**: it switches off as soon as fewer than two terminals are open, so closing your
  tabs ends it, and it can only be switched on with two or more connected terminals. The bar shows only over a
  terminal, not over a Remote Desktop or another tab, where typing does not go to the terminals.

## 0.15.2 · 23 October 2026

- **Everything on current versions**: the parts Cablebag is built from are brought up to their latest major
  versions (among them SvelteKit 3, reqwest 0.13 and the cryptography libraries used for Vaultwarden, the vault
  and sync), and WireGuard runs on boringtun 0.7. That also closes known issues in older versions of ring and
  curve25519-dalek. Nothing changes in how Cablebag works.
- Cablebag is now developed in the open on GitHub, with automatic checks on Windows and Linux, a user guide in
  the [wiki](https://github.com/maferick/cablebag/wiki) and [Discussions](https://github.com/maferick/cablebag/discussions)
  for questions and ideas.

## 0.15.1 · 22 October 2026

- **First steps towards Linux**, by @martijnvandijk: the Windows-only parts are kept to Windows, so the rest
  builds and its tests run on Linux. Off Windows the vault key is now protected by a key file only your account
  can read, instead of lying in the database as it was. Nothing changes on Windows.

## 0.15.0 · 22 October 2026

**Proxmox behind WireGuard**

- **A Proxmox server can go through a WireGuard network**: choose it under Network when you add or edit the
  server (Settings, Proxmox). Its machines list, start and stop, the console, snapshots and the node shell then all
  go through that network.
- **Sessions Cablebag makes for its machines** (SSH, RDP) go through the same network.
- **The Proxmox web interface and the console in a browser** open in a Cablebag tab through that network, since
  the web browser on this PC cannot reach it.

## 0.14.2 · 22 October 2026

- **WireGuard networks are made where you use them**: next to Network in a session or a folder there are
  **Import .conf**, **New** and **Edit**. A network you add there is chosen for that session straight away. The
  Tunnels page keeps the overview with their live state.

## 0.14.1 · 22 October 2026

- **Edit session works again**: since 0.13.0, opening the editor of a session without a WireGuard network stopped
  Cablebag's window from updating, so the editor did not appear and menus stayed open.

## 0.14.0 · 22 October 2026

**WireGuard for web pages**

- **Web sessions go through WireGuard too**: choose the network under Network, as for SSH or RDP. Cablebag
  gives the page a proxy of its own that only that tab uses, so every address, including local ones, goes through
  the tunnel, and WebRTC cannot show the page this PC's own address. Each WireGuard network keeps its cookies
  apart, and they are removed with the network.
- **Public key of a key in Vaultwarden**: the WireGuard editor can show it (Cablebag reads the key from the
  vault for it), and reads it again when the key changed there.
- **No reachability dot for sessions that go through WireGuard**: checked from this PC the address means
  something else, and could even be a machine on your own network.

## 0.13.0 · 22 October 2026

**WireGuard per session**

- **Cablebag runs WireGuard itself**, only for the sessions that choose it. Each WireGuard network is its own
  small network inside Cablebag, so two VPNs whose machines have the same addresses work at the same time.
  Nothing changes in Windows: no adapter, no routes, no admin rights.
- **Tunnels, WireGuard networks:** import the .conf you use in the WireGuard app, or fill it in. The private key
  is kept in Cablebag's vault (or comes from a Vaultwarden item). **Test** shakes hands with the server, and while
  a network runs you see its open connections, last handshake and data.
- **Network** in a session (SSH, SFTP, RDP, VNC, telnet) or a folder picks the WireGuard network. A folder's
  choice goes to its sessions, and a session or subfolder can still go directly.
- The same .conf can be used in Cablebag and in the WireGuard app, just not both switched on at once (the server
  talks to one of them). Cablebag warns when it sees the WireGuard app has the same tunnel on. For both at once,
  **New key pair** makes a key of Cablebag's own; its public key goes on the server as an extra peer.
- WireGuard networks travel with Sync; their keys do not.
- **Settings and Tunnels close again** when you press their button a second time, and you are back in the tab
  you came from.

## 0.12.1 · 22 October 2026

- **Disconnect no longer reports an error**: ending a remote desktop session with Disconnect said "An internal
  error has occurred." with lines of test output under it. It now says you disconnected the session, and a
  session ended on the remote computer says that.

## 0.12.0 · 22 October 2026

**Edit remote files in Cablebag**

- **A double-click on a text file opens it in Cablebag's editor** (over SFTP): line numbers, Ctrl+S, and before
  anything is written you see exactly what changes, removed lines in red and new ones in green. The version on
  the server is kept as name.bak (as private as the file itself), and Cablebag refuses to overwrite a file that
  changed on the server since you opened it. Larger or binary files open with a program on this PC, as before;
  right-click offers both.
- An editor tab with unsaved changes shows a dot and asks before it closes.
- **Sync names its conflicts**: Settings, Sync lists the items that were changed on this PC and another one since
  the last sync, and whose version was kept.

## 0.11.0 · 22 October 2026

**More of Proxmox in the tree**

- **Snapshots** (right-click a VM or container): see them, take one (for a VM optionally with its memory), roll
  back or delete one. Cablebag follows the Proxmox task until it is done. The API token needs the `VM.Snapshot`
  privilege (and `VM.Snapshot.Rollback` to roll back); without it Cablebag says so.
- **Tasks**: the recent tasks of a Proxmox server, or of one guest, with what each one wrote.
- **A shell on the node** (right-click the Proxmox server): over SSH, with your saved session for the node when
  there is one.
- **The assistant takes a snapshot before its first change inside a guest**, after one click, or by itself
  (Settings, Assistant, Proxmox guests).

## 0.10.0 · 22 October 2026

**Folders that do the work**

- **Folder settings**: a folder can give its sessions a user name, a key file, a login from the vault, jump hosts
  and a tab colour (right-click the folder, Folder settings). Sessions use them wherever they have no value of
  their own; a subfolder can set its own, and the session editor shows what comes from the folder. Folder
  settings travel with Sync.
- **Run on many servers** (Tools, or right-click a folder): one command on every SSH session of a folder, a few at
  a time, with the answers side by side. Uptime, disk space, pending updates or failed services on 30 servers in
  one go; click a row for the whole output, copy or save all of it. Optionally as root, where sudo needs no
  password.

## 0.9.0 · 22 October 2026

**Pick up where you left off**

- **The tabs from last time.** Cablebag remembers the tabs that were open when it closed; Home offers to open them
  again in one click, or they open by themselves at the start (Settings, General, Tabs from last time).
- **Workspaces**: save the open tabs under a name, such as everything for one customer, and open them again from
  Home, the + menu or the search box. Workspaces travel with Sync.
- **SSH connects again by itself** when the connection drops (sleep, a network change, a VPN that goes away): in
  the same terminal, with everything it showed, after 2, 5, 10, 20 and 30 seconds and then every 30 seconds for
  up to ten minutes. Esc stops it; Settings, SSH can switch it off. Pressing R in an ended terminal also connects
  again in place now, instead of starting an empty one.
- **SSH agent forwarding**, per session (Advanced): the server can use the keys in Pageant or the Windows OpenSSH
  agent while you are connected, for example to log in further from a jump server. Off unless you switch it on,
  and only for servers you trust.

## 0.8.0 · 22 October 2026

**Rough edges smoothed, and more in About**

- **Keyboard shortcuts** has its own page in Settings (also from the search box), with every shortcut and the
  macros that have one.
- **About Cablebag** can back up all your data to one file and restore such a backup (it takes the place of the
  current data at the next start, which is kept). **Copy details** puts the version, Windows, WebView2 and .NET
  versions on the clipboard for a problem report, and the page says so when Remote Desktop's .NET 8 runtime is
  missing. It also shows when updates were last checked.
- **Assistant limits are settings now** (Settings, Assistant, Limits): terminal lines per question, commands and
  minutes per question, how much command output it reads, how much memory goes along, and the conversation size.
- **Long assistant conversations keep working.** Older questions lose their terminal text, and above the size limit
  the oldest turns are left out, so the AI host does not run out of room.
- **The model menu lists every model of the host**, and a conversation can be saved as a text file.
- **What was sent** can be filtered by server or word, shows more on request, and can be cleared.
- In a protected folder the assistant picks an AI host that may see it.
- Safer and fewer cards: `rm -rf` inside `/tmp` or `/var/tmp` is an ordinary change, `rm --recursive --force` is
  caught like `rm -rf`, `docker compose ps` and `logs` only read, as do `passwd -S` and `chage -l`; `usermod` is a
  change instead of dangerous.
- File and web sessions imported from MobaXterm open again (they said the session had no path).
- The session editor no longer shows serial and address fields twice, and X11 forwarding says plainly it is not
  available yet.

## 0.7.2 · 22 October 2026

- **About Cablebag**, in Settings and in the search box (Ctrl+K): the version, updates, what the licence
  allows, where your data is kept (with **Open**), and links to what's new, the project page and its issues.
  Updates moved here from General.
- **Remote files when an SSH session connects** (show or don't show) is now in Settings, General, where you would
  look for it, instead of under SSH.

## 0.7.1 · 22 October 2026

- **PowerShell commands in Windows VMs are judged as PowerShell.** `Get-`, `Select-`, `Measure-` and other looking
  commands run by themselves; `Set-`, `Stop-` and the like count as changes; `Restart-Computer`, `Format-Volume`,
  `Clear-Disk` and `Remove-Item -Recurse` are dangerous. Before, every PowerShell command asked as a change.

## 0.7.0 · 22 October 2026

**The assistant inside Proxmox guests, and on a longer leash if you want**

- **It runs commands inside Proxmox VMs and containers**, from the guest's console tab: in a VM through the QEMU
  guest agent (PowerShell on Windows), in a container with `pct exec` on its node over SSH. Everything runs as
  root, so the first command asks once. A VM needs the guest agent switched on and installed, and the API token
  needs the `VM.GuestAgent.Unrestricted` privilege (Proxmox 9). For a container, Cablebag uses your saved SSH
  session for the node, or logs in as root with the vault.
- **Choose what it may do by itself** (Settings, Assistant): read by itself and ask before every change, as
  before, or do what is needed and ask only before dangerous commands, such as `rm -rf /`, wiping a disk, a
  reboot or destroying a VM. Protected folders always ask for everything. The top of the assistant column shows
  the choice, with **Change**.

## 0.6.0 · 22 October 2026

**The assistant on Remote Desktop and VNC**

- **It sees the remote screen.** Every question in a Remote Desktop or VNC tab takes a picture of the screen along
  (switch it off with **Send the screen with it**). The picture comes from Cablebag's own window, also when another
  program covers it, so no other window can end up in it.
- **It can click, scroll, type and press keys** on that screen, to do what you ask. Every action waits for your
  **Do it**, or **Always on this screen**; after its actions it gets a new picture and checks the result.
- **A Remote Desktop session gets the size of its pane** when it connects, also for a new pane that is still being
  laid out. Some servers cannot change the size later and stayed tiny.
- **Switching tabs while a pane fills the area** now shows the tab you pick (also Settings), instead of nothing.
- Ctrl+Alt+Del in a Remote Desktop tab sends the three keys reliably.

## 0.5.4 · 22 October 2026

- **The assistant asks for a sudo password only when it needs one.** On servers where sudo works without a
  password, it no longer asks. Otherwise it first looks in your vault for a login for that server, also when
  you logged in with a key. A password you type can be remembered, so it is asked only once.

## 0.5.3 · 22 October 2026

- **Clicking a Remote Desktop pane makes it the active one.** Before, the pane only lit up after you clicked its
  title bar, because the click went straight to the remote screen.

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
