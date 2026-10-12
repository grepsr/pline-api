'use strict';

// `npx -y pline-api init`: one command that registers the pline.ai MCP server with every MCP client
// it finds and installs the pline-api agent skill. The server itself never prompts for anything; this
// is the interactive setup step. Everything goes to stderr except the help text.

const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const readline = require('node:readline');

const DEFAULT_BASE_URL = 'https://apix.pline.ai/v1';
const SKILL_SOURCE = 'grepsr/pline-api';
const SKILL_NAME = 'pline-api';
const WINDOWS = process.platform === 'win32';

const HELP = `usage: npx -y pline-api@latest init [options]

Registers the pline.ai MCP server with your MCP clients and installs the pline-api agent skill.
With no --agent, it sets up every supported client found on this machine.

  -a, --agent <id>   only this client; repeat for several. One of:
                     claude-code, codex, cursor, vscode, claude-desktop, windsurf
      --all          every client found (the default)
      --mcp-only     register the MCP server only
      --skill-only   install the agent skill only
      --base-url <u> pline.ai API base URL (default ${DEFAULT_BASE_URL})
  -y, --yes          do not ask for confirmation
      --dry-run      show what would change without changing anything
  -h, --help         show this help

The API key comes from PLINE_API_KEY, or a hidden prompt. It is written into each client's own
MCP configuration (VS Code instead prompts for it when the server first starts). Existing config
files are merged, never replaced, and a backup is written next to each one before it changes.
`;

function home() {
  return os.homedir();
}

function appData() {
  return process.env.APPDATA || path.join(home(), 'AppData', 'Roaming');
}

function configHome() {
  return process.env.XDG_CONFIG_HOME || path.join(home(), '.config');
}

function onPath(command) {
  const dirs = (process.env.PATH || '').split(path.delimiter).filter(Boolean);
  const exts = WINDOWS ? (process.env.PATHEXT || '.EXE;.CMD;.BAT').split(';') : [''];
  return dirs.some((dir) => exts.some((ext) => fs.existsSync(path.join(dir, command + ext))));
}

function run(command, args, options = {}) {
  // Windows CLIs installed through npm are .cmd shims, which only start through a shell.
  return spawnSync(command, args, { encoding: 'utf8', shell: WINDOWS, ...options });
}

// The stdio command each file-based client runs. Windows GUI clients start npx through cmd.
function serverCommand() {
  return WINDOWS
    ? { command: 'cmd', args: ['/c', 'npx', '-y', 'pline-api'] }
    : { command: 'npx', args: ['-y', 'pline-api'] };
}

function vscodeUserDir() {
  if (process.platform === 'darwin') return path.join(home(), 'Library', 'Application Support', 'Code', 'User');
  if (WINDOWS) return path.join(appData(), 'Code', 'User');
  return path.join(configHome(), 'Code', 'User');
}

function claudeDesktopDir() {
  if (process.platform === 'darwin') return path.join(home(), 'Library', 'Application Support', 'Claude');
  if (WINDOWS) return path.join(appData(), 'Claude');
  return null; // Claude Desktop has no Linux build.
}

// Each client: how to detect it, how to register the server, and its skills.sh agent id (if any).
const CLIENTS = {
  'claude-code': {
    label: 'Claude Code',
    detect: () => onPath('claude'),
    skillAgent: 'claude-code',
    // Claude Code only accepts letters, digits, hyphens and underscores in server names.
    where: () => 'claude mcp add --scope user pline-ai',
    configure: (ctx) => configureCli(ctx, 'claude', 'pline-ai',
      ['mcp', 'remove', 'pline-ai', '--scope', 'user'],
      ['mcp', 'add', 'pline-ai', '--scope', 'user',
        '-e', `PLINE_BASE_URL=${ctx.baseUrl}`, '-e', `PLINE_API_KEY=${ctx.apiKey}`,
        '--', ...(WINDOWS ? ['cmd', '/c'] : []), 'npx', '-y', 'pline-api']),
  },
  codex: {
    label: 'Codex',
    detect: () => onPath('codex'),
    skillAgent: 'codex',
    where: () => 'codex mcp add pline-api (~/.codex/config.toml)',
    configure: (ctx) => configureCli(ctx, 'codex', 'pline-api',
      ['mcp', 'remove', 'pline-api'],
      ['mcp', 'add', 'pline-api',
        '--env', `PLINE_BASE_URL=${ctx.baseUrl}`, '--env', `PLINE_API_KEY=${ctx.apiKey}`,
        '--', 'npx', '-y', 'pline-api']),
  },
  cursor: {
    label: 'Cursor',
    detect: () => fs.existsSync(path.join(home(), '.cursor')),
    skillAgent: 'cursor',
    file: () => path.join(home(), '.cursor', 'mcp.json'),
    configure: (ctx) => configureFile(ctx, CLIENTS.cursor.file(), 'mcpServers', {
      ...serverCommand(),
      env: { PLINE_BASE_URL: ctx.baseUrl, PLINE_API_KEY: ctx.apiKey },
    }),
  },
  vscode: {
    label: 'VS Code',
    detect: () => fs.existsSync(vscodeUserDir()),
    skillAgent: 'github-copilot',
    needsKey: false,
    file: () => path.join(vscodeUserDir(), 'mcp.json'),
    // VS Code supports prompted inputs, so the key is asked for at first start and never stored here.
    configure: (ctx) => configureFile(ctx, CLIENTS.vscode.file(), 'servers', {
      type: 'stdio',
      ...serverCommand(),
      env: { PLINE_BASE_URL: ctx.baseUrl, PLINE_API_KEY: '${input:pline-api-key}' },
    }, [{ type: 'promptString', id: 'pline-api-key', description: 'pline.ai API key', password: true }]),
  },
  'claude-desktop': {
    label: 'Claude Desktop',
    detect: () => Boolean(claudeDesktopDir()) && fs.existsSync(claudeDesktopDir()),
    skillAgent: null, // Claude Desktop skills are uploaded in the app, not installed from disk.
    file: () => path.join(claudeDesktopDir(), 'claude_desktop_config.json'),
    configure: (ctx) => configureFile(ctx, CLIENTS['claude-desktop'].file(), 'mcpServers', {
      ...serverCommand(),
      env: { PLINE_BASE_URL: ctx.baseUrl, PLINE_API_KEY: ctx.apiKey },
    }),
  },
  windsurf: {
    label: 'Windsurf',
    detect: () => fs.existsSync(path.join(home(), '.codeium', 'windsurf')),
    skillAgent: 'windsurf',
    file: () => path.join(home(), '.codeium', 'windsurf', 'mcp_config.json'),
    configure: (ctx) => configureFile(ctx, CLIENTS.windsurf.file(), 'mcpServers', {
      ...serverCommand(),
      env: { PLINE_BASE_URL: ctx.baseUrl, PLINE_API_KEY: ctx.apiKey },
    }),
  },
};

function configureCli(ctx, command, name, removeArgs, addArgs) {
  if (ctx.dryRun) return `would run: ${command} ${redact(addArgs).join(' ')}`;
  // Replace any earlier pline entry so a re-run updates the key or URL; a missing entry is fine.
  run(command, removeArgs);
  const result = run(command, addArgs);
  if (result.error || result.status !== 0) {
    const detail = (result.error && result.error.message) || (result.stderr || result.stdout || '').trim();
    throw new Error(`${command} ${addArgs.slice(0, 3).join(' ')} failed: ${detail || `exit ${result.status}`}`);
  }
  return `registered ${name}`;
}

function redact(args) {
  return args.map((arg) => (arg.startsWith('PLINE_API_KEY=') ? 'PLINE_API_KEY=********' : arg));
}

function configureFile(ctx, file, section, entry, inputs) {
  let config = {};
  const exists = fs.existsSync(file);
  if (exists) {
    const text = fs.readFileSync(file, 'utf8');
    if (text.trim()) {
      try {
        config = JSON.parse(text);
      } catch (error) {
        // Comments or trailing commas: refuse rather than rewrite the user's file without them.
        throw new Error(`${file} is not plain JSON (${error.message}); add the pline.ai entry by hand`);
      }
    }
    if (!config || typeof config !== 'object' || Array.isArray(config)) {
      throw new Error(`${file} does not hold a JSON object; add the pline.ai entry by hand`);
    }
  }
  const servers = config[section] && typeof config[section] === 'object' ? config[section] : {};
  const action = servers['pline.ai'] ? 'updated' : 'added';
  config[section] = { ...servers, 'pline.ai': entry };
  if (inputs) {
    const ids = new Set(inputs.map((input) => input.id));
    config.inputs = [...(Array.isArray(config.inputs) ? config.inputs : []).filter((input) => !ids.has(input && input.id)), ...inputs];
  }
  if (ctx.dryRun) return `would ${action === 'added' ? 'add' : 'update'} pline.ai in ${file}`;

  fs.mkdirSync(path.dirname(file), { recursive: true });
  let backup = '';
  if (exists) {
    backup = `${file}.pline-backup-${new Date().toISOString().replace(/[:.]/g, '-')}`;
    fs.copyFileSync(file, backup);
  }
  const tmp = `${file}.pline-tmp-${process.pid}`;
  // The file may now hold the API key, so keep it private to the user.
  fs.writeFileSync(tmp, `${JSON.stringify(config, null, 2)}\n`, { mode: 0o600 });
  fs.renameSync(tmp, file);
  if (!WINDOWS) fs.chmodSync(file, 0o600);
  return `${action} pline.ai in ${file}${backup ? ` (backup: ${path.basename(backup)})` : ''}`;
}

function installSkill(ctx, agents) {
  const args = ['-y', 'skills', 'add', SKILL_SOURCE, '--skill', SKILL_NAME, ...agents.flatMap((a) => ['-a', a]), '-g', '-y'];
  if (ctx.dryRun) return `would run: npx ${args.join(' ')}`;
  const result = run('npx', args, { stdio: ['ignore', 'inherit', 'inherit'] });
  if (result.error || result.status !== 0) {
    throw new Error(`npx skills add failed${result.error ? `: ${result.error.message}` : ` (exit ${result.status})`}`);
  }
  return `installed the ${SKILL_NAME} skill for ${agents.join(', ')}`;
}

function parseArgs(argv) {
  const opts = { agents: [], mcp: true, skill: true, baseUrl: DEFAULT_BASE_URL, yes: false, dryRun: false, help: false };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const value = () => {
      const next = argv[i + 1];
      if (next === undefined || next.startsWith('-')) throw new Error(`${arg} needs a value`);
      i += 1;
      return next;
    };
    if (arg === '-a' || arg === '--agent') opts.agents.push(...value().split(',').map((s) => s.trim()).filter(Boolean));
    else if (arg === '--all') opts.agents = [];
    else if (arg === '--mcp-only') opts.skill = false;
    else if (arg === '--skill-only') opts.mcp = false;
    else if (arg === '--base-url') opts.baseUrl = value();
    else if (arg === '-y' || arg === '--yes') opts.yes = true;
    else if (arg === '--dry-run') opts.dryRun = true;
    else if (arg === '-h' || arg === '--help') opts.help = true;
    else throw new Error(`unknown option ${arg}`);
  }
  if (!opts.mcp && !opts.skill) throw new Error('--mcp-only and --skill-only cannot be combined');
  for (const id of opts.agents) {
    if (!CLIENTS[id]) throw new Error(`unknown agent "${id}"; use one of ${Object.keys(CLIENTS).join(', ')}`);
  }
  if (!/^https?:\/\/\S+$/.test(opts.baseUrl)) throw new Error(`--base-url must be an http(s) URL (got ${opts.baseUrl})`);
  return opts;
}

function ask(question) {
  const rl = readline.createInterface({ input: process.stdin, output: process.stderr });
  return new Promise((resolve) => rl.question(question, (answer) => { rl.close(); resolve(answer.trim()); }));
}

function askHidden(question) {
  return new Promise((resolve, reject) => {
    const input = process.stdin;
    // Turn off terminal echo before showing the prompt, so a key typed or pasted at once is not shown.
    input.setRawMode(true);
    process.stderr.write(question);
    let value = '';
    input.resume();
    input.setEncoding('utf8');
    const onData = (chunk) => {
      for (const ch of chunk) {
        if (ch === '\r' || ch === '\n') {
          finish();
          resolve(value.trim());
          return;
        }
        if (ch === '\u0003') {
          finish();
          reject(new Error('cancelled'));
          return;
        }
        if (ch === '\u007f' || ch === '\b') value = value.slice(0, -1);
        else value += ch;
      }
    };
    const finish = () => {
      input.removeListener('data', onData);
      input.setRawMode(false);
      input.pause();
      process.stderr.write('\n');
    };
    input.on('data', onData);
  });
}

async function init(argv) {
  const say = (message) => process.stderr.write(`${message}\n`);
  let opts;
  try {
    opts = parseArgs(argv);
  } catch (error) {
    say(`pline-api init: ${error.message}\n`);
    process.stderr.write(HELP);
    return 2;
  }
  if (opts.help) {
    process.stdout.write(HELP);
    return 0;
  }

  const ids = opts.agents.length ? [...new Set(opts.agents)] : Object.keys(CLIENTS).filter((id) => CLIENTS[id].detect());
  if (!ids.length) {
    say('pline-api init: found no supported MCP client (Claude Code, Codex, Cursor, VS Code, Claude Desktop, Windsurf).');
    say('Install one, or name it with --agent. Manual setup: https://github.com/grepsr/pline-api#quick-install');
    return 1;
  }
  const skillAgents = opts.skill ? [...new Set(ids.map((id) => CLIENTS[id].skillAgent).filter(Boolean))] : [];

  say('pline-api init will:');
  if (opts.mcp) {
    for (const id of ids) {
      const client = CLIENTS[id];
      say(`  - register the pline.ai MCP server with ${client.label} (${client.file ? client.file() : client.where()})`);
    }
  }
  if (skillAgents.length) say(`  - install the pline-api skill with skills.sh for: ${skillAgents.join(', ')}`);
  say(`  API base URL: ${opts.baseUrl}`);

  const interactive = Boolean(process.stdin.isTTY);
  if (!opts.yes && !opts.dryRun) {
    if (!interactive) {
      say('pline-api init: not a terminal; re-run with --yes to apply these changes.');
      return 1;
    }
    const answer = await ask('Continue? [Y/n] ');
    if (answer && !/^y(es)?$/i.test(answer)) {
      say('Nothing changed.');
      return 1;
    }
  }

  let apiKey = (process.env.PLINE_API_KEY || '').trim();
  const needsKey = opts.mcp && ids.some((id) => CLIENTS[id].needsKey !== false);
  if (needsKey && !apiKey && !opts.dryRun) {
    if (!interactive) {
      say('pline-api init: set PLINE_API_KEY to your pline.ai API key (no terminal to prompt in).');
      return 1;
    }
    try {
      apiKey = await askHidden('pline.ai API key (input hidden): ');
    } catch {
      say('Nothing changed.');
      return 1;
    }
    if (!apiKey) {
      say('pline-api init: no API key entered; nothing changed.');
      return 1;
    }
  }

  const ctx = { apiKey, baseUrl: opts.baseUrl, dryRun: opts.dryRun };
  const results = [];
  if (opts.mcp) {
    for (const id of ids) {
      try {
        results.push([true, `${CLIENTS[id].label}: ${CLIENTS[id].configure(ctx)}`]);
      } catch (error) {
        results.push([false, `${CLIENTS[id].label}: ${error.message}`]);
      }
    }
  }
  if (skillAgents.length) {
    try {
      results.push([true, `Skill: ${installSkill(ctx, skillAgents)}`]);
    } catch (error) {
      results.push([false, `Skill: ${error.message}`]);
    }
  }

  say('');
  for (const [ok, line] of results) say(`${ok ? '✓' : '✗'} ${line}`);
  const failed = results.filter(([ok]) => !ok).length;
  if (!opts.dryRun && opts.mcp) say('\nRestart your MCP clients to load pline.ai.');
  if (!opts.dryRun && skillAgents.length) {
    say('The skill uses the pline.ai MCP tools when connected, otherwise its Python CLI, which reads');
    say('PLINE_BASE_URL and PLINE_API_KEY from the shell environment.');
  }
  return failed ? 1 : 0;
}

module.exports = { init, parseArgs, CLIENTS, HELP };
