"""Assemble the per-platform npm packages (pline-mcp-<os>-<cpu>) from release archives.

Each package carries one prebuilt binary under bin/ and declares os/cpu so npm installs only the
matching one through the optionalDependencies of the main `pline-mcp` package.

    python3 npm/build-platform-packages.py --dist dist --out npm/dist
    python3 npm/build-platform-packages.py --check   # print the platform table, build nothing

`dist` must contain the release archives and SHA256SUMS produced by release.yml.
"""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tarfile
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]
MAIN = json.loads((ROOT / 'npm/package.json').read_text(encoding='utf-8'))

# Keep in sync with PLATFORMS in npm/bin/pline-mcp.js and the matrix in release.yml.
PLATFORMS = [
    # (npm package, node os, node cpu, cargo target, archive extension)
    ('pline-mcp-darwin-arm64', 'darwin', 'arm64', 'aarch64-apple-darwin', 'tar.gz'),
    ('pline-mcp-darwin-x64', 'darwin', 'x64', 'x86_64-apple-darwin', 'tar.gz'),
    ('pline-mcp-linux-x64', 'linux', 'x64', 'x86_64-unknown-linux-gnu', 'tar.gz'),
    ('pline-mcp-linux-arm64', 'linux', 'arm64', 'aarch64-unknown-linux-gnu', 'tar.gz'),
    ('pline-mcp-win32-x64', 'win32', 'x64', 'x86_64-pc-windows-msvc', 'zip'),
]


def read_sums(dist):
    sums = {}
    for line in (dist / 'SHA256SUMS').read_text(encoding='utf-8').splitlines():
        if line.strip():
            digest, name = line.split(None, 1)
            sums[name.strip().lstrip('*')] = digest.lower()
    return sums


def extract_binary(archive, exe, destination):
    with tempfile.TemporaryDirectory() as tmp:
        if archive.suffix == '.zip':
            with zipfile.ZipFile(archive) as package:
                package.extract(exe, tmp)
        else:
            with tarfile.open(archive) as package:
                package.extract(package.getmember(exe), tmp, filter='data')
        shutil.move(Path(tmp) / exe, destination)
    destination.chmod(0o755)


def build(dist, out):
    version = MAIN['version']
    sums = read_sums(dist)
    out.mkdir(parents=True, exist_ok=True)
    built = []
    for name, node_os, cpu, target, ext in PLATFORMS:
        if MAIN['optionalDependencies'].get(name) != version:
            raise SystemExit(f'npm/package.json optionalDependencies[{name}] must be {version}')
        archive = dist / f'pline-mcp-v{version}-{target}.{ext}'
        if not archive.is_file():
            raise SystemExit(f'missing release archive {archive}')
        with archive.open('rb') as stream:
            digest = hashlib.file_digest(stream, 'sha256').hexdigest()
        if sums.get(archive.name) != digest:
            raise SystemExit(f'checksum mismatch for {archive.name}: SHA256SUMS={sums.get(archive.name)} actual={digest}')

        package_dir = out / name
        if package_dir.exists():
            shutil.rmtree(package_dir)
        (package_dir / 'bin').mkdir(parents=True)
        exe = 'pline-mcp.exe' if node_os == 'win32' else 'pline-mcp'
        extract_binary(archive, exe, package_dir / 'bin' / exe)

        manifest = {
            'name': name,
            'version': version,
            'description': f'{node_os}/{cpu} binary for the pline-mcp MCP server. Install `pline-mcp` instead of this package.',
            'repository': MAIN['repository'],
            'homepage': MAIN['homepage'],
            'bugs': MAIN['bugs'],
            'keywords': MAIN['keywords'],
            'os': [node_os],
            'cpu': [cpu],
            'files': [f'bin/{exe}'],
            'publishConfig': {'access': 'public'},
        }
        if 'license' in MAIN:
            manifest['license'] = MAIN['license']
        (package_dir / 'package.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
        (package_dir / 'README.md').write_text(
            f'# {name}\n\nPrebuilt `{exe}` ({target}) for the [pline-mcp](https://www.npmjs.com/package/pline-mcp) '
            f'MCP server, version {version}. npm selects this package automatically; install `pline-mcp` '
            f'rather than this package directly.\n',
            encoding='utf-8')
        built.append(package_dir)
        print(f'built {package_dir} from {archive.name}')
    return built


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--dist', type=Path, help='directory holding the release archives and SHA256SUMS')
    parser.add_argument('--out', type=Path, default=ROOT / 'npm/dist', help='where to write the packages')
    parser.add_argument('--check', action='store_true', help='print the platform table and exit')
    args = parser.parse_args()

    if args.check or not args.dist:
        print(f'pline-mcp {MAIN["version"]} platform packages:')
        for name, node_os, cpu, target, ext in PLATFORMS:
            pinned = MAIN['optionalDependencies'].get(name)
            print(f'  {name:26} {node_os:7} {cpu:6} {target:28} .{ext}  optionalDependency={pinned}')
            if pinned != MAIN['version']:
                print(f'    !! optionalDependencies[{name}] is {pinned}, expected {MAIN["version"]}', file=sys.stderr)
                return 1
        if not args.dist:
            return 0
    build(args.dist, args.out)
    return 0


if __name__ == '__main__':
    sys.exit(main())
