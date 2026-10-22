<picture>
  <source media="(prefers-color-scheme: dark)" srcset="branding/cablebag-logo.svg">
  <img src="branding/cablebag-logo-on-light.svg" alt="Cablebag" height="64">
</picture>

Cablebag is a remote access workstation for Windows: SSH, Remote Desktop, VNC, Telnet, serial ports, SFTP and FTP,
local shells, Proxmox consoles and an AI assistant, all as tabs in one window.

![Cablebag with tiled SSH sessions, remote files and the assistant](assets/screenshot.png)

## Highlights

- **Everything in one window.** Tabs for every kind of session, a split view for terminals, or tile all open tabs
  and resize, swap and zoom the panes from the keyboard.
- **Find anything by typing.** The search box (Ctrl+K) finds any session, open tab, Proxmox machine, tool, macro or
  setting; type `user@host` to connect right away.
- **Remote files beside every SSH tab.** Browse, drag files in from Explorer, edit a file with a Windows program and
  it is uploaded again when you save.
- **Proxmox servers as live folders.** VMs and containers show with their state and addresses; double-click opens
  Remote Desktop, SSH or the console, and you can start, shut down and reboot from the tree.
- **Logins from Vaultwarden**, unlocked with Windows Hello. Passwords kept on this PC are encrypted under your
  Windows account and never leave it.
- **Your sessions follow you.** Import from MobaXterm, PuTTY, KiTTY, WinSCP, the OpenSSH config and `.rdp` files; sync sessions,
  folders, macros and tunnels through a private git repository, encrypted, passwords never included.
- **An assistant that works on your servers.** Ask about what is on screen; in an SSH tab it investigates by itself,
  and on a Remote Desktop or VNC screen it looks, clicks and types. It asks before every change, remembers what it
  learns per server, and hides passwords and secrets before anything is sent to the AI.
- **Tunnels and network tools.** SSH port forwards (local, remote, SOCKS), a network and port scanner, Wake on LAN
  and an SSH key generator that puts the key on the server in one step.

## Install

Download `Cablebag_<version>_x64-setup.exe` from the [latest release](https://github.com/maferick/cablebag/releases/latest)
and run it. It installs for your Windows account only, no administrator rights needed. Cablebag keeps itself up to
date: updates come from this page and nowhere else, and are installed only when they carry the Cablebag signature.

Needs Windows 10 or 11. Remote Desktop tabs need the
[.NET 8 Desktop Runtime](https://dotnet.microsoft.com/download/dotnet/8.0).

What changed in each version: [CHANGELOG.md](CHANGELOG.md).

## Licence

Cablebag is free to use, but it is **not open source**: you may install and use it, but not share copies, change it
or build on its code. The source is published so you can see what it does. See [LICENSE](LICENSE); the open-source
components inside are listed in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

## Keyboard shortcuts

| Keys | Action |
|---|---|
| Ctrl+K | Search box (inside a terminal: Ctrl+Shift+K) |
| Ctrl+Shift+T | New local terminal |
| Ctrl+Shift+N | New session |
| Ctrl+Shift+W | Close tab |
| Ctrl+Tab, Ctrl+Shift+Tab | Next, previous tab |
| Ctrl+Shift+E | Show or hide the remote files |
| Ctrl+Shift+A | Show or hide the assistant |
| Ctrl+Alt+arrows | Tiling: focus the pane in that direction (with Shift: move the pane there) |
| Ctrl+Shift+C, Ctrl+Shift+V | Copy, paste in the terminal |
| Ctrl+comma | Settings |

Every shortcut is listed in the app: open the search box and type `>`.

## Building from source

Needs Rust, Node.js, the .NET 8 SDK and the Visual Studio C++ build tools. Running the Remote Desktop helper needs
the .NET 8 Desktop Runtime.

```
npm install
npm run tauri dev      # run from source
npm run tauri build    # setup.exe in src-tauri/target/release/bundle/nsis
```
