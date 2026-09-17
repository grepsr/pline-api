# pline.ai API — Agent skill and MCP

Use the hosted pline.ai API from an agent skill or an MCP client. Both integrations use your own
API key and call the same hosted scraping API. The scraping backend is not part of this repository.

| Integration | Contents | Requirements |
|---|---|---|
| [pline-api skill](skills/pline-api/SKILL.md) | Agent instructions, API reference, Python CLI for scrape, batch, crawl, map, and search | Python 3.10+; no pip dependencies |
| [MCP server](mcp-server/README.md) | Ten tools over local stdio; embedded usage guide and API reference | Prebuilt binary; Rust and Cargo only for source builds |

## Clone and configure for source builds or skills

```sh
git clone https://github.com/grepsr/pline-api.git
cd pline-api
cp .env.example .env
```

Edit `.env`: set `PLINE_BASE_URL` to the public HTTPS API URL supplied by the service
operator and `PLINE_API_KEY` to your own key. `https://api.example.com` is a placeholder.

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
requires **Python 3.10+, Git, a pline.ai API key, and the operator's API base URL**. It does not
require Rust, a compiled MCP binary, or pip packages.

Install the **whole directory**, not just `SKILL.md`:

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
$env:PLINE_BASE_URL = 'https://api.example.com'
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

For published versions, download the matching archive and `SHA256SUMS` from
[GitHub Releases](https://github.com/grepsr/pline-api/releases). No Rust installation is needed.
Before the first release is published, use the source build below.

| Platform | Archive suffix |
|---|---|
| macOS Apple Silicon | `aarch64-apple-darwin.tar.gz` |
| macOS Intel | `x86_64-apple-darwin.tar.gz` |
| Linux x64, glibc 2.35+ (Ubuntu 22.04 or newer) | `x86_64-unknown-linux-gnu.tar.gz` |
| Windows x64 | `x86_64-pc-windows-msvc.zip` |

Archives are named `pline-mcp-vVERSION-TARGET.tar.gz` (or `.zip` on Windows).
Compare the archive's SHA-256 hash with its line in `SHA256SUMS`: use `shasum -a 256 FILE`
on macOS, `sha256sum FILE` on Linux, or `Get-FileHash FILE -Algorithm SHA256` in PowerShell.
Extract the archive into a permanent folder and use the executable's absolute path in the MCP
configuration below. Windows uses `pline-mcp.exe`; macOS and Linux use `pline-mcp`.

## MCP setup

If you downloaded a binary, skip compilation. To build from source instead:

```sh
cargo build --manifest-path mcp-server/Cargo.toml --release --locked
```

The included `.mcp.json` is for source builds: it runs Cargo from this repository.
To use a downloaded binary, configure your MCP client with its installed path instead.
For clients accepting `mcpServers` JSON configuration:

```json
{
  "mcpServers": {
    "pline.ai": {
      "command": "/absolute/path/to/pline-mcp",
      "args": ["--stdio"],
      "env": {
        "PLINE_BASE_URL": "https://api.example.com",
        "PLINE_API_KEY": "your-own-api-key"
      }
    }
  }
}
```

On Windows, use the binary's `.exe` suffix. Keep real credentials in local client settings.
The MCP client launches the process; no local HTTP listener or public port is needed.
The executable is `pline-mcp`; its public MCP identity is `pline.ai`.

## Update and verify

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
A license has not yet been selected.

## Build and release binaries

[Build release binaries](.github/workflows/release.yml) builds and tests all four platforms.
Run it manually from a branch in the Actions tab to download build artifacts without creating
a release. It needs no API keys or extra repository secrets.

When ready to release, push a version tag such as `v0.1.0`. The tag must exactly match the
version in `mcp-server/Cargo.toml` with a `v` prefix. After every platform passes unit and MCP/skill
smoke tests, the workflow creates a **draft** GitHub Release containing all four archives and
`SHA256SUMS`. Review the draft and publish it manually. Reruns can refresh draft assets;
they refuse to replace assets on an already published release.
