"""Check the Claude directory plugin in plugin/ against the rest of the repository.

    python3 tests/check_plugin.py          # verify, exit 1 on any problem
    python3 tests/check_plugin.py --sync   # copy skills/pline-api into the plugin, then verify

The plugin is submitted from its own folder so the directory scans only what the plugin ships.
It carries a copy of the agent skill, which must stay identical to skills/pline-api (the copy
`npx skills add grepsr/pline-api` installs).
"""

import argparse
import filecmp
import json
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]
PLUGIN = ROOT / 'plugin'
SKILL = ROOT / 'skills/pline-api'
PLUGIN_SKILL = PLUGIN / 'skills/pline-api'
HOSTED_MCP_URL = 'https://mcp.pline.ai/mcp'
IGNORED = {'__pycache__', '.DS_Store'}


def files(root):
    return sorted(p.relative_to(root) for p in root.rglob('*')
                  if p.is_file() and not IGNORED.intersection(p.parts))


def sync():
    if PLUGIN_SKILL.exists():
        shutil.rmtree(PLUGIN_SKILL)
    shutil.copytree(SKILL, PLUGIN_SKILL, ignore=shutil.ignore_patterns(*IGNORED))


def check():
    problems = []
    source, copy = files(SKILL), files(PLUGIN_SKILL)
    if source != copy:
        problems.append(f'plugin skill files differ: only in skills/: {sorted(set(map(str, source)) - set(map(str, copy)))}, '
                        f'only in plugin/: {sorted(set(map(str, copy)) - set(map(str, source)))}')
    for rel in set(source) & set(copy):
        if not filecmp.cmp(SKILL / rel, PLUGIN_SKILL / rel, shallow=False):
            problems.append(f'plugin/skills/pline-api/{rel} differs from skills/pline-api/{rel}')

    manifest = json.loads((PLUGIN / '.claude-plugin/plugin.json').read_text(encoding='utf-8'))
    servers = manifest.get('mcpServers', {})
    if [s.get('url') for s in servers.values()] != [HOSTED_MCP_URL] or any(s.get('type') != 'http' for s in servers.values()):
        problems.append(f'plugin.json must declare exactly one http MCP server at {HOSTED_MCP_URL}: {servers}')
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
        print('Plugin check failed (run `python3 tests/check_plugin.py --sync` after editing the skill):', file=sys.stderr)
        for problem in problems:
            print(f'  - {problem}', file=sys.stderr)
        return 1
    print(f'Plugin OK: {len(copy)} skill files in sync, hosted MCP server {HOSTED_MCP_URL}')
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--sync', action='store_true', help='copy skills/pline-api into plugin/ first')
    if parser.parse_args().sync:
        sync()
    return check()


if __name__ == '__main__':
    sys.exit(main())
