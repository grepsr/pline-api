"""Keep every published version string in step with mcp-server/Cargo.toml.

Checked: server.json (MCP Registry), npm/package.json (npm launcher and its platform packages),
.claude-plugin/plugin.json and marketplace.json (Claude Code plugin, including the npx pin),
mcp-server/Dockerfile (registry ownership label), and both READMEs (release download links,
archive names, pinned GHCR image tags and npm versions, one-click install links).

    python3 tests/check_versions.py              # verify, exit 1 on any mismatch
    python3 tests/check_versions.py --tag v0.1.1 # also require this release tag
    python3 tests/check_versions.py --set 0.1.1  # rewrite every location, then verify

--set also updates the pline-mcp entry in Cargo.lock so `cargo build --locked` keeps working.
"""

import argparse
import base64
import json
from pathlib import Path
import re
import sys
import tomllib
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[1]
IMAGE = 'ghcr.io/grepsr/pline-mcp'
NPM = 'pline-api'
READMES = ('README.md', 'mcp-server/README.md')
SEMVER = r'[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?'


def read(path):
    return (ROOT / path).read_text(encoding='utf-8')


def write(path, text):
    (ROOT / path).write_text(text, encoding='utf-8')


def load_json(path):
    return json.loads(read(path))


def dump_json(path, data):
    write(path, json.dumps(data, indent=2) + '\n')


def cargo_version():
    return tomllib.loads(read('mcp-server/Cargo.toml'))['package']['version']


def set_version(new):
    old = cargo_version()
    if not re.fullmatch(SEMVER, new):
        raise SystemExit(f'--set expects a semantic version, got {new!r}')
    print(f'{old} -> {new}')

    write('mcp-server/Cargo.toml', re.sub(r'(?m)^version = "[^"]+"', f'version = "{new}"', read('mcp-server/Cargo.toml'), count=1))
    lock = read('mcp-server/Cargo.lock')
    lock, count = re.subn(r'(name = "pline-mcp"\nversion = ")[^"]+(")', rf'\g<1>{new}\g<2>', lock)
    if count != 1:
        raise SystemExit('Cargo.lock: expected exactly one pline-mcp package entry')
    write('mcp-server/Cargo.lock', lock)

    server = load_json('server.json')
    server['version'] = new
    for package in server.get('packages', []):
        # The registry rejects a separate version on OCI packages; the tag in identifier carries it.
        if package.get('registryType') == 'oci':
            package['identifier'] = f'{IMAGE}:{new}'
        else:
            package['version'] = new
    dump_json('server.json', server)

    npm = load_json('npm/package.json')
    npm['version'] = new
    for name in npm.get('optionalDependencies', {}):
        npm['optionalDependencies'][name] = new
    dump_json('npm/package.json', npm)

    plugin = load_json('.claude-plugin/plugin.json')
    plugin['version'] = new
    for mcp in plugin.get('mcpServers', {}).values():
        mcp['args'] = [re.sub(rf'^{re.escape(NPM)}@.*$', f'{NPM}@{new}', arg) for arg in mcp.get('args', [])]
    dump_json('.claude-plugin/plugin.json', plugin)

    marketplace = load_json('.claude-plugin/marketplace.json')
    for entry in marketplace.get('plugins', []):
        entry['version'] = new
    dump_json('.claude-plugin/marketplace.json', marketplace)

    for readme in READMES:
        text = read(readme)
        text = text.replace(f'{IMAGE}:{old}', f'{IMAGE}:{new}')
        text = text.replace(f'{NPM}@{old}', f'{NPM}@{new}')
        text = re.sub(r'(releases/(?:tag|download)/v)' + re.escape(old) + r'\b', rf'\g<1>{new}', text)
        text = text.replace(f'pline-mcp-v{old}-', f'pline-mcp-v{new}-')
        text = text.replace(f'MCP v{old}', f'MCP v{new}')
        write(readme, text)


def check(tag):
    version = cargo_version()
    problems = []

    def expect(label, actual, wanted=None):
        wanted = version if wanted is None else wanted
        if actual != wanted:
            problems.append(f'{label}: {actual!r} != {wanted!r}')

    def check_install_target(label, config):
        # A one-click link must run this repository's npm package or image, never a stale or
        # unclaimed name that someone else could publish.
        args = config.get('args', [])
        if config.get('command') == 'npx':
            if not any(arg == NPM or arg.startswith(f'{NPM}@') for arg in args):
                problems.append(f'{label} runs npx {args!r}, expected the {NPM} package')
        elif config.get('command') == 'docker':
            if not any(arg.startswith(f'{IMAGE}:') for arg in args):
                problems.append(f'{label} runs docker {args!r}, expected the {IMAGE} image')
        else:
            problems.append(f'{label} runs {config.get("command")!r}, expected npx or docker')

    if tag:
        expect('release tag', tag, f'v{version}')

    lock = re.search(r'name = "pline-mcp"\nversion = "([^"]+)"', read('mcp-server/Cargo.lock'))
    expect('Cargo.lock pline-mcp entry', lock.group(1) if lock else None)

    server = load_json('server.json')
    expect('server.json version', server.get('version'))
    npm = load_json('npm/package.json')
    expect('npm/package.json version', npm.get('version'))
    expect('npm/package.json mcpName', npm.get('mcpName'), server['name'])
    for name, pinned in npm.get('optionalDependencies', {}).items():
        expect(f'npm/package.json optionalDependencies[{name}]', pinned)
    for index, package in enumerate(server.get('packages', [])):
        if package.get('registryType') == 'oci':
            # The registry rejects registryBaseUrl and version on OCI packages.
            for field in ('registryBaseUrl', 'version'):
                if field in package:
                    problems.append(f'server.json packages[{index}] is OCI and must not set {field}')
            expect(f'server.json packages[{index}].identifier', package.get('identifier'), f'{IMAGE}:{version}')
        else:
            expect(f'server.json packages[{index}].version', package.get('version'))
        if package.get('registryType') == 'npm':
            expect(f'server.json packages[{index}].identifier', package.get('identifier'), npm['name'])

    label = re.search(r'io\.modelcontextprotocol\.server\.name="([^"]+)"', read('mcp-server/Dockerfile'))
    expect('Dockerfile io.modelcontextprotocol.server.name label', label.group(1) if label else None, server['name'])

    plugin = load_json('.claude-plugin/plugin.json')
    expect('plugin.json version', plugin.get('version'))
    for name, mcp in plugin.get('mcpServers', {}).items():
        for arg in mcp.get('args', []):
            if arg.startswith(f'{NPM}@'):
                expect(f'plugin.json mcpServers[{name}] npx pin', arg, f'{NPM}@{version}')
    marketplace = load_json('.claude-plugin/marketplace.json')
    for entry in marketplace.get('plugins', []):
        expect(f'marketplace.json plugin {entry.get("name")} version', entry.get('version'))
        expect(f'marketplace.json plugin {entry.get("name")} name', entry.get('name'), plugin['name'])

    for readme in READMES:
        text = read(readme)
        for found in re.findall(re.escape(IMAGE) + r':([0-9][^\s`"\')\]]*)', text):
            expect(f'{readme} image tag', found)
        for found in re.findall(r'(?<![\w/-])' + re.escape(NPM) + r'@([0-9][^\s`"\')\]]*)', text):
            expect(f'{readme} npm version pin', found)
        for found in re.findall(r'releases/(?:tag|download)/v(' + SEMVER + r')', text):
            expect(f'{readme} release link', found)
        for found in re.findall(r'pline-mcp-v([0-9]+\.[0-9]+\.[0-9]+)-', text):
            expect(f'{readme} release archive name', found)
        # GitHub strips non-http(s) link targets, so a cursor:// deeplink renders as a dead button.
        if 'cursor://' in text:
            problems.append(f'{readme} uses a cursor:// link; GitHub strips it, use https://cursor.com/install-mcp')
        for encoded in re.findall(r'https://cursor\.com/(?:en/)?install-mcp\?name=[^&]+&config=([A-Za-z0-9+/=_-]+)', text):
            parsed = json.loads(base64.b64decode(encoded + '=' * (-len(encoded) % 4)))
            check_install_target(f'{readme} Cursor install link', parsed)
            config = json.dumps(parsed)
            for found in re.findall(re.escape(IMAGE) + r':([^"\s]+)', config):
                expect(f'{readme} Cursor install link image tag', found)
            for found in re.findall(r'"' + re.escape(NPM) + r'@([^"]+)"', config):
                expect(f'{readme} Cursor install link npm pin', found)
        for encoded in re.findall(r'vscode(?:-insiders)?:mcp/install\?(\S+?)\)', text):
            parsed = json.loads(unquote(encoded))
            check_install_target(f'{readme} VS Code install link', parsed)
            config = json.dumps(parsed)
            for found in re.findall(re.escape(IMAGE) + r':([^"\s]+)', config):
                expect(f'{readme} VS Code install link image tag', found)
            for found in re.findall(r'"' + re.escape(NPM) + r'@([^"]+)"', config):
                expect(f'{readme} VS Code install link npm pin', found)

    if problems:
        print(f'Version mismatch against mcp-server/Cargo.toml ({version}):', file=sys.stderr)
        for problem in problems:
            print(f'  - {problem}', file=sys.stderr)
        return 1
    print(f'All published versions agree: {version}')
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--tag', help='release tag that must equal v<Cargo.toml version>')
    parser.add_argument('--set', metavar='VERSION', help='rewrite every location to this version before checking')
    args = parser.parse_args()
    if args.set:
        set_version(args.set)
    return check(args.tag)


if __name__ == '__main__':
    sys.exit(main())
