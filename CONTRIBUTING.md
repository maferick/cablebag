# Contributing to Cablebag

Thank you for taking an interest. Cablebag is a small project, free to use but not open source, and this
page explains how bug reports, ideas and code reach it. Please read the section on the licence before you
open a pull request; it is short.

## Reporting a bug

Open an issue with the **Bug report** form. It asks for the Cablebag version (About Cablebag has
**Copy details**, which copies the version and build information), your Windows version, the kind of session
it concerns, what happened, what you expected and how to make it happen again.

Please do not paste passwords, private keys, tokens, or hostnames and addresses you want to keep private.
Replace them with placeholders such as `server1` or `10.0.0.x`.

Security problems go through the Security tab, not through an issue: see [SECURITY.md](SECURITY.md).

## Suggesting a feature

Open an issue with the **Feature request** form. Say what you are trying to do and where Cablebag gets in the
way, rather than only the change you have in mind: there may be a better way to solve it, or it may already
exist under another name. Small, well-described requests are the easiest to take on.

## How pull requests land

This repository is where Cablebag is developed, so pull requests work the normal way: fork, branch from
`main`, open the pull request, and it is reviewed and merged here by the maintainer or a trusted
collaborator. Your commits keep you as their author.

Releases are made from `main` by the maintainer: `scripts/release.mjs` sets the version, tags, builds the
installer, signs it with the update key (which never leaves the maintainer's PC) and creates the GitHub
release. A merged change ships in the next release, and the release notes say what changed.

A few practical points:

- For anything larger than a small fix, open an issue first so we can agree on the approach before you spend
  time on it.
- Keep a pull request to one change. Several small ones are easier to review and merge than one large one.
- Make sure the checks below pass; CI runs the same ones on every pull request.
- Write commit messages that say what changed and why, in plain language.

## Your contribution and the licence

Cablebag's [licence](LICENSE) is "use only": people may use it, but not redistribute or change it. For your
work to be part of Cablebag, you therefore need to give permission, and this clause is that permission:

> By opening a pull request, you agree that your contribution may be included in Cablebag and distributed
> under Cablebag's licence, as it is now and as it may change later, and you confirm that you wrote it or
> otherwise have the right to give it on these terms. You keep the copyright on your own work and remain free
> to use it elsewhere.

If you cannot agree to that (for example because your employer owns what you write), please say so in the
pull request and we will see what is possible.

## Building from source

The licence allows building for reading and checking the code, and for working on a contribution; it does not
allow changed or redistributed builds.

You need:

- [Rust](https://rustup.rs/) (stable);
- [Node.js](https://nodejs.org/) (LTS);
- the [.NET 8 SDK](https://dotnet.microsoft.com/download/dotnet/8.0), for the Remote Desktop helper;
- the Visual Studio C++ build tools (the "Desktop development with C++" workload), which Rust needs on Windows.

Then, in the repository folder:

```
npm install
npm run build:rdp      # builds the Remote Desktop helper once; the app build expects it to exist
npm run tauri dev      # runs Cablebag from source
```

`npm run tauri build` makes the installer in `src-tauri/target/release/bundle/nsis`. It is unsigned, so an
installed copy built this way will not accept updates from it, and that is intended.

## Running the checks

Before opening a pull request, please run the same checks as CI:

```
npx svelte-check --threshold error       # frontend types and Svelte components
cd src-tauri && cargo test --lib          # backend unit tests
```

The CI workflow (`.github/workflows/ci.yml`) runs both on Windows and on Linux. The Linux job only checks that
the non-Windows parts build and their tests pass; Cablebag itself is a Windows app.

## House style

Cablebag's rules for text that users see, in the app and in documents:

- **Plain human language.** Write as you would explain it to a colleague across the desk. Say what happened
  and what to do, not what the code did. "The server closed the connection" rather than "EOF on socket".
- **British spelling**: licence, colour, organisation, centre.
- **No em-dash characters** (U+2014) anywhere, not in the UI, the documents or the code comments. Use a
  comma, a colon, a full stop or brackets instead.
- **No codes or jargon in the UI.** No error codes, enum names, internal identifiers or abbreviations that only
  make sense with the source open. Settings, buttons and messages use the words a user would use.
- **No emoji** in the app or the documents.
- The same goes for commit messages, issue replies and changelog entries.

For the code itself: follow what is already there (the backend is Rust, the frontend Svelte 5 with
TypeScript), and keep Windows-only code behind `cfg(windows)` so the Linux build keeps working.
