<picture>
  <source media="(prefers-color-scheme: dark)" srcset="branding/cablebag-logo.svg">
  <img src="branding/cablebag-logo-on-light.svg" alt="Cablebag" height="64">
</picture>

[![Latest release](https://img.shields.io/github/v/release/maferick/cablebag?style=flat&label=release)](https://github.com/maferick/cablebag/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/maferick/cablebag/total?style=flat&label=downloads)](https://github.com/maferick/cablebag/releases)
[![Windows 10 | 11](https://img.shields.io/badge/Windows-10%20%7C%2011-0078d4?style=flat)](#install)
[![Licence: free to use](https://img.shields.io/badge/licence-free%20to%20use-2e8b57?style=flat)](LICENSE)
[![WireGuard: per session](https://img.shields.io/badge/WireGuard-per%20session-88171a?style=flat)](#wireguard-per-session)
[![CI](https://img.shields.io/github/actions/workflow/status/maferick/cablebag/ci.yml?branch=main&style=flat&label=CI)](https://github.com/maferick/cablebag/actions/workflows/ci.yml)

Cablebag is a free remote access app for Windows 10 and 11. It puts SSH, Remote Desktop (RDP), VNC, Telnet,
serial ports, SFTP and FTP, local shells and WSL, web pages and Proxmox consoles in tabs in one window, with
remote files beside every SSH tab, a password vault, session sync, and an assistant that can work on your servers.
It can run WireGuard inside the app, per session, so two customers' VPNs with the same address ranges can be
open at the same time. It is made for system administrators, IT consultants and MSP staff who look after many
customer networks, and for homelab users who want their servers, Proxmox machines and consoles in one place.
It imports your sessions from MobaXterm, PuTTY, KiTTY, WinSCP, the OpenSSH config and `.rdp` files.

![Cablebag with tiled SSH sessions, remote files and the assistant](assets/screenshot.png)

## WireGuard per session

Most VPN clients connect the whole PC. That is a problem when you work for more than one customer: two
networks that both use 10.0.0.0/24 cannot be connected at once, so you switch one VPN off and the other on,
all day long, and a tab you left open for customer A quietly starts talking to customer B's address.

Cablebag runs WireGuard itself, inside the app (userspace: boringtun with a small TCP/IP stack), and only for
the sessions, folders or Proxmox servers that choose a WireGuard network. Each network is its own small network
inside Cablebag. Two VPNs whose machines have exactly the same addresses can be open side by side, in two
tabs, at the same time.

![The WireGuard networks overview: two networks with the same address range, one of them live with its handshake and traffic](assets/wireguard-networks.png)

- **Nothing changes in Windows.** No WireGuard adapter, no routes, no admin rights. The WireGuard app can stay
  installed; Cablebag warns when it sees the app has the same tunnel switched on.
- **Works for every session type that talks to the network:** SSH, SFTP, RDP, VNC, Telnet, and web pages in a
  tab. A web page gets an internal proxy that only that tab uses, so every address, including local ones, goes
  through the tunnel, and each network keeps its own cookies.
- **Works for Proxmox servers:** the API, the machines list, start and stop, snapshots, the console, the node
  shell and the SSH and RDP sessions Cablebag opens for its VMs and containers all go through the network you
  chose for that server. The Proxmox web interface opens in a Cablebag tab through it too.
- **Set it where you use it.** Next to Network in a session's or a folder's editor: Import .conf, New, Edit. A
  folder's network applies to its sessions; a session or subfolder can still go directly. The Tunnels page keeps
  the overview with the live state of every network: open connections, last handshake and data.
- **Keys stay private.** Import the `.conf` you already use, or let Cablebag make a key pair of its own (its
  public key goes on the server as an extra peer, so you can keep the WireGuard app's tunnel as well). Private
  keys live in Cablebag's encrypted vault or come from a Vaultwarden item. Networks travel with Sync; their keys
  do not.

![A session's editor with the Network choice set to a WireGuard network, and the Import .conf, New and Edit buttons next to it](assets/wireguard-session.png)

## Sessions

- **SSH**, with remote files beside the terminal, port forwards (local, remote, SOCKS), jump hosts, agent
  forwarding (Pageant or the Windows OpenSSH agent, per session, off by default), windows of Linux programs
  shown on your PC (X11, with the X server that comes with Cablebag) and automatic reconnect in the same terminal
  when the connection drops.
- **Remote Desktop (RDP)**, sized to its pane, Ctrl+Alt+Del that works, and a clear message when a session ends.
- **VNC**, **Telnet** and **serial ports**.
- **SFTP and FTP** as file sessions.
- **Local shells and WSL** in the same tabs as everything else (Ctrl+Shift+T).
- **Web pages** in a tab, optionally through a WireGuard network.
- **Tabs, split view and tiling.** Split the window for terminals, or tile every open tab and resize, swap, zoom
  and focus panes from the keyboard.
- **Search box (Ctrl+K).** Type a few letters to open any session, tab, Proxmox machine, tool, macro or setting;
  type `user@host` to connect right away; type `>` for the list of commands.
- **Folders that do the work.** A folder can give its sessions a user name, a key file, a login from the vault,
  jump hosts, a tab colour and a WireGuard network. Sessions use them where they have nothing of their own.
- **Run on many servers.** One command on every SSH session in a folder, a few at a time, answers side by side.
  Uptime, disk space, pending updates or failed services on 30 servers in one go.
- **Workspaces and the tabs from last time.** Save the open tabs under a name (everything for one customer) and
  open them again from Home; or let Cablebag reopen what was open when it closed.
- **Import.** MobaXterm (`MobaXterm.ini` or an `.mxtsessions` export), PuTTY and KiTTY saved sessions, WinSCP
  sites, your OpenSSH config file and `.rdp` files. Each source gets its own folder, and importing again updates
  instead of duplicating.

## Files

- **Remote files beside every SSH tab** (Ctrl+Shift+E): browse the server and drag files in from Explorer.
  Each session can choose whether the files open when it connects.
- **Edit in Cablebag.** Double-click a text file and it opens in Cablebag's editor over SFTP, with line numbers
  and Ctrl+S. Before anything is written you see the exact change, removed lines in red and new ones in green.
  The server keeps the old version as `name.bak`, and Cablebag refuses to overwrite a file that changed on the
  server since you opened it.
- **Edit with a Windows program.** Larger or binary files open with the program of your choice and are uploaded
  again when you save. Right-click offers both.

## Proxmox

- **A Proxmox server is a live folder.** Its VMs and containers show with their state and addresses. Double-click
  opens Remote Desktop, SSH or the console; start, shut down and reboot from the tree.
- **Snapshots and tasks.** Take, roll back and delete snapshots (for a VM optionally with memory); see recent
  tasks of a server or a guest with what each one wrote.
- **A shell on the node** over SSH, with your saved session for the node when there is one.
- **Through WireGuard** when you choose a network for the server (Settings, Proxmox), including the sessions
  Cablebag opens for its machines and the Proxmox web interface in a tab.
- Uses an API token. Snapshots need `VM.Snapshot` (and `VM.Snapshot.Rollback`); the assistant inside a VM needs
  `VM.GuestAgent.Unrestricted` on Proxmox 9.

## Vault and passwords

- **Logins from your credential sources**: as many Vaultwarden or Bitwarden vaults as you use (work and home
  side by side, unlocked with Windows Hello), CyberArk (Privilege Cloud or your own PVWA, with single sign-on in a
  Cablebag window, reasons and tickets, check-out and check-in) and Delinea Secret Server. API keys and WireGuard
  private keys can come from there as well.
- **Through CyberArk PSM**: a session or a whole folder can connect over SSH through PSM for SSH, or over Remote
  Desktop through the PSM server, so the target's password never reaches your PC.
- **Passwords kept on this PC** are encrypted under your Windows account and never leave it. They are never
  included in Sync, and the assistant never sees them.
- **SSH key generator** that puts the new key on the server in one step.
- **Backup and restore** of all your data to one file, from About Cablebag.

## Sync

Sessions, folders, folder settings, macros, tunnels, WireGuard networks, credential sources (without their
secrets), workspaces and the assistant's memory sync through a private git repository of your own, encrypted. Passwords and private keys stay out. When the
same item changed on two PCs, Settings, Sync names the conflict and says whose version was kept.

## Assistant

An optional assistant column (Ctrl+Shift+A) that works with any AI host speaking the OpenAI API: LiteLLM,
vLLM, Ollama on your own hardware, or a cloud service. Nothing is configured until you add a host.

- **Ask about what is on screen.** The last lines of the terminal, or a picture of a Remote Desktop or VNC
  screen, go with your question.
- **It investigates by itself** in an SSH tab, or inside a Proxmox VM or container, with commands that only read.
- **Every change waits for you.** A card shows the command, why, how to undo it and what kind of change it is.
  Cablebag classifies the command, not the AI; dangerous ones (deleting folders, formatting disks, rebooting,
  destroying machines) always get a red card. You can let it ask before every change, or only before dangerous
  ones. Protected folders always ask for everything.
- **On a remote desktop it can click, scroll and type**, after your OK per action or per screen.
- **Secrets stay home.** Passwords it knows and anything that looks like a key, token or password hash are
  hidden before text is sent. Root is given from the vault without the AI seeing the password.
- **It remembers** what it learns per server, per folder and in general, inside Cablebag, with Undo and an editor.
- Settings, Assistant lists everything that was sent and what came back.

## Tools

- SSH tunnels drawn step by step, from where they listen to where they arrive.
- A network and port scanner, Wake on LAN, and Run on many servers.
- Macros with keyboard shortcuts.

## Install

Download `Cablebag_<version>_x64-setup.exe` from the [latest release](https://github.com/maferick/cablebag/releases/latest)
and run it. It installs for your Windows account only, no administrator rights needed. Cablebag keeps itself up to
date: updates come from this page and nowhere else, and are installed only when they carry the Cablebag signature.

Needs Windows 10 or 11. Remote Desktop tabs need the
[.NET 8 Desktop Runtime](https://dotnet.microsoft.com/download/dotnet/8.0).

What changed in each version: [CHANGELOG.md](CHANGELOG.md).

## FAQ

**Is it free?** Yes, for private and work use, on as many PCs as you like. It is not open source; see Licence.

**Does it need admin rights?** No. The installer, the app and WireGuard inside it all run under your own
Windows account.

**Does it change my network settings?** No. WireGuard runs inside Cablebag only for the sessions that choose a
network. No adapter is created, no route is added, and nothing else on the PC goes through it.

**Can I use my existing WireGuard config?** Yes, import the `.conf`. Use it in Cablebag or in the WireGuard app,
not both at once; or make a key pair of Cablebag's own and add it to the server as a second peer.

**Can I import from MobaXterm, PuTTY or WinSCP?** Yes: MobaXterm, PuTTY, KiTTY, WinSCP, the OpenSSH config and
`.rdp` files. Cablebag is not a fork of any of them.

**Where are my passwords kept?** Either in your credential sources (Vaultwarden, CyberArk, Secret Server), or on
this PC, encrypted under your Windows account. They never go into Sync and are hidden from the assistant.

**Does it work on Linux or macOS?** No, Windows 10 and 11 only.

**Where does the assistant send my data?** Only to the AI host you configured yourself, after secrets are
removed. Without a host it does nothing.

## Licence

Cablebag is free to use, but it is **not open source**: you may install and use it, but not share copies, change it
or build on its code. The source is published so you can see what it does. See [LICENSE](LICENSE); the open-source
components inside are listed in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

<details>
<summary>Keyboard shortcuts</summary>

| Keys | Action |
|---|---|
| Ctrl+K | Search box (inside a terminal: Ctrl+Shift+K) |
| Ctrl+Shift+T | New local terminal |
| Ctrl+Shift+N | New session |
| Ctrl+Shift+W | Close tab |
| Ctrl+Shift+D | Open the same session again in a new tab |
| Ctrl+Shift+H | Home |
| Ctrl+Tab, Ctrl+Shift+Tab | Next, previous tab |
| Ctrl+Shift+E | Show or hide the remote files |
| Ctrl+Shift+A | Show or hide the assistant |
| Ctrl+Alt+arrows | Tiling: focus the pane in that direction (with Shift: move the pane there) |
| Ctrl+Alt+Enter | Tiling: let the pane fill the area, or back |
| Ctrl+Shift+C, Ctrl+Shift+V | Copy, paste in the terminal |
| Ctrl+Shift+F | Find in the terminal |
| Ctrl+comma | Settings |

All shortcuts, also for tiling, are in Settings, Keyboard shortcuts; in the search box, `>` lists the commands
with their keys.

</details>

<details>
<summary>Building from source</summary>

Building is for reading and checking the code; the licence does not allow changed or redistributed builds.
Needs Rust, Node.js, the .NET 8 SDK and the Visual Studio C++ build tools. Running the Remote Desktop helper
needs the .NET 8 Desktop Runtime.

```
npm install
npm run tauri dev      # run from source
npm run tauri build    # setup.exe in src-tauri/target/release/bundle/nsis
```

</details>
