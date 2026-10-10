#!/usr/bin/env node
'use strict';

// Launcher for the pline.ai MCP server. The server itself is a native Rust binary; this script
// finds the right build for the current platform and runs it with the caller's arguments and
// environment, so `npx -y pline-api` works like any Node-based MCP server.
//
// Resolution order:
//   1. PLINE_MCP_BINARY, an explicit path (development and tests).
//   2. The platform package installed through optionalDependencies, e.g. @pline/api-darwin-arm64.
//   3. A cached download of the matching GitHub Release archive, verified against SHA256SUMS.
//
// Everything this script prints goes to stderr: stdout is the MCP stdio channel.

const { spawn, spawnSync } = require('node:child_process');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const pkg = require('../package.json');

const REPO = 'grepsr/pline-api';
// Keep in sync with PLATFORMS in npm/build-platform-packages.py and the matrix in release.yml.
const PLATFORMS = {
  'darwin-arm64': { pkg: '@pline/api-darwin-arm64', target: 'aarch64-apple-darwin', ext: 'tar.gz' },
  'darwin-x64': { pkg: '@pline/api-darwin-x64', target: 'x86_64-apple-darwin', ext: 'tar.gz' },
  'linux-x64': { pkg: '@pline/api-linux-x64', target: 'x86_64-unknown-linux-gnu', ext: 'tar.gz' },
  'linux-arm64': { pkg: '@pline/api-linux-arm64', target: 'aarch64-unknown-linux-gnu', ext: 'tar.gz' },
  'win32-x64': { pkg: '@pline/api-win32-x64', target: 'x86_64-pc-windows-msvc', ext: 'zip' },
};

const exeName = process.platform === 'win32' ? 'pline-mcp.exe' : 'pline-mcp';

function log(message) {
  process.stderr.write(`[pline-mcp] ${message}\n`);
}

function fail(message) {
  log(message);
  process.exit(1);
}

function platformInfo() {
  const key = `${process.platform}-${process.arch}`;
  const info = PLATFORMS[key];
  if (!info) {
    fail(
      `no prebuilt pline-mcp binary for ${key}. Build from source with Cargo or download another ` +
        `build from https://github.com/${REPO}/releases, then set PLINE_MCP_BINARY to its path.`,
    );
  }
  return { key, ...info };
}

function fromPlatformPackage(info) {
  try {
    const binary = require.resolve(`${info.pkg}/bin/${exeName}`);
    return fs.existsSync(binary) ? binary : null;
  } catch {
    return null;
  }
}

function cacheDir(info) {
  const root =
    process.env.PLINE_MCP_CACHE_DIR ||
    (process.platform === 'win32'
      ? path.join(process.env.LOCALAPPDATA || os.homedir(), 'pline-mcp', 'cache')
      : path.join(process.env.XDG_CACHE_HOME || path.join(os.homedir(), '.cache'), 'pline-mcp'));
  return path.join(root, pkg.version, info.key);
}

async function fetchBuffer(url) {
  if (typeof fetch !== 'function') {
    fail(`Node ${process.versions.node} lacks fetch; use Node 18 or newer, or install the platform package ${platformInfo().pkg}.`);
  }
  const response = await fetch(url, { redirect: 'follow' });
  if (!response.ok) {
    throw new Error(`${url} returned HTTP ${response.status}`);
  }
  return Buffer.from(await response.arrayBuffer());
}

async function downloadRelease(info) {
  const version = pkg.version;
  const archiveName = `pline-mcp-v${version}-${info.target}.${info.ext}`;
  // PLINE_MCP_DOWNLOAD_BASE overrides the release URL for mirrors and tests.
  const base = process.env.PLINE_MCP_DOWNLOAD_BASE || `https://github.com/${REPO}/releases/download/v${version}`;
  const dir = cacheDir(info);
  const binary = path.join(dir, exeName);
  if (fs.existsSync(binary)) {
    return binary;
  }

  log(`downloading ${archiveName} from ${base} (first run only)`);
  const [archive, sums] = await Promise.all([
    fetchBuffer(`${base}/${archiveName}`),
    fetchBuffer(`${base}/SHA256SUMS`),
  ]);

  const expectedLine = sums
    .toString('utf8')
    .split('\n')
    .find((line) => line.trim().endsWith(`  ${archiveName}`));
  if (!expectedLine) {
    throw new Error(`SHA256SUMS has no entry for ${archiveName}`);
  }
  const expected = expectedLine.trim().split(/\s+/)[0].toLowerCase();
  const actual = crypto.createHash('sha256').update(archive).digest('hex');
  if (actual !== expected) {
    throw new Error(`checksum mismatch for ${archiveName}: expected ${expected}, got ${actual}`);
  }

  fs.mkdirSync(dir, { recursive: true });
  const tmpDir = fs.mkdtempSync(path.join(dir, 'extract-'));
  try {
    const archivePath = path.join(tmpDir, archiveName);
    fs.writeFileSync(archivePath, archive);
    // bsdtar on macOS and Windows 10+ and GNU tar on Linux all extract both .tar.gz and .zip.
    const tar = spawnSync('tar', ['-xf', archivePath, '-C', tmpDir], { stdio: ['ignore', 'ignore', 'inherit'] });
    if (tar.status !== 0) {
      throw new Error(`tar failed to extract ${archiveName}${tar.error ? `: ${tar.error.message}` : ''}`);
    }
    const extracted = path.join(tmpDir, exeName);
    if (!fs.existsSync(extracted)) {
      throw new Error(`${archiveName} did not contain ${exeName}`);
    }
    fs.renameSync(extracted, binary);
    if (process.platform !== 'win32') {
      fs.chmodSync(binary, 0o755);
    }
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
  return binary;
}

async function resolveBinary() {
  const override = process.env.PLINE_MCP_BINARY;
  if (override) {
    if (!fs.existsSync(override)) {
      fail(`PLINE_MCP_BINARY points to ${override}, which does not exist`);
    }
    return override;
  }
  const info = platformInfo();
  const installed = fromPlatformPackage(info);
  if (installed) {
    return installed;
  }
  try {
    return await downloadRelease(info);
  } catch (error) {
    fail(
      `could not obtain the pline-mcp binary for ${info.key}: ${error.message}. ` +
        `Reinstall with optional dependencies enabled, or download it from ` +
        `https://github.com/${REPO}/releases/tag/v${pkg.version} and set PLINE_MCP_BINARY.`,
    );
  }
}

async function main() {
  const binary = await resolveBinary();
  const child = spawn(binary, process.argv.slice(2), { stdio: 'inherit', windowsHide: true });

  const forward = (signal) => () => {
    if (!child.killed) {
      child.kill(signal);
    }
  };
  for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
    try {
      process.on(signal, forward(signal));
    } catch {
      // Not every signal exists on every platform.
    }
  }

  child.on('error', (error) => fail(`failed to start ${binary}: ${error.message}`));
  child.on('exit', (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
      return;
    }
    process.exit(code == null ? 1 : code);
  });
}

main().catch((error) => fail(error.stack || String(error)));
