"""Test `pline-api init` against a throwaway home directory and stand-in client CLIs.

    python3 tests/init_test.py

Nothing outside a temporary directory is touched: HOME, APPDATA and XDG_CONFIG_HOME point into it,
and `claude`, `codex` and `npx` on PATH are stubs that record their arguments.
"""

import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
LAUNCHER = ROOT / 'npm/bin/pline-mcp.js'
NODE = shutil.which('node')
KEY = 'init-test-key'
URL = 'https://apix.pline.ai/v1'


def stub(bin_dir, name, log):
    script = bin_dir / name
    script.write_text(f'#!/bin/sh\nprintf "%s\\n" "{name} $*" >> "{log}"\n', encoding='utf-8')
    script.chmod(0o755)


def layout(home):
    """Per-platform config locations, mirroring npm/lib/init.js."""
    if sys.platform == 'darwin':
        support = home / 'Library/Application Support'
        return support / 'Code/User', support / 'Claude'
    return home / '.config/Code/User', None  # Linux: no Claude Desktop


def run(home, args, env_extra=None):
    bin_dir = home / 'bin'
    env = {
        'HOME': str(home),
        'XDG_CONFIG_HOME': str(home / '.config'),
        'APPDATA': str(home / 'AppData'),
        'PATH': os.pathsep.join([str(bin_dir), str(Path(NODE).parent), '/usr/bin', '/bin']),
    }
    env.update(env_extra or {})
    return subprocess.run([NODE, str(LAUNCHER), 'init', *args], env=env, stdin=subprocess.DEVNULL,
                          capture_output=True, text=True, timeout=60)


def fresh_home(temp, name):
    home = Path(temp) / name
    (home / 'bin').mkdir(parents=True)
    log = home / 'calls.log'
    for tool in ('claude', 'codex', 'npx'):
        stub(home / 'bin', tool, log)
    (home / '.cursor').mkdir()
    (home / '.codeium/windsurf').mkdir(parents=True)
    vscode, desktop = layout(home)
    vscode.mkdir(parents=True)
    if desktop:
        desktop.mkdir(parents=True)
    return home, log


def check_full_setup(temp):
    home, log = fresh_home(temp, 'full')
    vscode, desktop = layout(home)
    windsurf = home / '.codeium/windsurf/mcp_config.json'
    windsurf.write_text(json.dumps({'mcpServers': {'other': {'command': 'other'}}}), encoding='utf-8')
    (vscode / 'mcp.json').write_text(json.dumps({'servers': {}, 'inputs': [{'id': 'keep-me', 'type': 'promptString'}]}),
                                     encoding='utf-8')

    result = run(home, ['--yes'], {'PLINE_API_KEY': KEY})
    assert result.returncode == 0, result.stderr

    cursor = json.loads((home / '.cursor/mcp.json').read_text())
    assert cursor['mcpServers']['pline.ai'] == {
        'command': 'npx', 'args': ['-y', 'pline-api'], 'env': {'PLINE_BASE_URL': URL, 'PLINE_API_KEY': KEY}}
    assert stat.S_IMODE((home / '.cursor/mcp.json').stat().st_mode) == 0o600

    merged = json.loads(windsurf.read_text())
    assert merged['mcpServers']['other'] == {'command': 'other'}, 'existing servers must be kept'
    assert merged['mcpServers']['pline.ai']['env']['PLINE_API_KEY'] == KEY
    assert list(windsurf.parent.glob('mcp_config.json.pline-backup-*')), 'existing file must be backed up'

    code = json.loads((vscode / 'mcp.json').read_text())
    assert code['servers']['pline.ai']['env']['PLINE_API_KEY'] == '${input:pline-api-key}'
    assert KEY not in (vscode / 'mcp.json').read_text(), 'VS Code must prompt for the key, not store it'
    assert [i['id'] for i in code['inputs']] == ['keep-me', 'pline-api-key']

    if desktop:
        claude_desktop = json.loads((desktop / 'claude_desktop_config.json').read_text())
        assert claude_desktop['mcpServers']['pline.ai']['env']['PLINE_API_KEY'] == KEY

    calls = log.read_text().splitlines()
    assert 'claude mcp remove pline-ai --scope user' in calls
    assert (f'claude mcp add pline-ai --scope user -e PLINE_BASE_URL={URL} -e PLINE_API_KEY={KEY} '
            '-- npx -y pline-api') in calls, calls
    assert 'codex mcp remove pline-api' in calls
    assert f'codex mcp add pline-api --env PLINE_BASE_URL={URL} --env PLINE_API_KEY={KEY} -- npx -y pline-api' in calls
    skill = [c for c in calls if c.startswith('npx ')]
    assert skill == ['npx -y skills add grepsr/pline-api --skill pline-api -a claude-code -a codex -a cursor '
                     '-a github-copilot -a windsurf -g -y'], skill
    assert KEY not in result.stderr, 'the key must never be printed'


def check_rerun_updates(temp):
    home, _ = fresh_home(temp, 'rerun')
    run(home, ['--yes', '--agent', 'cursor', '--mcp-only'], {'PLINE_API_KEY': 'old'})
    result = run(home, ['--yes', '--agent', 'cursor', '--mcp-only', '--base-url', 'https://example.test/v1'],
                 {'PLINE_API_KEY': 'new'})
    assert result.returncode == 0, result.stderr
    env = json.loads((home / '.cursor/mcp.json').read_text())['mcpServers']['pline.ai']['env']
    assert env == {'PLINE_BASE_URL': 'https://example.test/v1', 'PLINE_API_KEY': 'new'}
    assert 'updated pline.ai' in result.stderr


def check_invalid_json_is_left_alone(temp):
    home, _ = fresh_home(temp, 'invalid')
    target = home / '.cursor/mcp.json'
    target.write_text('{ // comment\n "mcpServers": {} }', encoding='utf-8')
    before = target.read_text()
    result = run(home, ['--yes', '--agent', 'cursor', '--mcp-only'], {'PLINE_API_KEY': KEY})
    assert result.returncode == 1 and 'not plain JSON' in result.stderr, result.stderr
    assert target.read_text() == before, 'a file that is not plain JSON must not be rewritten'


def check_dry_run_and_guards(temp):
    home, log = fresh_home(temp, 'guards')
    result = run(home, ['--dry-run'])
    assert result.returncode == 0, result.stderr
    assert 'would add pline.ai' in result.stderr and 'would run: claude mcp add' in result.stderr
    assert not (home / '.cursor/mcp.json').exists() and not log.exists(), 'dry run must change nothing'

    result = run(home, ['--dry-run'], {'PLINE_API_KEY': KEY})
    assert 'PLINE_API_KEY=********' in result.stderr and KEY not in result.stderr, 'dry run must not print the key'

    result = run(home, [], {'PLINE_API_KEY': KEY})
    assert result.returncode == 1 and '--yes' in result.stderr, 'no terminal and no --yes must not apply'

    result = run(home, ['--yes', '--agent', 'cursor'])
    assert result.returncode == 1 and 'PLINE_API_KEY' in result.stderr, 'no key and no terminal must stop'
    assert not (home / '.cursor/mcp.json').exists()

    result = run(home, ['--agent', 'nope'])
    assert result.returncode == 2 and 'unknown agent' in result.stderr

    result = run(home, ['--yes', '--agent', 'vscode', '--mcp-only'])
    assert result.returncode == 0, 'VS Code needs no key at setup: ' + result.stderr

    empty = Path(temp) / 'empty'
    (empty / 'bin').mkdir(parents=True)
    result = run(empty, ['--yes'], {'PLINE_API_KEY': KEY})
    assert result.returncode == 1 and 'found no supported MCP client' in result.stderr


def main():
    if os.name == 'nt':
        print('SKIP: init_test.py uses POSIX shell stubs')
        return
    assert NODE, 'node is required'
    with tempfile.TemporaryDirectory() as temp:
        check_full_setup(temp)
        check_rerun_updates(temp)
        check_invalid_json_is_left_alone(temp)
        check_dry_run_and_guards(temp)
    print('PASS: init detects clients, merges and backs up configs, keeps VS Code keyless, installs the skill')


if __name__ == '__main__':
    main()
