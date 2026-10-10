# Publishing the pline.ai MCP server and skill

This runbook covers every channel where the MCP server and the skill are, or should be, listed: npm,
the official MCP Registry, Anthropic's Claude directory, Cursor and VS Code, Glama, the `skills` CLI,
and the community GitHub lists. It records what the repository automates, what a maintainer does by
hand, which credentials each step needs, and the prerequisites that gate everything.

## Prerequisites that gate every channel

| Prerequisite | Why | Status |
|---|---|---|
| Public repository | The MCP Registry resolves `repository.url`; npm provenance attestations need a public repo; the Claude directory requires a public GitHub repository before a plugin bundle goes live; awesome lists, Glama, and `npx skills add` only read public repositories; GHCR images pushed from a private repository are private; the `ubuntu-*-arm` runners used for Linux arm64 and the arm64 image are free only for public repositories. | **Done.** `grepsr/pline-api` is public. |
| Open-source license | The Claude directory blocks a plugin without a `LICENSE` file or `license` in `plugin.json`; npm warns without one; awesome-list maintainers and crates.io require one. | **Done.** MIT, in `LICENSE`, `mcp-server/Cargo.toml`, `npm/package.json`, `.claude-plugin/plugin.json`, and the README. |
| Public GHCR package | The registry validates the OCI package anonymously. | Verify `pline-mcp` is public before publishing the MCP Registry entry. |
| npm package names | Launcher: `pline-api`; platform packages are scoped as `@pline/api-<os>-<cpu>` because npm's spam detection rejects new unscoped `*-win32-x64` names. | The `pline` npm org exists; `NPM_TOKEN` must have publish rights on it for the first scoped publish. |
| A new version tag | Tags are immutable release inputs; `v0.3.1` already exists with the unscoped platform package names, and its npm publish stopped at the Windows package. | Use the next version, `0.3.2`, after the version bump reaches `main`. |

## Credentials and secrets

Nothing in the repository needs a long-lived secret once trusted publishing is set up. The table
lists what each step authenticates with and who sets it up.

| Step | Auth | Secret in GitHub? | One-time setup |
|---|---|---|---|
| GitHub Release, binaries, SHA256SUMS | `GITHUB_TOKEN` (`contents: write`) | No | None |
| GHCR image push | `GITHUB_TOKEN` (`packages: write`) | No | Set the package public once |
| npm publish, steady state | npm **trusted publishing**: the workflow's GitHub OIDC token (`id-token: write`) is exchanged for a short-lived npm credential; provenance is attached automatically on a public repo | No | On npmjs.com, for **each** of the six packages: **Package settings → Trusted publisher → GitHub Actions**, organization `grepsr`, repository `pline-api`, workflow filename `publish.yml`, environment `npm-publish` (the `npm` job declares it, so it must match exactly). Requires npm CLI 11.5.1+ (the workflow uses Node 24). |
| npm publish, first time | A package must exist before a trusted publisher can be configured on it, so the first version of each package is published with a token | `NPM_TOKEN`, temporarily | Create a **granular access token** on npmjs.com with **Read and write** on packages, **Bypass 2FA** enabled, and the shortest expiry that covers the release. Store it as an **Environment secret** named `NPM_TOKEN` on the `npm-publish` environment (repo → Settings → Environments); the workflow passes it as `NODE_AUTH_TOKEN`. After the first publish, configure trusted publishers and delete the token and the secret. Alternatively run `npm publish` locally once from `npm/` and `npm/dist/*` while logged in. |
| MCP Registry | `mcp-publisher login github-oidc`: GitHub OIDC grants the `io.github.grepsr/*` namespace to workflows in this organization | No | None. Switching to an `ai.pline/...` name instead needs DNS auth: publish an Ed25519 public key in a TXT record on `pline.ai`, store the private key as `MCP_PRIVATE_KEY`, change `name` in `server.json`, `mcpName` in `npm/package.json`, and the Dockerfile label together, and change the login step to `mcp-publisher login dns --domain pline.ai`. |
| Claude directory | Interactive, from a claude.ai account on a Pro, Max, Team, or Enterprise plan | No | None |
| Glama, awesome lists, mcpservers.org | A GitHub account for PRs, an email for the form | No | None |
| crates.io (optional, not wired up) | `CARGO_REGISTRY_TOKEN`, or crates.io trusted publishing | Only if you add the `cargo` package type | Needs `license` in `Cargo.toml` |

The `npm` job already declares `environment: npm-publish`; GitHub creates the environment unprotected on
first use, so configure it before the first release: Settings → Environments → `npm-publish` → add the
`NPM_TOKEN` secret, restrict **Deployment branches and tags** to `v*`, and add a required reviewer.
Anyone who can push a tag can otherwise run the publish job with the token. The same hardening applies to `MCP_PRIVATE_KEY` if DNS auth is
adopted. GitHub OIDC tokens need no such care: they exist only for the duration of the job.

## Release flow (one tag drives everything)

1. Bump the version with `python3 tests/check_versions.py --set X.Y.Z`. It rewrites
   `mcp-server/Cargo.toml` and `Cargo.lock`, `server.json` (top level, every `packages[].version`, the
   OCI image tag), `npm/package.json` (`version` and all five `optionalDependencies`),
   `.claude-plugin/plugin.json` (`version` and the `pline-api@<version>` pin),
   `.claude-plugin/marketplace.json`, and the release links, archive names, and image tags in both
   READMEs, then verifies. Without `--set` the same script runs in CI and fails on any mismatch.
2. Push the tag `v<version>`. [release.yml](.github/workflows/release.yml) then:
   - builds and smoke-tests the five platform binaries (macOS arm64/x64, Linux x64/arm64, Windows x64);
   - builds the Docker image natively on amd64 and arm64 runners, pushes both by digest to
     `ghcr.io/grepsr/pline-mcp`, stitches them into the `<version>` and `latest` tags, and verifies the
     `io.modelcontextprotocol.server.name` label equals the `name` in `server.json`;
   - creates a **draft** GitHub Release with the archives and `SHA256SUMS`.
3. Review and publish the draft release.
4. Publishing the release fires [publish.yml](.github/workflows/publish.yml):
   - **npm**: downloads the release archives, assembles the five platform packages with
     [`npm/build-platform-packages.py`](npm/build-platform-packages.py), publishes each (skipping
     versions already on npm), then publishes the `pline-api` launcher, and finally runs
     `npx -y pline-api@<version> --help` to prove the published package works;
   - **registry**: checks that `pline-api@<version>` on npm carries `mcpName` and that the image is
     publicly pullable with the right label, validates `server.json`, logs in with GitHub OIDC, and
     publishes `io.github.grepsr/pline-api`.
   Re-run it from the Actions tab with the tag as input if it needs repeating; tick **skip_npm** to
   republish only the registry entry. If the registry rejects the tag's `server.json`, fix it on
   `main` and set **server_ref** to `main`: the registry stage then reads `server.json` from that ref
   (its versions must still match the tag) without a new release.

The README's one-click Cursor and VS Code buttons run `npx -y pline-api` unpinned, so they do not
change between releases. Regenerate them only if the command or environment variables change:

```sh
python3 - <<'EOF'
import base64, json
from urllib.parse import quote
cursor = {"command": "npx", "args": ["-y", "pline-api"], "env": {"PLINE_BASE_URL": "https://apix.pline.ai/v1", "PLINE_API_KEY": "your-own-api-key"}}
print("cursor config:", base64.b64encode(json.dumps(cursor, separators=(",", ":")).encode()).decode())
vscode = {"name": "pline.ai", "command": "npx", "args": ["-y", "pline-api"],
          "env": {"PLINE_BASE_URL": "${input:pline-base-url}", "PLINE_API_KEY": "${input:pline-api-key}"},
          "inputs": [{"id": "pline-base-url", "type": "promptString", "description": "pline.ai API base URL", "default": "https://apix.pline.ai/v1"},
                     {"id": "pline-api-key", "type": "promptString", "description": "pline.ai API key", "password": True}]}
print("vscode query:", quote(json.dumps(vscode, separators=(",", ":")), safe=""))
EOF
```

## 1. npm (`npx -y pline-api`)

- **Layout:** [`npm/`](npm) holds the `pline-api` launcher (`bin/pline-mcp.js`, README, package.json
  with `mcpName`). It resolves the binary from `PLINE_MCP_BINARY`, then from the platform package
  installed through `optionalDependencies` (`@pline/api-darwin-arm64`, `-darwin-x64`, `-linux-x64`,
  `-linux-arm64`, `-win32-x64`), then from a checksum-verified download of the GitHub Release archive
  cached under `~/.cache/pline-mcp/<version>/`. Platform packages are generated in CI, never committed.
- **Testing locally:** `cargo build --manifest-path mcp-server/Cargo.toml --locked`, then
  `PLINE_MCP_BINARY=$PWD/mcp-server/target/debug/pline-mcp python3 tests/smoke.py npm/bin/pline-mcp.js`.
  CI runs the same, plus `npm pack --dry-run`.
- **Publishing:** the npm stage of `publish.yml`, see above. All six packages share one version.
- **Check a release:** `npm view pline-api@<version>` and `npx -y pline-api@<version> --help`.

## 2. Official MCP Registry

- **What is published:** [`server.json`](server.json), name `io.github.grepsr/pline-api`, two packages:
  the `npm` package `pline-api` (run with `npx`) and the `oci` image `ghcr.io/grepsr/pline-mcp:<version>`,
  both over stdio with `PLINE_BASE_URL` and `PLINE_API_KEY` (the key marked secret).
- **Ownership proof:** `mcpName` in `npm/package.json` and the Dockerfile `LABEL
  io.modelcontextprotocol.server.name` must both equal the `name` in `server.json`; CI and the
  publish workflow check both. The registry rejects the publish if either artifact is missing or private.
- **Optional extra packages** (add only once the artifact exists, because the registry validates
  every package):
  - `cargo`: publish `pline-mcp` to crates.io and add a visible `mcp-name: io.github.grepsr/pline-api`
    line to `mcp-server/README.md`; crates.io strips HTML comments, so it must be plain text.
  - `mcpb`: zip a prebuilt binary with an MCPB `manifest.json`, attach it to the GitHub Release, and
    record its SHA-256 in `fileSha256`.
  - `remotes`: if the operator hosts the Streamable HTTP mode publicly, add
    `{"type": "streamable-http", "url": "https://.../mcp"}`.
- **Check a listing:**

  ```sh
  curl -s "https://registry.modelcontextprotocol.io/v0.1/servers?search=io.github.grepsr/pline-api"
  ```

- **Review and timing:** none. Publishing is immediate once validation passes. Aggregators such as
  PulseMCP and Glama ingest the registry automatically.

## 3. Claude directory (claude.ai, Claude Desktop, Cowork, Claude Code)

The directory accepts two listing kinds. Desktop extensions (MCPB) are no longer accepted, so a local
server is distributed as a **plugin bundle**.

### Plugin bundle (ready in this repository)

- [`.claude-plugin/plugin.json`](.claude-plugin/plugin.json) makes the repository root a Claude Code
  plugin named `pline-ai`: the `skills/pline-api` skill loads everywhere, and the `pline.ai` MCP
  server runs `npx -y pline-api@<version>` (pinned, as the directory requires) with the API URL and
  key collected through `userConfig`; the key goes to secure storage, never to settings files.
- [`.claude-plugin/marketplace.json`](.claude-plugin/marketplace.json) lets anyone install today,
  without the directory:

  ```sh
  claude plugin marketplace add grepsr/pline-api
  claude plugin install pline-ai@pline-ai
  ```

- Validate before submitting: `claude plugin validate .` (add `--strict` once CI uses Claude Code
  2.1.281 or newer, which recognises the directory listing fields).
- Submit at [claude.ai/directory/manage](https://claude.ai/directory/manage) from a Pro, Max, Team, or
  Enterprise account: **Submit new → Plugin bundle**, enter `grepsr/pline-api` with the plugin at the
  repository root, select **Validate**, fix anything marked **Blocking**, then **Submit for review**.
  The first organization to submit a repository owns the listing, so submit from the Grepsr
  organization account.
- Expect these review notes, which are holds rather than blocks: a pinned `npx` package is always
  held for a reviewer because its dependencies resolve at install time, and the root `.mcp.json`
  reads `PLINE_API_KEY` from the environment for source development (the plugin manifest overrides it
  with `userConfig`). `privacyPolicyUrl` and `termsOfServiceUrl` point at pline.ai's legal pages; add an `icon` PNG to
  `plugin.json` before submitting.
- After approval, every commit on the tracked branch is re-scanned and published automatically.
  Raise `version` and the `pline-api@<version>` pin in `plugin.json` with each release.

### MCP connector (needs a hosted endpoint)

If the operator hosts the Streamable HTTP mode on a public URL, submit it as an **MCP connector** too:
**Submit new → MCP connector**, enter the `/mcp` URL. Anthropic probes it anonymously, so it must
answer `initialize` and `tools/list` or reply `401` with a `WWW-Authenticate` header that leads to an
OAuth flow with dynamic client registration. The current `/mcp` route accepts an API key header, which
the directory cannot supply, so an OAuth front door is required first. Pairing the connector with the
plugin bundle gives one set of tools to people who install both.

## 4. Cursor and VS Code

- [`.cursor/mcp.json`](.cursor/mcp.json) and [`.vscode/mcp.json`](.vscode/mcp.json) are picked up when
  the repository is opened as a workspace. Both run `npx -y pline-api`; Cursor reads
  `PLINE_BASE_URL`/`PLINE_API_KEY` from the environment, VS Code prompts for them (the key is masked).
- The README carries one-click **Add to Cursor** and **Install in VS Code** buttons with the same
  command, so users need Node.js but not Rust or Docker.
- Cursor has no submission form; its marketplace reads community indices and the official registry.

## 5. Skill distribution (`npx skills add`)

The [`skills`](https://skills.sh) CLI installs skills from any public GitHub repository into the
folders of 70+ agents. It discovers `skills/pline-api/SKILL.md` directly and also reads
`.claude-plugin/marketplace.json`, so once the repository is public:

```sh
npx skills add grepsr/pline-api            # prompts for agents; -g for user-wide, -y to accept
npx skills add grepsr/pline-api -a cursor  # one agent
npx skills add grepsr/pline-api --list     # show what it found
```

skills.sh lists repositories automatically from install activity; there is no submission form.

## 6. Glama

Glama indexes public GitHub repositories automatically and ingests the official registry.
[`glama.json`](glama.json) at the repository root names the GitHub accounts allowed to claim and
edit the listing; add maintainers there. After the repository is public, claim the listing at
`https://glama.ai/mcp/servers/grepsr/pline-api` (sign in with GitHub). Glama's scorecard rewards a
license, a README with install steps, and tests, all of which this repository has.

## 7. Community lists and aggregators

Open these only after the repository is public and licensed. Each is a pull request or form filled in
by a maintainer; none are automated here.

### punkpeye/awesome-mcp-servers (pull request)

Fork, add the line under **🔎 Search & Data Extraction** (or **📂 Browser Automation**, where other
scrape/crawl/map servers appear) keeping alphabetical order by repository, and open a PR titled
`Add grepsr/pline-api`. The legend markers: 🎖️ official, 🦀 Rust, ☁️ cloud service (the scraping
runs on pline.ai), and the three OS badges because the server runs on all three.

```markdown
- [grepsr/pline-api](https://github.com/grepsr/pline-api) 🎖️ 🦀 ☁️ 🍎 🪟 🐧 - Scrape, batch-scrape, crawl, map and search the web through the pline.ai API, returning Markdown, HTML, screenshots or structured JSON with session reuse and background jobs.
```

Typical turnaround is a few days to two weeks.

### wong2/awesome-mcp-servers and mcpservers.org (form)

This list does not accept pull requests. Submit at <https://mcpservers.org/submit> with:
name `pline.ai`, category **Web Scraping**, the one-sentence description above, repository
`https://github.com/grepsr/pline-api`, official registry name `io.github.grepsr/pline-api`, remote
connections unchecked (until a hosted endpoint exists), and a contact email. The free tier reviews
within about two weeks.

### modelcontextprotocol/servers (pull request)

The official repository's README lists third-party servers under **🎖️ Official Integrations** for
servers maintained by the company that owns the service. Add one alphabetical line in the same
format as the others:

```markdown
- <img height="12" width="12" src="https://pline.ai/favicon.ico" alt="pline.ai Logo" /> **[pline.ai](https://github.com/grepsr/pline-api)** - Scrape, crawl, map, batch-scrape and search the web through the pline.ai API, with Markdown, HTML, screenshot and structured JSON output.
```

### PulseMCP

Submissions are paused (September 2026) while PulseMCP rebuilds its pipeline; it ingests the official
registry automatically when they resume, so the registry publish above covers it.

## Status checklist

| Channel | Mechanism | Done in repo | Maintainer action |
|---|---|---|---|
| npm `pline-api` + platform packages | `npm/` + `publish.yml` npm stage | Yes | First publish with `NPM_TOKEN`, then configure trusted publishers and drop the token |
| MCP Registry | `server.json` + `publish.yml` registry stage (OIDC) | Yes | Make repo and GHCR package public, then publish a release |
| GHCR image | `release.yml` image jobs | Yes | Set package visibility to public once |
| Claude directory, plugin bundle | `.claude-plugin/` | Yes | Submit at claude.ai/directory/manage |
| Claude directory, MCP connector | Hosted `/mcp` with OAuth | No | Needs a hosted endpoint and OAuth |
| Cursor / VS Code | Workspace configs + README buttons | Yes | None |
| Skill via `npx skills add` | `skills/pline-api/` + marketplace.json | Yes | Make the repo public |
| Glama | `glama.json` + auto-index | Yes | Claim the listing after going public |
| awesome-mcp-servers | PR | Entry drafted above | Open the PR |
| mcpservers.org | Form | Values drafted above | Submit the form |
| modelcontextprotocol/servers | PR | Entry drafted above | Open the PR |
