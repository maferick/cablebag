# Code signing with SignPath

Windows releases can be signed through the [SignPath Foundation](https://signpath.org), which offers this free to
open-source projects that are already well known. [release.yml](../.github/workflows/release.yml) already has the
steps; they stay off until the repository variable below is set. The project's
[code signing policy](../README.md#code-signing-policy) says who does what.

The first application (November 2026) was turned down because the project was still too new and little known:
the Foundation looks for outside signals such as stars, forks, contributors, articles and discussions elsewhere.
Apply again at signpath.org/apply once there are more of those. A paid SignPath subscription would also work
with these steps, but its certificate carries the subscriber's own name.

## Setting it up, once SignPath has approved the project

1. On signpath.io, make a project with the slug `cablebag`, and link it to this repository as a GitHub trusted
   build system.
2. Add two artifact configurations with the slugs `programs` and `installer`, from the files in
   [artifact-configurations](artifact-configurations).
3. Make a signing policy with the slug `release-signing` that needs your approval.
4. Make an API token for a user who may submit signing requests.
5. In this repository's settings, under Secrets and variables, Actions:
   - add the secret `SIGNPATH_API_TOKEN`, with that token;
   - add the variable `SIGNPATH_ORGANIZATION_ID`, with your organisation's id on signpath.io.

## What a release does then

`scripts/release.mjs` works as before, but GitHub's build stops twice to wait for your approval on signpath.io:
first for `cablebag.exe` and `cablebag-rdp.exe`, then for the installer that holds them. Each waits up to an hour.
The update signature is made last, because it has to match the signed installer.

The X server in the `vcxsrv` folder is someone else's program, shipped unchanged and not signed by this project.

## After the first signed release

Change the code signing policy in the README to say releases are signed now, and remove the sentence in
its Install section that says the installer is not signed.
