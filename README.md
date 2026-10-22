<picture>
  <source media="(prefers-color-scheme: dark)" srcset="branding/cablebag-logo.svg">
  <img src="branding/cablebag-logo-on-light.svg" alt="Cablebag" height="64">
</picture>

All-in-one remote access workstation for Windows: tabbed terminals, SSH with a remote file browser, RDP, VNC,
Proxmox consoles, tunnels and network tools in one window.

## Install

Download `Cablebag_<version>_x64-setup.exe` from the [latest release](https://github.com/maferick/cablebag/releases/latest)
and run it. It installs for your Windows account only, no administrator rights needed. Cablebag then keeps
itself up to date: it checks this page at the start and every six hours, shows "Update to ..." in the status
line, and installs an update only when it carries the Cablebag signature. Updates come from this page and
nowhere else.

What changed in each version: [CHANGELOG.md](CHANGELOG.md).

Needs Windows 10 or 11. Remote Desktop tabs need the
[.NET 8 Desktop Runtime](https://dotnet.microsoft.com/download/dotnet/8.0).

## Licence

Cablebag is free to use, but it is **not open source**: you may install and use it, but not share copies,
change it or build on its code. The source is published so you can see what it does. See [LICENSE](LICENSE);
the open-source components inside are listed in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

## What it does

- **The window**: one strip at the top with the sidebar button, the tabs, the + menu (new session, local
  terminals, tools), the search box, Layout and Settings. The session tree fills the left side, with Macros and
  Tools folded at the bottom; Remote files sit at the right of the tab area; a status line at the bottom shows
  connected tabs, tunnels, vault, sync, transfers, Send to all and Record. Home is what shows when no tab is
  open (Ctrl+Shift+H brings it back). Settings opens as a tab. Interface size 90 to 175% (Settings, General,
  or Ctrl+Shift+plus and minus).
- **Sessions**: folders, search, drag a session onto a folder to move it, edit, duplicate, delete.
- **Search box** at the top (Ctrl+K, or Ctrl+Shift+K inside a terminal): find any session, open tab, Proxmox
  guest, tool, macro, setting or action by typing, or type `user@host` to connect. Start with `>` for all
  commands.
- **Reachability dots**: a dot on each session in open folders shows whether its host answers (green), answers
  ping but not on its port (orange, when port checks are on), or does not answer (grey). Ping first by default,
  since port checks leave a line in the server's log; sessions behind a jump host or gateway are skipped.
- **Sync through git**: sessions, folders, macros, tunnels and Proxmox servers synced to a private repository
  (Settings, Sync): Gitea or Forgejo, GitHub or GitLab through their APIs, any other repository through the git
  program. Encrypted by default, each item on its own with a key from the vault (or a passphrase); three-way
  merge against the last sync; passwords never go there.
- **Vault unlock**: after one unlock with the master password the vault opens with Windows Hello on later
  starts (or without a question, or the master password every time: Settings, Vault). The kept key and login
  are protected by your Windows account (DPAPI).
- **Proxmox**: add Proxmox servers (Settings, Proxmox) with an API token from Vaultwarden or kept on this PC; their
  VMs and containers show as a live folder with state and addresses (guest agent or container interfaces).
  Double-click opens RDP for Windows and SSH for the rest (the console when no address is known); the console
  opens in a tab (Proxmox's VNC over its WebSocket) or in the browser; also save as a session,
  start, shut down, reboot and force stop. A self-signed certificate is trusted by fingerprint before the token
  is ever sent.
- **Import sessions**: from PuTTY and KiTTY saved sessions, WinSCP sites, the OpenSSH config file, Remote
  Desktop (`.rdp`) files and `.ini` or `.mxtsessions` session files, with their folders.
  Importing again updates the same sessions.
- **SSH**: password, keyboard-interactive (two-factor), private keys (OpenSSH and PuTTY `.ppk`, passphrase
  prompt), Pageant and the Windows OpenSSH agent, jump hosts in a chain, host key confirmation with SHA-256
  and MD5 fingerprints, keep-alive, command after login, reconnect with R.
- **Remote files** beside the tab area for the active SSH tab: browse, upload (button or drag from Explorer),
  download, new file or folder, rename, permissions, delete, open with the Windows program and upload again
  after saving, paste a path into the terminal with a middle click. Whether they show when an SSH session
  connects is a choice in Settings, SSH, and per session; Ctrl+Shift+E shows or hides them any time.
- **Local terminals**: PowerShell 7, Windows PowerShell, Command Prompt, Git Bash, MSYS2 and WSL
  distributions, through the Windows pseudo console.
- **Terminal**: tabs with colours, rename, duplicate, close left/right/others, find, copy on select,
  right-click paste with a multi-line warning, colour schemes, font size with Ctrl+plus and Ctrl+minus.
- **Split view**: Layout, at the top right, shows two terminals side by side or above each other, or four.
- **Tiling**: Layout, Tile all tabs, apart from Split, shows every open tab in its own pane: terminals, RDP, VNC, consoles, files and
  tools. A new tab splits the focused pane; drag the borders to resize, drag a pane's header onto another to
  swap them, Ctrl+Alt+arrows to move the focus (with Shift: move the pane), Ctrl+Alt+Enter or double-click
  the header to zoom. Make the focused pane bigger or smaller (also only wider, narrower, taller or lower), give
  all panes equal sizes (or double-click a border for the panes it divides), turn a split, make a pane the main
  one, and choose whether the next tab opens beside or below a pane; all of it is in the menu on each pane
  header (the dots, or right-click). Arrange as a grid, side by side, above each other, or one main pane with
  the others beside it.
- **Assistant** (Ctrl+Shift+A, a column at the right): ask any OpenAI-compatible AI host (LiteLLM, vLLM,
  Ollama, a cloud API) about the tab you are in; the last lines of a terminal go with the question. Passwords
  Cablebag knows and text that looks like a secret are hidden before anything is sent, every exchange is in a
  log (Settings, Assistant). Keys come from the vault or are kept on this PC. In an SSH tab it can work on the
  server: it runs commands that only read by itself and asks before every change (Run it, Always for that kind
  of command on that server, Refuse); dangerous commands always ask, folders you protect ask for everything,
  and sudo gets its password from Cablebag, never from the AI.
- **Send to all**: type once into every connected terminal.
- **Macros**: record what you type, edit steps (text, pause, wait for text), run with a click or a hotkey.
- **Highlighting**: words such as error, warning and ok are coloured; network-device set and your own words.
- **Logging**: every terminal to a file, as clean text, text with timestamps, or raw; file name variables.
- **Save or copy terminal output** from the tab menu, and a check before closing Cablebag with live sessions.
- **Tunnels**: local, remote and dynamic (SOCKS 4/5) port forwards over SSH, with jump hosts, live status,
  start/stop all, and start with Cablebag.
- **Network tools**: network scanner (ping, open service ports, names, hardware addresses, open SSH or
  Remote Desktop from a result), port scanner, open ports on this PC with the owning program, Wake on LAN.
- **SSH key generator**: Ed25519, ECDSA or RSA keys, open existing OpenSSH or PuTTY keys, save with a
  passphrase, and add the public key to a server's authorized_keys in one step.
- **Vaultwarden**: sessions (SSH, RDP, SFTP, VNC) can take their login from a Vaultwarden or Bitwarden-compatible
  vault. Unlocked with the master password when first needed; two-step login (authenticator or email code) with
  "remember this PC"; personal and organization items. The decrypted vault stays in memory only.
- **Password vault**: passwords and key passphrases are encrypted with AES-256-GCM under a key that only your
  Windows account can unwrap (DPAPI).
- **RDP**: the Windows Remote Desktop control inside a tab (helper `cablebag-rdp.exe`), screen follows the tab
  size, clipboard/drive/printer/smart card/port/microphone sharing, sound choice, admin console, CredSSP,
  restricted admin, Remote Credential Guard, Remote Desktop Gateway, SSH gateway, Ctrl+Alt+Del, reconnect.
  Saved passwords come from the vault, or you log in on the remote screen.
- **VNC**: the remote screen in a tab (Tight, ZRLE, CopyRect, Raw), fit to the tab or real size, keyboard and
  mouse, clipboard both ways, Ctrl+Alt+Del, view only, SSH gateway. The password comes from the vault, a saved
  password or a prompt.
- **SFTP**: a session that only opens the file browser, without a terminal.
- **Telnet**: a terminal with the Telnet handshake (window size, terminal type); with a user name it answers the
  login and password prompts by itself, the password from the vault.
- **Serial**: a COM port as a terminal (speed, data bits, parity, stop bits, flow control).
- **FTP**: the same file browser over FTP or FTPS (TLS after connecting or from the start, also for servers that
  require TLS session reuse such as vsftpd).
- **Browser**: a web page in a tab, with back, forward, reload and its own address bar. It only shows http and
  https pages, gets no access to Cablebag, and keeps its cookies apart.
- **File or folder**: opens a file, folder or link in the program Windows uses for it.

## Building

Needs Rust, Node.js, the .NET 8 SDK and the Visual Studio C++ build tools. Running the RDP helper needs
the .NET 8 Desktop Runtime. `rdp-helper/lib/MSTSCLib.dll` (bindings for the Remote Desktop control) is
regenerated with `rdp-helper/tools/make-interop.ps1` in Windows PowerShell 5.1.

```
npm install
npm run tauri dev      # run from source
npm run tauri build    # setup.exe in src-tauri/target/release/bundle/nsis
```

Cablebag stores its database in `%APPDATA%\Cablebag\cablebag.db`.

## Keyboard shortcuts

| Keys | Action |
|---|---|
| Ctrl+Shift+T | New local terminal |
| Ctrl+Shift+W | Close tab |
| Ctrl+Shift+D | Duplicate tab |
| Ctrl+Shift+N | New session |
| Ctrl+Shift+H | Home |
| Ctrl+Shift+E | Show or hide the remote files |
| Ctrl+Shift+A | Show or hide the assistant |
| Ctrl+comma | Settings |
| Ctrl+Shift+plus, Ctrl+Shift+minus | Interface size |
| Ctrl+Alt+arrows | Tiling: focus the pane in that direction (with Shift: move the pane there) |
| Ctrl+Alt+Enter | Tiling: zoom the focused pane, or back |
| Ctrl+Alt+= and Ctrl+Alt+- | Tiling: make the focused pane bigger or smaller |
| Ctrl+Alt+0 | Tiling: equal sizes for all panes |
| Ctrl+Alt+R | Tiling: turn the split of the focused pane (side by side or above each other) |
| Ctrl+Alt+M | Tiling: make the focused pane the main one (swaps it with the largest) |
| Ctrl+K, Ctrl+Shift+K | Search box (in a terminal only Ctrl+Shift+K; Ctrl+K stays with the shell) |
| Ctrl+Tab, Ctrl+Shift+Tab | Next, previous tab |
| Ctrl+Shift+C, Ctrl+Shift+V | Copy, paste in the terminal |
| Ctrl+Shift+F | Find in the terminal |
| Ctrl+plus, Ctrl+minus | Larger, smaller terminal font |
