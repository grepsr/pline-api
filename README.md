# pline.ai API — Agent skill and MCP

Use the hosted pline.ai API from an agent skill or an MCP client. Both integrations use your own
API key and call the same hosted scraping API. The scraping backend is not part of this repository.

| Integration | Contents | Requirements |
|---|---|---|
| [pline-api skill](skills/pline-api/SKILL.md) | Agent instructions, API reference, Python CLI for scrape, batch, crawl, map, and search | Python 3.10+; no pip dependencies |
| [MCP server](mcp-server/README.md) | Ten tools over local stdio; embedded usage guide and API reference | `npx -y pline-api` (Node.js 18+); also a Docker image, prebuilt binaries, or a Cargo source build |

For remote agents, the MCP server also supports Streamable HTTP with Pline OAuth. Users sign in,
choose a workspace API key, and connect at the hosted `/mcp` URL; see
[`mcp-server/docs/oauth-hosting.md`](mcp-server/docs/oauth-hosting.md) for setup.

## Quick install

The two integrations are installed separately; use either or both.

**Agent skill** (instructions plus a Python CLI the agent runs):

```sh
npx skills add grepsr/pline-api
```

This copies [`skills/pline-api`](skills/pline-api/SKILL.md) from this GitHub repository into your
agent's skills folder (Claude Code, Cursor, Codex, and more). `skills` is the
[skills.sh](https://skills.sh) installer, not a pline package; the skill is not published to npm.
Running it needs Python 3.10+ and the `PLINE_BASE_URL`/`PLINE_API_KEY` variables; see
[Skill setup](#skill-setup).

**MCP server** (ten tools for any MCP client):

```sh
npx -y pline-api
```

You do not run this yourself: add it to your MCP client's configuration below and the client starts
it. It needs Node.js 18+ (or Docker).

The MCP server is the [`pline-api`](https://www.npmjs.com/package/pline-api) npm package, listed in the
[official MCP Registry](https://registry.modelcontextprotocol.io/v0.1/servers?search=io.github.grepsr/pline-api)
as `io.github.grepsr/pline-api`, and also shipped as the `ghcr.io/grepsr/pline-mcp` image.
Every MCP client takes the same configuration:

```json
{
  "mcpServers": {
    "pline.ai": {
      "command": "npx",
      "args": ["-y", "pline-api"],
      "env": {
        "PLINE_BASE_URL": "https://apix.pline.ai/v1",
        "PLINE_API_KEY": "your-own-api-key"
      }
    }
  }
}
```

| Client | One step |
|---|---|
| Claude Code | `claude mcp add pline.ai -e PLINE_BASE_URL=https://apix.pline.ai/v1 -e PLINE_API_KEY=your-own-api-key -- npx -y pline-api` |
| Claude Code plugin (skill + server) | `claude plugin marketplace add grepsr/pline-api` then `claude plugin install pline-ai@pline-ai`; Claude Code prompts for the URL and key |
| Cursor | [![Add pline.ai to Cursor](https://cursor.com/deeplink/mcp-install-dark.svg)](https://cursor.com/install-mcp?name=pline.ai&config=eyJjb21tYW5kIjoibnB4IiwiYXJncyI6WyIteSIsInBsaW5lLWFwaSJdLCJlbnYiOnsiUExJTkVfQkFTRV9VUkwiOiJodHRwczovL2FwaXgucGxpbmUuYWkvdjEiLCJQTElORV9BUElfS0VZIjoieW91ci1vd24tYXBpLWtleSJ9fQ==) then replace `your-own-api-key` in Cursor's MCP settings |
| VS Code | [![Install pline.ai in VS Code](https://img.shields.io/badge/VS_Code-Install_Server-0098FF?style=flat-square&logo=visualstudiocode&logoColor=white)](https://vscode.dev/redirect?url=vscode:mcp/install?%7B%22name%22%3A%22pline.ai%22%2C%22command%22%3A%22npx%22%2C%22args%22%3A%5B%22-y%22%2C%22pline-api%22%5D%2C%22env%22%3A%7B%22PLINE_BASE_URL%22%3A%22%24%7Binput%3Apline-base-url%7D%22%2C%22PLINE_API_KEY%22%3A%22%24%7Binput%3Apline-api-key%7D%22%7D%2C%22inputs%22%3A%5B%7B%22id%22%3A%22pline-base-url%22%2C%22type%22%3A%22promptString%22%2C%22description%22%3A%22pline.ai%20API%20base%20URL%22%2C%22default%22%3A%22https%3A%2F%2Fapix.pline.ai%2Fv1%22%7D%2C%7B%22id%22%3A%22pline-api-key%22%2C%22type%22%3A%22promptString%22%2C%22description%22%3A%22pline.ai%20API%20key%22%2C%22password%22%3Atrue%7D%5D%7D) [![Install pline.ai in VS Code Insiders](https://img.shields.io/badge/VS_Code_Insiders-Install_Server-24bfa5?style=flat-square&logo=visualstudiocode&logoColor=white)](https://insiders.vscode.dev/redirect?url=vscode-insiders:mcp/install?%7B%22name%22%3A%22pline.ai%22%2C%22command%22%3A%22npx%22%2C%22args%22%3A%5B%22-y%22%2C%22pline-api%22%5D%2C%22env%22%3A%7B%22PLINE_BASE_URL%22%3A%22%24%7Binput%3Apline-base-url%7D%22%2C%22PLINE_API_KEY%22%3A%22%24%7Binput%3Apline-api-key%7D%22%7D%2C%22inputs%22%3A%5B%7B%22id%22%3A%22pline-base-url%22%2C%22type%22%3A%22promptString%22%2C%22description%22%3A%22pline.ai%20API%20base%20URL%22%2C%22default%22%3A%22https%3A%2F%2Fapix.pline.ai%2Fv1%22%7D%2C%7B%22id%22%3A%22pline-api-key%22%2C%22type%22%3A%22promptString%22%2C%22description%22%3A%22pline.ai%20API%20key%22%2C%22password%22%3Atrue%7D%5D%7D); VS Code prompts for the URL and key |
| Codex (CLI, IDE extension, app) | `codex mcp add pline-api --env PLINE_BASE_URL=https://apix.pline.ai/v1 --env PLINE_API_KEY=your-own-api-key -- npx -y pline-api`; see [Codex](#codex-1) to keep the key out of the config file |
| Windsurf, Claude Desktop, others | Paste the JSON above into the client's MCP configuration; see [MCP setup](#mcp-setup) for each file's location |

Opening this repository as a workspace configures the server automatically through
[`.cursor/mcp.json`](.cursor/mcp.json) and [`.vscode/mcp.json`](.vscode/mcp.json).
[PUBLISHING.md](PUBLISHING.md) tracks every registry and directory listing.

## Clone and configure for source builds or skills

```sh
git clone https://github.com/grepsr/pline-api.git
cd pline-api
cp .env.example .env
```

Edit `.env`: keep `PLINE_BASE_URL=https://apix.pline.ai/v1` (the public pline.ai API) and set
`PLINE_API_KEY` to your own key.

Load them before launching your agent or MCP client (POSIX shells):

```sh
set -a
. ./.env
set +a
```

Neither client loads `.env` automatically. On other platforms, set the same variables in your
shell or client settings. Git ignores `.env`, local session files, and build output.

## Skill setup

The `pline-api` skill teaches an agent how to choose an API operation, reuse scrape sessions,
extract structured data, poll background jobs, and save results. It uses connected pline.ai MCP
tools when available; otherwise it runs the bundled Python client. A skill-only installation
requires **Python 3.10+, Git, and a pline.ai API key**; the API base URL is `https://apix.pline.ai/v1`. It does not
require Rust, a compiled MCP binary, or pip packages.

The quickest install uses the [`skills`](https://skills.sh) CLI, which copies the skill from this GitHub
repository (not from npm) into the right folder for each agent it detects (add `-g` for a user-wide
install, `-a cursor` to pick agents):

```sh
npx skills add grepsr/pline-api
```

To install by hand, copy the **whole directory**, not just `SKILL.md`:

```text
pline-api/
├── SKILL.md                 # Instructions and when to use the skill
├── scripts/pline.py         # Python client
├── references/endpoints.md  # API fields, examples, and limits
└── agents/openai.yaml       # Codex display metadata
```

The agent needs permission to execute Python, reach your pline.ai API over HTTPS, and write to
any output directory you request. Shell examples below use macOS, Linux, or WSL; native Windows
instructions follow the client sections. These are local-agent installations. A cloud agent needs
the skill, Python, network access, and credentials configured in its own execution environment.

| Client | Personal skill directory | Project-only directory | Invoke in agent chat |
|---|---|---|---|
| [Claude Code](#claude-code) | `~/.claude/skills/pline-api/` | `<project>/.claude/skills/pline-api/` | `/pline-api` |
| [Codex](#codex) | `~/.agents/skills/pline-api/` | `<project>/.agents/skills/pline-api/` | `$pline-api` in CLI/IDE |
| [Cursor](#cursor) | `~/.cursor/skills/pline-api/` | `<project>/.cursor/skills/pline-api/` | `/pline-api` |
| [DeepSeek through Claude Code](#deepseek-through-claude-code) | Same as Claude Code | Same as Claude Code | `/pline-api` |

### Prepare credentials and Python

First complete [Clone and configure](#clone-and-configure-for-source-builds-or-skills), including
loading `.env` in your shell. Run these commands from the cloned `pline-api` repository:

```sh
python3 --version
python3 skills/pline-api/scripts/pline.py --help
python3 -c 'import os; print({name: bool(os.environ.get(name)) for name in ("PLINE_BASE_URL", "PLINE_API_KEY")})'
```

The version must be 3.10 or later, help must list the API commands, and both environment checks
must print `True`. The check prints only whether each value is present. `--help` does not contact
the service; actual scrape, crawl, batch, map, and search requests use your API account.

Start the agent from this configured shell. An already-running desktop app may not inherit new
variables. If necessary, configure them in the agent's execution environment or have the agent
source your local `.env` **in the same shell invocation** that runs `pline.py`. Never paste an API
key into an agent conversation. The Python client does not load `.env` automatically.

### Claude Code

Install [Claude Code](https://code.claude.com/docs/en/setup) and sign in if it is not already set up.
For a personal installation, run from this repository root:

```sh
mkdir -p "$HOME/.claude/skills/pline-api"
cp -R skills/pline-api/. "$HOME/.claude/skills/pline-api/"
python3 "$HOME/.claude/skills/pline-api/scripts/pline.py" --help
```

Launch `claude` in the project where you want to save results, using the shell with your pline.ai
variables loaded. Type `/pline-api` in the conversation and select the skill, for example:

```text
/pline-api Scrape https://example.com as Markdown and save it under ./page-output.
```

Alternatively, install the skill and the MCP server together as the `pline-ai` plugin; Claude Code
prompts for your API URL and key and stores the key in secure storage. This route builds the server
from source, so it needs Rust and Cargo:

```sh
claude plugin marketplace add grepsr/pline-api
claude plugin install pline-ai@pline-ai
```

For a project-only installation, replace the destination above with
`/absolute/path/to/your-project/.claude/skills/pline-api/`, then start Claude Code in that project.
If the skill does not appear, restart the session and confirm `SKILL.md` is directly inside that
folder. Local personal folders do not automatically install the skill into Claude's web or Cowork
sessions; those use account-managed skills. See the official
[Claude skill discovery documentation](https://code.claude.com/docs/en/skills#choose-where-skills-load).

### Codex

For a personal installation, run from this repository root:

```sh
mkdir -p "$HOME/.agents/skills/pline-api"
cp -R skills/pline-api/. "$HOME/.agents/skills/pline-api/"
python3 "$HOME/.agents/skills/pline-api/scripts/pline.py" --help
```

Open your working project in Codex. In the CLI or IDE extension, use `/skills` to find `pline-api`,
or type `$pline-api` in your prompt:

```text
$pline-api Extract the product name and price from https://example.com/product as JSON.
```

In the desktop interface, select the installed skill from its skills picker; the current ChatGPT
interface uses `@` to attach skills. Set the pline.ai variables in the environment where its shell
tools run. For a project-only installation, copy the directory to
`/absolute/path/to/your-project/.agents/skills/pline-api/` and open that project. Restart Codex if
newly copied files are not discovered. These local paths and invocation methods are documented in
[OpenAI's skill guide](https://learn.chatgpt.com/docs/build-skills).

### Cursor

For a personal installation, run from this repository root:

```sh
mkdir -p "$HOME/.cursor/skills/pline-api"
cp -R skills/pline-api/. "$HOME/.cursor/skills/pline-api/"
python3 "$HOME/.cursor/skills/pline-api/scripts/pline.py" --help
```

Open your working project in Cursor and use Agent chat. Type `/` and select `pline-api`, then ask:

```text
/pline-api Read urls.txt, scrape those URLs as a batch in Markdown, and save the completed results under ./batch-output.
```

Use `/absolute/path/to/your-project/.cursor/skills/pline-api/` instead for a project-only install.
Restart Cursor if needed and verify that its agent terminal can see the pline.ai variables.
Cursor also discovers `.agents/skills` and Claude-compatible skill directories, so check for an
existing `pline-api` before making another copy. Its
[official skill guide](https://cursor.com/help/customization/skills) describes these locations and
invocation. The [Skills panel](https://cursor.com/docs/skills) lists discovered skills. These copy
commands install the skill directly; this repository is not packaged as a Cursor marketplace plugin.

For the MCP server in Cursor, either use the **Add to Cursor** button under [Quick install](#quick-install)
(Docker image, any project) or open this repository as a workspace: [`.cursor/mcp.json`](.cursor/mcp.json)
runs the source build and reads `PLINE_BASE_URL` and `PLINE_API_KEY` from Cursor's environment.

### DeepSeek through Claude Code

Here DeepSeek supplies the model and Claude Code runs the skill and Python commands. Follow the
[Claude Code installation above](#claude-code), then configure the provider using
[DeepSeek's official integration guide](https://api-docs.deepseek.com/guides/coding_agents/).
This route does not install a skill into the DeepSeek browser chat.

You need **two separate credentials**: a DeepSeek API key for the model and `PLINE_API_KEY` for
scraping. Keep `PLINE_BASE_URL` pointing at your pline.ai API. In a new terminal, load your pline.ai
`.env`, then configure the model provider:

```sh
export ANTHROPIC_BASE_URL='https://api.deepseek.com/anthropic'
export ANTHROPIC_AUTH_TOKEN='your-deepseek-api-key'
export ANTHROPIC_MODEL='deepseek-flash[1m]'
export ANTHROPIC_DEFAULT_OPUS_MODEL='deepseek-flash[1m]'
export ANTHROPIC_DEFAULT_SONNET_MODEL='deepseek-flash[1m]'
export ANTHROPIC_DEFAULT_HAIKU_MODEL='deepseek-flash'
export CLAUDE_CODE_SUBAGENT_MODEL='deepseek-flash'
export CLAUDE_CODE_EFFORT_LEVEL='max'
claude
```

Replace the key placeholder locally. Model names above follow DeepSeek's guide as checked on
2026-09-17; consult that guide if the provider changes them. These exports affect this shell and
processes launched from it. Invoke the same installed skill:

```text
/pline-api Map https://example.com and show me up to 20 discovered URLs.
```

For another DeepSeek-powered agent, use that host's Agent Skills installation path and credential
settings. The model API alone does not provide the local Python execution environment.

### Native Windows / PowerShell

Run from the cloned repository. Set the variables for this PowerShell session; replace both
placeholders with your operator's API URL and your pline.ai key:

```powershell
$env:PLINE_BASE_URL = 'https://apix.pline.ai/v1'
$env:PLINE_API_KEY = 'your-pline-api-key'
python --version
python skills/pline-api/scripts/pline.py --help
```

Choose **one** destination for the client you are installing into:

```powershell
# Claude Code, including the DeepSeek-through-Claude-Code setup:
$plineSkillDir = Join-Path $HOME '.claude/skills/pline-api'
# Codex: use this assignment instead.
# $plineSkillDir = Join-Path $HOME '.agents/skills/pline-api'
# Cursor: use this assignment instead.
# $plineSkillDir = Join-Path $HOME '.cursor/skills/pline-api'

New-Item -ItemType Directory -Force -Path $plineSkillDir | Out-Null
Copy-Item -Path 'skills/pline-api/*' -Destination $plineSkillDir -Recurse -Force
python (Join-Path $plineSkillDir 'scripts/pline.py') --help
```

For a project-only install, set `$plineSkillDir` to the corresponding `.claude`, `.agents`, or
`.cursor` path inside that project. Launch the client from the configured session. If Python is
available as `py` rather than `python`, use `py -3` in these examples and tell the agent which
interpreter to use. Keep installations and credentials inside WSL when the agent runs in WSL.

For DeepSeek, translate each provider export above to PowerShell syntax before launching `claude`,
for example `$env:ANTHROPIC_BASE_URL = 'https://api.deepseek.com/anthropic'` and
`$env:ANTHROPIC_MODEL = 'deepseek-flash[1m]'`. The other variable names and values stay the same.

### Verify the installed skill

After copying, set `PLINE_SKILL_DIR` to your chosen installation directory, for example:

```sh
PLINE_SKILL_DIR="$HOME/.agents/skills/pline-api"
python3 "$PLINE_SKILL_DIR/scripts/pline.py" --help
python3 "$PLINE_SKILL_DIR/scripts/pline.py" scrape \
  --url https://example.com --output markdown --save ./pline-check
```

The second command makes a real API request. With working credentials it should create
`./pline-check/markdown.md` when the page returns Markdown. On Windows, run the same arguments
with `python (Join-Path $plineSkillDir 'scripts/pline.py')`.

Then invoke the skill in your agent and request one page. Confirm it calls a pline.ai MCP tool or
runs the installed `scripts/pline.py`. If MCP is not configured, ask it to use the Python client.
Files are saved relative to the agent's working project, not the skill installation directory.

### Example tasks and expected results

Prefix these requests with your client's invocation from the table above:

| Request | What the skill should do |
|---|---|
| "Read this page and save the Markdown under `./pages`." | Scrape once and save text locally. |
| "Extract each product's name, price, and URL from this page as JSON." | Supply an extraction prompt or schema with JSON output. |
| "Scrape the URLs in `urls.txt`; download the finished batch into `./results`." | Start a batch, report its ID, poll, and download JSONL artifacts. |
| "Map this site, then crawl at most 20 pages as Markdown." | Discover URLs, respect the page limit, and return the crawl ID and ZIP artifacts. |
| "Search for these terms and return the result URLs." | Use the search endpoint (`serp` in the CLI). |
| "Check crawl job `<job-uuid>` and download its output." | Poll an existing job; do not start a duplicate crawl. |

The Python client and local stdio MCP can save files. HTTP MCP rejects `save_dir` and
`download_dir`; for that transport, use returned content or artifact links, or the Python client
for local downloads. Repeated requests to a site should reuse the MCP's remembered scrape session
or a CLI `--session-file`. For background jobs, keep the returned UUID so you can resume polling
or cancel the correct job.

### Update or remove an installed skill

Run `git pull --ff-only` in your clone, then rerun the copy commands for your chosen client. The
`skills/pline-api/.` source copies the contents into the existing destination without creating a
nested `pline-api/pline-api` directory. Copying updates existing files but does not remove obsolete
ones; for a clean replacement, move your old installed directory aside and copy into a new one.
Keep customizations in your backup and reload the agent after an update. Copy-based installations
do not update themselves when the source repository changes.

To uninstall, remove only the installed `pline-api` directory from the selected personal or project
skill location and reload the agent. Your source clone, outputs, and MCP configuration are separate.

### Troubleshooting

| Symptom | Check |
|---|---|
| The skill is missing from the picker | Confirm the exact path ends in `skills/pline-api/SKILL.md`, with `scripts/` and `references/` alongside it. Open the right project and restart the client. |
| Duplicate skill entries | Keep one intended copy per client/scope; Cursor can discover shared and Claude-compatible locations too. |
| `python3` is not found | Install Python 3.10+ in the environment where the agent runs; on Windows use the available `python` or `py -3` command. |
| `PLINE_BASE_URL` or `PLINE_API_KEY` is not set | Verify the agent's shell environment, not just your separate terminal. Load `.env` in the same command that executes the client if needed. |
| The API returns 401 or 403 | Check the pline.ai key and API base URL. A DeepSeek or Claude credential is not a pline.ai credential. |
| `scripts/pline.py` or the API reference cannot be found | Copy the complete skill directory and use an absolute installation path. |
| The agent cannot run Python or reach the API | Enable the host's required command/network permissions, or use an already configured pline.ai MCP connection. |
| The response is empty | Ask the skill to inspect status and retry with JavaScript rendering when appropriate; empty content can be a soft failure. |

## Direct CLI use

No skill installation is needed to run the client yourself:

```sh
python3 skills/pline-api/scripts/pline.py scrape --url https://example.com --output markdown
python3 skills/pline-api/scripts/pline.py batch start --urls-file urls.txt --output markdown
python3 skills/pline-api/scripts/pline.py crawl start --url https://example.com --limit 20
python3 skills/pline-api/scripts/pline.py map --url https://example.com --limit 100
python3 skills/pline-api/scripts/pline.py serp --query 'example search' --mode fast
```

Use `--help` on any command for flags. For wire-level examples, see the skill's
[API reference](skills/pline-api/references/endpoints.md).

## Download the MCP binary

Download a prebuilt binary from [pline.ai MCP v0.3.3](https://github.com/grepsr/pline-api/releases/tag/v0.3.3)
using the links below. No Rust installation is needed. See
[GitHub Releases](https://github.com/grepsr/pline-api/releases) for all versions.

| Platform | Download |
|---|---|
| macOS Apple Silicon | [pline-mcp-v0.3.3-aarch64-apple-darwin.tar.gz](https://github.com/grepsr/pline-api/releases/download/v0.3.3/pline-mcp-v0.3.3-aarch64-apple-darwin.tar.gz) |
| macOS Intel | [pline-mcp-v0.3.3-x86_64-apple-darwin.tar.gz](https://github.com/grepsr/pline-api/releases/download/v0.3.3/pline-mcp-v0.3.3-x86_64-apple-darwin.tar.gz) |
| Linux x64, glibc 2.35+ (Ubuntu 22.04 or newer) | [pline-mcp-v0.3.3-x86_64-unknown-linux-gnu.tar.gz](https://github.com/grepsr/pline-api/releases/download/v0.3.3/pline-mcp-v0.3.3-x86_64-unknown-linux-gnu.tar.gz) |
| Linux arm64, glibc 2.35+ (Ubuntu 22.04 or newer) | [pline-mcp-v0.3.3-aarch64-unknown-linux-gnu.tar.gz](https://github.com/grepsr/pline-api/releases/download/v0.3.3/pline-mcp-v0.3.3-aarch64-unknown-linux-gnu.tar.gz) |
| Windows x64 | [pline-mcp-v0.3.3-x86_64-pc-windows-msvc.zip](https://github.com/grepsr/pline-api/releases/download/v0.3.3/pline-mcp-v0.3.3-x86_64-pc-windows-msvc.zip) |

Download [SHA256SUMS](https://github.com/grepsr/pline-api/releases/download/v0.3.3/SHA256SUMS)
and compare the archive's SHA-256 hash with its line in that file: use `shasum -a 256 FILE`
on macOS, `sha256sum FILE` on Linux, or `Get-FileHash FILE -Algorithm SHA256` in PowerShell.
Extract the archive into a permanent folder and use the executable's absolute path in the MCP
configuration below. Windows uses `pline-mcp.exe`; macOS and Linux use `pline-mcp`.

## MCP setup

The server runs locally as a stdio process that your MCP client starts; it calls the hosted
pline.ai API over HTTPS with your key. `npx -y pline-api` downloads the
[`pline-api`](https://www.npmjs.com/package/pline-api) launcher, which picks the prebuilt binary for
your platform (macOS arm64/x64, Linux x64/arm64, Windows x64). Node.js 18 or newer is the only
requirement. `https://apix.pline.ai/v1` is the public pline.ai API; replace `your-own-api-key` with your key
and keep real keys in the client's local settings, never in a committed file.

### Cursor

Click **Add to Cursor** under [Quick install](#quick-install), or open **Cursor Settings → MCP →
Add new global MCP server** and paste:

```json
{
  "mcpServers": {
    "pline.ai": {
      "command": "npx",
      "args": ["-y", "pline-api"],
      "env": {
        "PLINE_BASE_URL": "https://apix.pline.ai/v1",
        "PLINE_API_KEY": "your-own-api-key"
      }
    }
  }
}
```

Project-level configuration lives in `.cursor/mcp.json`; this repository ships one that reads the two
variables from Cursor's environment. On Windows, if `npx` is not found, use
`"command": "cmd", "args": ["/c", "npx", "-y", "pline-api"]`.

### Windsurf

Add the same `mcpServers` entry to `~/.codeium/windsurf/mcp_config.json`.

### VS Code

Click **Install in VS Code** under [Quick install](#quick-install), or add to your user settings or
to `.vscode/mcp.json` in a workspace (this repository ships one). VS Code prompts for the values
and masks the key:

```json
{
  "inputs": [
    { "type": "promptString", "id": "pline-base-url", "description": "pline.ai API base URL" },
    { "type": "promptString", "id": "pline-api-key", "description": "pline.ai API key", "password": true }
  ],
  "servers": {
    "pline.ai": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "pline-api"],
      "env": {
        "PLINE_BASE_URL": "${input:pline-base-url}",
        "PLINE_API_KEY": "${input:pline-api-key}"
      }
    }
  }
}
```

### Claude Desktop

Add the `mcpServers` entry from [Cursor](#cursor) to `claude_desktop_config.json` (**Settings →
Developer → Edit Config**) and restart Claude Desktop.

### Claude Code

```sh
claude mcp add pline.ai -e PLINE_BASE_URL=https://apix.pline.ai/v1 -e PLINE_API_KEY=your-own-api-key -- npx -y pline-api
```

Or install the skill and the server together as the `pline-ai` plugin (Claude Code prompts for the
URL and key and stores the key in secure storage):

```sh
claude plugin marketplace add grepsr/pline-api
claude plugin install pline-ai@pline-ai
```

Inside this repository, `.mcp.json` runs the server from source through Cargo instead, for development.

### Codex

The Codex CLI, IDE extension, and app share `~/.codex/config.toml` (or `.codex/config.toml` in a
trusted project). Add the server with one command:

```sh
codex mcp add pline-api --env PLINE_BASE_URL=https://apix.pline.ai/v1 --env PLINE_API_KEY=your-own-api-key -- npx -y pline-api
```

That writes the key into `config.toml`. To keep it there only as a name, edit the file instead and
let Codex forward `PLINE_API_KEY` from the environment it starts in (Codex does not pass its
environment to MCP servers unless a variable is listed in `env_vars`):

```toml
[mcp_servers.pline-api]
command = "npx"
args = ["-y", "pline-api"]
env_vars = ["PLINE_API_KEY"]

[mcp_servers.pline-api.env]
PLINE_BASE_URL = "https://apix.pline.ai/v1"
```

Check it with `codex mcp list`. Codex can read this repository's Claude Code marketplace
(`codex plugin marketplace add grepsr/pline-api`), but it does not resolve the plugin's
`${user_config.*}` placeholders, so the server would start with those literal strings as its URL
and key. Use the configuration above for the server and [the skill steps](#codex) for the skill.

### Docker

Every release is also a multi-platform image (`linux/amd64`, `linux/arm64`) at `ghcr.io/grepsr/pline-mcp`.
Use it where Node.js is unavailable or containers are preferred:

```json
{
  "mcpServers": {
    "pline.ai": {
      "command": "docker",
      "args": ["run", "-i", "--rm", "-e", "PLINE_BASE_URL", "-e", "PLINE_API_KEY",
               "ghcr.io/grepsr/pline-mcp:0.3.3", "--stdio"],
      "env": {
        "PLINE_BASE_URL": "https://apix.pline.ai/v1",
        "PLINE_API_KEY": "your-own-api-key"
      }
    }
  }
}
```

`-e NAME` without a value forwards each variable from the environment the client sets, so the key
stays out of the command line. The container cannot see your filesystem, so `save_dir` and
`download_dir` need a volume mount. Pin a release tag rather than `latest`. Details are in
[the server README](mcp-server/README.md#run-the-docker-image).

### Prebuilt binary or source build

[Download the MCP binary](#download-the-mcp-binary) and point the client at its absolute path with
`"command": "/absolute/path/to/pline-mcp", "args": ["--stdio"]` (`pline-mcp.exe` on Windows). The npm
launcher also honours `PLINE_MCP_BINARY=/path/to/pline-mcp` to run a binary you built yourself:

```sh
cargo build --manifest-path mcp-server/Cargo.toml --release --locked
```

The executable is `pline-mcp`; its public MCP identity is `pline.ai`. Streamable HTTP mode is
`npx -y pline-api --http 127.0.0.1:8080`; see [HTTP mode](mcp-server/README.md#http-mode).

## Update and verify

`npx -y pline-api` resolves the latest published version on each start, so npm installs need no
action; pin `pline-api@<version>` in the client configuration if you want to control upgrades.
For a prebuilt installation, download the new release, verify its checksum, and replace the
installed executable. Reconnect your MCP client after updating.

For source installations, pull updates and rebuild; refresh any installed skill copies:

```sh
git pull --ff-only
cargo build --manifest-path mcp-server/Cargo.toml --release --locked
```

Development checks:

```sh
cargo fmt --manifest-path mcp-server/Cargo.toml -- --check
cargo clippy --manifest-path mcp-server/Cargo.toml --locked --all-targets --all-features -- -D warnings
cargo test --manifest-path mcp-server/Cargo.toml --locked
cargo build --manifest-path mcp-server/Cargo.toml --locked
python3 tests/smoke.py
```

The smoke check uses a localhost mock API and dummy credentials to exercise the skill, stdio, and
HTTP transports. HTTP is stateless and rejects local file output options; local Git installs use
stdio and support saving files. See [HTTP mode](mcp-server/README.md#http-mode) for transport details.

## Build, release, and publish

[Build release binaries](.github/workflows/release.yml) builds and tests all four platforms and
builds the Docker image. Run it manually from a branch in the Actions tab to check builds without
creating a release or pushing an image. It needs no API keys or extra repository secrets.

When ready to release, bump the version everywhere `python3 tests/check_versions.py` looks
(`mcp-server/Cargo.toml`, `server.json`, `.claude-plugin/*.json`, and the pinned image tags in the
READMEs), then push a version tag such as `v0.1.0`. The tag must exactly match the Cargo version
with a `v` prefix. After every platform passes unit and MCP/skill smoke tests, the workflow pushes
`ghcr.io/grepsr/pline-mcp:<version>` and `:latest` for amd64 and arm64, then creates a **draft**
GitHub Release containing all four archives and `SHA256SUMS`. Review the draft and publish it
manually. Reruns can refresh draft assets; they refuse to replace assets on an already published
release.

Publishing the release triggers [Publish release](.github/workflows/publish.yml), which builds the
per-platform npm packages from the release archives, publishes them and the `pline-api` launcher to
npm, then publishes [`server.json`](server.json) to the official MCP Registry with GitHub OIDC.
[PUBLISHING.md](PUBLISHING.md) covers that flow, the credentials involved, and the Claude directory,
Cursor, VS Code, Glama, and community list submissions.

## License

[MIT](LICENSE). Copyright (c) 2026 Grepsr.
