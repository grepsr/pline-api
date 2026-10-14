"""Check the Claude directory plugin in plugin/ against the rest of the repository.

    python3 tests/check_plugin.py          # verify, exit 1 on any problem

The plugin is submitted from its own folder so the directory scans only what the plugin ships. Its
MCP server is the hosted https://mcp.pline.ai/mcp, where the user signs in with OAuth, so its copy
of the pline-api skill is MCP-only: it must not read a credential from the user's machine (no
PLINE_API_KEY, no direct API host, no bundled scripts). The directory holds any plugin that pairs a
local credential read with a remote destination. The standalone skill in skills/pline-api, which
`npx skills add grepsr/pline-api` installs, keeps its Python client and API reference.
"""

import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
PLUGIN = ROOT / 'plugin'
SKILL = ROOT / 'skills/pline-api'
PLUGIN_SKILL = PLUGIN / 'skills/pline-api'
HOSTED_MCP_URL = 'https://mcp.pline.ai/mcp'
LISTING_LINKS = {
    'documentationUrl': 'https://github.com/grepsr/pline-api#readme',
    'supportUrl': 'https://github.com/grepsr/pline-api/issues',
    'privacyPolicyUrl': 'https://www.pline.ai/privacy-policy',
    'termsOfServiceUrl': 'https://www.pline.ai/terms-of-service',
}
# What the directory's credential scan treats as reading a key and sending it off the machine.
FORBIDDEN = re.compile(
    r'PLINE_API_KEY|PLINE_BASE_URL|apix\.pline\.ai|x-api-key|os\.environ|process\.env|api[ _-]?key',
    re.IGNORECASE,
)
EXECUTABLE_SUFFIXES = {'.py', '.sh', '.js', '.mjs', '.cjs', '.ts', '.rb', '.ps1'}


def frontmatter(path):
    text = path.read_text(encoding='utf-8')
    match = re.match(r'^---\n(.*?)\n---\n', text, re.S)
    if not match:
        return {}
    fields = {}
    for line in match.group(1).splitlines():
        key, _, value = line.partition(':')
        fields[key.strip()] = value.strip()
    return fields


def check():
    problems = []

    if not (PLUGIN_SKILL / 'SKILL.md').is_file():
        problems.append('plugin/skills/pline-api/SKILL.md is missing')
    else:
        ours, theirs = frontmatter(PLUGIN_SKILL / 'SKILL.md'), frontmatter(SKILL / 'SKILL.md')
        for key in ('name', 'description'):
            if ours.get(key) != theirs.get(key):
                problems.append(f'plugin skill {key} differs from skills/pline-api: {ours.get(key)!r} != {theirs.get(key)!r}')
    for rel in ('agents/openai.yaml',):
        if (PLUGIN_SKILL / rel).read_bytes() != (SKILL / rel).read_bytes():
            problems.append(f'plugin/skills/pline-api/{rel} differs from skills/pline-api/{rel}')

    for path in sorted(PLUGIN.rglob('*')):
        if not path.is_file() or path.suffix in {'.png', '.jpg', '.jpeg', '.gif', '.webp'}:
            continue
        rel = path.relative_to(ROOT)
        if path.suffix in EXECUTABLE_SUFFIXES:
            problems.append(f'{rel}: the plugin must not bundle scripts; it uses the hosted MCP server')
        # README included: the scanner reads prose too ("API key" next to the API host was held).
        hit = FORBIDDEN.search(path.read_text(encoding='utf-8', errors='replace'))
        if hit:
            problems.append(f'{rel}: mentions {hit.group(0)!r}; the plugin must not read a local credential')

    manifest = json.loads((PLUGIN / '.claude-plugin/plugin.json').read_text(encoding='utf-8'))
    servers = manifest.get('mcpServers', {})
    if (len(servers) != 1 or any(s != {'type': 'http', 'url': HOSTED_MCP_URL} for s in servers.values())):
        problems.append(f'plugin.json must declare exactly one http MCP server at {HOSTED_MCP_URL} and nothing else: {servers}')
    # Set explicitly: without them the directory guesses from the README (it once took
    # example.com from an example prompt as the documentation link).
    for key, wanted in LISTING_LINKS.items():
        if manifest.get(key) != wanted:
            problems.append(f'plugin.json {key} must be {wanted}, found {manifest.get(key)!r}')
    if 'userConfig' in manifest:
        problems.append('plugin.json must not ask for credentials through userConfig; the server uses OAuth')
    icon = manifest.get('icon')
    if not icon or not (PLUGIN / icon).is_file():
        problems.append(f'plugin.json icon {icon!r} does not exist inside plugin/')
    for name in ('README.md', 'LICENSE'):
        if not (PLUGIN / name).is_file():
            problems.append(f'plugin/{name} is missing')

    marketplace = json.loads((ROOT / '.claude-plugin/marketplace.json').read_text(encoding='utf-8'))
    sources = [entry.get('source') for entry in marketplace.get('plugins', [])]
    if sources != ['./plugin']:
        problems.append(f'.claude-plugin/marketplace.json must list the plugin at ./plugin, found {sources}')

    if problems:
        print('Plugin check failed:', file=sys.stderr)
        for problem in problems:
            print(f'  - {problem}', file=sys.stderr)
        return 1
    print(f'Plugin OK: MCP-only skill, hosted MCP server {HOSTED_MCP_URL}, no local credential reads')
    return 0


if __name__ == '__main__':
    sys.exit(check())
