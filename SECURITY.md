# Security

Cablebag holds the keys to other people's servers, so a security problem in it matters more than most bugs.
If you think you have found one, please tell me privately first.

## How to report

Use GitHub's private vulnerability reporting: open the
[Security tab](https://github.com/maferick/cablebag/security) of this repository and choose
**Report a vulnerability**. Only you and the maintainer can read what you write there. There is no e-mail
address for reports.

Please do not open a public issue for a security problem, and do not post exploit details anywhere until a
fixed release is out.

## What is in scope

Anything in this repository or in a release built from it:

- the app itself (the Rust backend in `src-tauri/`, the Svelte frontend in `src/`);
- the installer and the update mechanism, including the signature check on updates;
- the vault and the way passwords, keys and tokens are stored, encrypted and unlocked;
- the assistant's redaction (secrets that should be hidden before text goes to an AI host);
- the WireGuard implementation inside the app (boringtun with a userspace TCP/IP stack);
- the Remote Desktop helper (`rdp-helper/`).

Out of scope: the servers, VPNs and AI hosts you connect Cablebag to, the open-source components it contains
(report those to their own projects, though a note here is welcome so the dependency can be updated), and
anything that needs a user to run untrusted software on their own PC first.

## What to include

- Which Cablebag version (About Cablebag has **Copy details**) and which Windows version.
- What the problem is and why it matters: what an attacker could do, and from where (a hostile server, a
  hostile network, another user on the same PC, a crafted file or import).
- Steps to reproduce it, or a proof of concept. Please keep it as small as it can be.
- If you already know where in the code it lives, say so.

Please do not include real passwords, private keys, tokens or the names of hosts that are not yours to share.

## What happens next

I read reports as soon as I can and reply through the same private channel. There is no bug bounty and no
guaranteed response time; this is a small project looked after in spare time.

Fixes ship as a new release. Installed copies pick it up through Cablebag's updater, which only installs an
update that carries the Cablebag signature. When a fix is out, the release notes say that a security problem
was fixed, and the reporter is credited there if they want to be.

## Supported versions

Only the latest release is supported. Fixes are not backported; please update.
