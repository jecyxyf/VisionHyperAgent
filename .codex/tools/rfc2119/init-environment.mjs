#!/usr/bin/env node
// This bootstrap uses only the tool directory's pinned package archives, never a registry.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import {
  cpSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync,
  readlinkSync, realpathSync, renameSync, rmSync, writeFileSync,
} from 'node:fs';
import { delimiter, dirname, join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';

export const LOCAL_CLI = 'node .codex/tools/rfc2119/node_modules/rfc2119/dist/cli.js';
const ASSETS = resolve(dirname(fileURLToPath(import.meta.url)), 'offline');
const VERSIONS = { rfc2119: '0.7.0', picomatch: '4.0.7', yaml: '2.9.1' };
const PATCH_VERSION = 1;
// Planning prose and progress tables are not RFC2119 requirement documents.
const DEFAULT_CONFIG = 'specs: ["specs/requirements/**/*.md"]\ntests: ["test/**", "tests/**", "**/*.test.*"]\n';

export function checkRuntime(nodeVersion, npmVersion) {
  if (Number(nodeVersion.split('.')[0]) < 20 || !/^\d+\./.test(nodeVersion)) {
    throw new Error('需要已安装 Node.js >=20；不会联网安装运行时。');
  }
  if (Number(npmVersion.split('.')[0]) < 9 || !/^\d+\./.test(npmVersion)) {
    throw new Error('需要已安装 npm >=9；不会联网安装运行时。');
  }
}

function findNpmCli() {
  // Invoke npm's JS entry with this Node, including on Windows; avoid shell quoting.
  const candidates = [
    join(dirname(process.execPath), 'node_modules/npm/bin/npm-cli.js'),
    resolve(dirname(process.execPath), '../lib/node_modules/npm/bin/npm-cli.js'),
  ];
  for (const dir of (process.env.PATH ?? '').split(delimiter)) {
    const launcher = join(dir, 'npm');
    if (existsSync(launcher)) candidates.push(realpathSync(launcher));
    candidates.push(join(dir, 'node_modules/npm/bin/npm-cli.js'));
  }
  const cli = candidates.find((p) => p.endsWith('npm-cli.js') && existsSync(p));
  if (!cli) throw new Error('找不到 npm-cli.js；请先离线配置 Node.js/npm。');
  return cli;
}

function checkBrainstorming(skillDir) {
  const path = join(resolve(skillDir), 'SKILL.md');
  const content = readFileSync(path, 'utf8');
  const header = content.match(/^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/);
  if (!header || !/^name:\s*(?:brainstorming|"brainstorming"|'brainstorming')\s*$/m.test(header[1])) {
    throw new Error(`brainstorming 技能无效：${path}；不会下载或覆盖现有技能。`);
  }
  if (!content.slice(header[0].length).trim()) {
    throw new Error(`brainstorming 技能正文为空：${path}`);
  }
  return path;
}

function loadBundle() {
  const manifest = readFileSync(join(ASSETS, 'package.json'), 'utf8');
  const lockText = readFileSync(join(ASSETS, 'package-lock.json'), 'utf8');
  const lock = JSON.parse(lockText);
  const expected = Object.keys(VERSIONS).map((n) => `node_modules/${n}`).sort();
  if (JSON.stringify(Object.keys(lock.packages).filter(Boolean).sort()) !== JSON.stringify(expected)
      || JSON.parse(manifest).dependencies?.rfc2119 !== VERSIONS.rfc2119) {
    throw new Error('离线清单不符合固定版本；拒绝继续。');
  }
  const digest = createHash('sha256').update(manifest).update(lockText);
  const archives = Object.entries(VERSIONS).map(([name, version]) => {
    const entry = lock.packages[`node_modules/${name}`];
    const path = join(ASSETS, `${name}-${version}.tgz`);
    const bytes = readFileSync(path);
    const integrity = `sha512-${createHash('sha512').update(bytes).digest('base64')}`;
    if (entry.version !== version || entry.integrity !== integrity) {
      throw new Error(`离线包校验失败：${path}；不会回退在线下载。`);
    }
    digest.update(bytes);
    return path;
  });
  return { manifest, lockText, archives, digest: digest.digest('hex') };
}

function treeHash(root) {
  const hash = createHash('sha256');
  function visit(dir, prefix = '') {
    for (const name of readdirSync(dir).sort()) {
      const path = join(dir, name);
      const rel = `${prefix}/${name}`;
      const stat = lstatSync(path);
      hash.update(`${rel}\0${stat.mode}\0`);
      if (stat.isSymbolicLink()) hash.update(`link:${readlinkSync(path)}`);
      else if (stat.isDirectory()) visit(path, rel);
      else hash.update(readFileSync(path));
      hash.update('\0');
    }
  }
  visit(root);
  return hash.digest('hex');
}

function replaceOnce(source, before, after) {
  if (source.split(before).length !== 2) throw new Error(`离线补丁不适配：${before}`);
  return source.replace(before, after);
}

function patchRuntime(toolDir) {
  // Keep upstream tarballs intact; apply a documented patch only to the installed copy.
  const dist = join(toolDir, 'node_modules/rfc2119/dist');
  for (const name of ['hook.js', 'review.js', 'adapters.js']) {
    const path = join(dist, name);
    let source = readFileSync(path, 'utf8');
    if (name === 'hook.js') {
      source = replaceOnce(source, 'const notice = await upgradeNotice();',
        'const notice = null; // Offline deployment: do not check for updates.');
      source = source.replace('it must exit 0, and CI runs the same command.',
        'it must exit 0. CI integration is configured separately.');
    }
    // Generated review packets and integrations must not send agents back to npx.
    source = source.replaceAll('npx --yes ${PINNED}', LOCAL_CLI)
      .replaceAll('npx ${PINNED}', LOCAL_CLI).replaceAll('npx rfc2119', LOCAL_CLI);
    source = source.replace(/^\/\/# sourceMappingURL=.*$/m, '// Local offline patch; upstream source map does not apply.');
    writeFileSync(path, source);
  }
}

export function mergeHooks(original) {
  const next = structuredClone(original);
  if (!next || typeof next !== 'object' || Array.isArray(next)) throw new Error('hooks.json 需要 JSON 对象。');
  next.hooks ??= {};
  if (typeof next.hooks !== 'object' || Array.isArray(next.hooks)) throw new Error('hooks 配置无效。');
  const events = { SessionStart: 'session-start', PostToolUse: 'after-edit', Stop: 'stop' };
  for (const [event, action] of Object.entries(events)) {
    const groups = next.hooks[event] ?? [];
    if (!Array.isArray(groups)) throw new Error(`hooks.${event} 需要数组。`);
    const command = `${LOCAL_CLI} hook ${action} --platform codex`;
    const legacy = new RegExp(`^npx (?:--yes )?rfc2119(?:@[^ ]+)? hook ${action} --platform codex$`);
    let found = false;
    next.hooks[event] = groups.flatMap((group) => {
      if (!group || !Array.isArray(group.hooks)) throw new Error(`hooks.${event} 分组格式无效。`);
      let managed = false;
      const hooks = group.hooks.filter((hook) => {
        if (hook.type !== 'command' || !(hook.command === command || legacy.test(hook.command ?? ''))) return true;
        managed = true;
        if (found) return false;
        found = true;
        hook.command = command;
        return true;
      });
      if (managed && event === 'PostToolUse' && !['Edit|Write', 'Write|Edit', 'apply_patch|Write|Edit'].includes(group.matcher)) {
        // Never expand a matcher shared with an unrelated command.
        if (hooks.some((h) => h.command !== command)) throw new Error('2119 与其他 hook 共用不兼容 matcher；请先人工拆分。');
        group.matcher = 'Edit|Write';
      }
      if (!managed) return [group];
      return hooks.length ? [{ ...group, hooks }] : [];
    });
    if (!found) next.hooks[event].push({
      ...(event === 'PostToolUse' ? { matcher: 'Edit|Write' } : {}),
      hooks: [{ type: 'command', command }],
    });
  }
  return next;
}

function writeIfChanged(path, text) {
  if (existsSync(path) && readFileSync(path, 'utf8') === text) return;
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
}

function hasExistingSpecs(dir) {
  if (!existsSync(dir)) return false;
  return readdirSync(dir, { withFileTypes: true }).some((entry) =>
    entry.isSymbolicLink() || (entry.isDirectory() ? hasExistingSpecs(join(dir, entry.name)) : entry.name.endsWith('.md')));
}

export function initializeEnvironment(project, brainstorming) {
  const root = realpathSync(resolve(project));
  if (!lstatSync(root).isDirectory()) throw new Error('目标项目根目录无效。');
  const skillPath = checkBrainstorming(brainstorming);
  const npmCli = findNpmCli();
  const npmVersion = execFileSync(process.execPath, [npmCli, '--version'], { encoding: 'utf8', timeout: 10000 }).trim();
  checkRuntime(process.versions.node, npmVersion);
  // Validate all inputs before changing an existing installation or hook configuration.
  for (const rel of ['.codex', '.codex/tools', '.codex/tools/rfc2119', '.codex/hooks.json', '.2119.yml', '.gitignore',
    '.codex/tools/rfc2119/package.json', '.codex/tools/rfc2119/package-lock.json', '.codex/tools/rfc2119/.offline-install.json']) {
    const stat = lstatSync(join(root, rel), { throwIfNoEntry: false });
    if (stat?.isSymbolicLink()) throw new Error(`受管理路径是符号链接：${rel}；请先人工核查，避免改动目标项目以外的文件。`);
  }
  const bundle = loadBundle();
  const toolDir = join(root, '.codex/tools/rfc2119');
  const hookPath = join(root, '.codex/hooks.json');
  const originalHooks = existsSync(hookPath) ? JSON.parse(readFileSync(hookPath, 'utf8')) : {};
  const nextHooks = mergeHooks(originalHooks);
  for (const [file, expected] of [['package.json', bundle.manifest], ['package-lock.json', bundle.lockText]]) {
    if (existsSync(join(toolDir, file)) && JSON.stringify(JSON.parse(readFileSync(join(toolDir, file), 'utf8'))) !== JSON.stringify(JSON.parse(expected))) {
      throw new Error(`${join(toolDir, file)} 与离线清单冲突；不覆盖用户配置。`);
    }
  }
  const configPath = join(root, '.2119.yml');
  if (!existsSync(configPath) && hasExistingSpecs(join(root, 'specs'))) {
    throw new Error('已有 specs 文档但没有 .2119.yml；请先确认正式需求的扫描范围，不自动排除已有需求。');
  }
  const configText = existsSync(configPath) ? readFileSync(configPath, 'utf8') : DEFAULT_CONFIG;
  const statePath = join(toolDir, '.offline-install.json');
  let reusable = false;
  try {
    const state = JSON.parse(readFileSync(statePath, 'utf8'));
    reusable = state.bundle === bundle.digest && state.patch === PATCH_VERSION
      && state.tree === treeHash(join(toolDir, 'node_modules'));
  } catch { /* Absent or damaged installations are repaired from local archives. */ }

  // A private staging area and empty npm config isolate us from global caches and proxies.
  const stage = mkdtempSync(join(tmpdir(), 'rfc2119-offline-'));
  try {
    const stagedTool = join(stage, 'tool');
    mkdirSync(stagedTool);
    const npmConfig = join(stage, 'npmrc');
    const npmGlobalConfig = join(stage, 'global-npmrc');
    writeFileSync(npmConfig, '');
    writeFileSync(npmGlobalConfig, '');
    const env = { ...process.env, npm_config_userconfig: npmConfig,
      npm_config_globalconfig: npmGlobalConfig, npm_config_update_notifier: 'false' };
    const runNpm = (args) => execFileSync(process.execPath, [npmCli, ...args,
      '--offline', '--ignore-scripts', '--no-audit', '--no-fund', '--update-notifier=false',
      '--global=false', '--prefix', stagedTool, '--cache', join(stage, 'cache')],
    { cwd: stagedTool, env, timeout: 60000, stdio: 'pipe' });
    if (!reusable) {
      writeFileSync(join(stagedTool, 'package.json'), bundle.manifest);
      writeFileSync(join(stagedTool, 'package-lock.json'), bundle.lockText);
      for (const archive of bundle.archives) runNpm(['cache', 'add', archive]);
      runNpm(['ci']);
      patchRuntime(stagedTool);
    }
    const installedTool = reusable ? toolDir : stagedTool;
    const cli = join(installedTool, 'node_modules/rfc2119/dist/cli.js');
    // Smoke-test against the project's actual config, without executing its verify commands.
    const probe = join(stage, 'probe');
    mkdirSync(probe);
    writeFileSync(join(probe, '.2119.yml'), configText);
    execFileSync(process.execPath, [cli, '--help'], { cwd: probe, timeout: 10000 });
    const session = JSON.parse(execFileSync(process.execPath,
      [cli, 'hook', 'session-start', '--platform', 'codex'], { cwd: probe, input: '{}', encoding: 'utf8', timeout: 10000 }));
    if (!session.hookSpecificOutput?.additionalContext?.includes(LOCAL_CLI) || session.systemMessage) {
      throw new Error('离线 session-start 自检失败；保留原有安装。');
    }
    // Exercise the remaining entrypoints only against an empty controlled project.
    writeFileSync(join(probe, '.2119.yml'), DEFAULT_CONFIG);
    for (const event of ['after-edit', 'stop']) {
      const result = JSON.parse(execFileSync(process.execPath, [cli, 'hook', event, '--platform', 'codex'],
        { cwd: probe, input: '{}', encoding: 'utf8', timeout: 10000 }));
      if (result.systemMessage || result.decision) throw new Error(`离线 ${event} 自检失败；保留原有安装。`);
    }
    if (!reusable) {
      mkdirSync(toolDir, { recursive: true });
      const modules = join(toolDir, 'node_modules');
      const backup = join(toolDir, '.node_modules-offline-backup');
      if (existsSync(backup)) throw new Error(`存在未清理的安装备份：${backup}；请先核查。`);
      // Stage on the target filesystem before the atomic replacement (tmp may be another mount).
      const ready = join(toolDir, '.node_modules-offline-ready');
      if (existsSync(ready)) throw new Error(`存在未完成的安装目录：${ready}；请先核查。`);
      cpSync(join(stagedTool, 'node_modules'), ready, { recursive: true, verbatimSymlinks: true });
      if (existsSync(modules)) renameSync(modules, backup);
      try { renameSync(ready, modules); }
      catch (error) { if (existsSync(backup)) renameSync(backup, modules); throw error; }
      rmSync(backup, { recursive: true, force: true });
      writeIfChanged(join(toolDir, 'package.json'), bundle.manifest);
      writeIfChanged(join(toolDir, 'package-lock.json'), bundle.lockText);
      writeIfChanged(statePath, `${JSON.stringify({ bundle: bundle.digest, patch: PATCH_VERSION, tree: treeHash(modules) }, null, 2)}\n`);
    }
    if (!existsSync(configPath)) writeIfChanged(configPath, configText);
    if (JSON.stringify(nextHooks) !== JSON.stringify(originalHooks)) writeIfChanged(hookPath, `${JSON.stringify(nextHooks, null, 2)}\n`);
    const ignorePath = join(root, '.gitignore');
    let ignore = existsSync(ignorePath) ? readFileSync(ignorePath, 'utf8') : '';
    for (const pattern of ['.codex/tools/rfc2119/node_modules/', '.codex/tools/rfc2119/.offline-install.json', '.codex/tools/rfc2119/.node_modules-offline-*']) {
      if (!ignore.split(/\r?\n/).includes(pattern)) ignore += `${ignore && !ignore.endsWith('\n') ? '\n' : ''}${pattern}\n`;
    }
    writeIfChanged(ignorePath, ignore);
    return { brainstorming: { path: skillPath, status: 'present-needs-agent-load' },
      rfc2119: { version: VERSIONS.rfc2119, status: reusable ? 'reused' : 'installed', offline: true, updates: 'disabled' },
      hooks: { status: 'configured', smokeTest: 'passed', activation: 'requires-host-verification' },
      requiredNextSteps: ['load-brainstorming', 'verify-hooks-feature', 'verify-user-trust', 'observe-host-events'],
      checkCommand: `${LOCAL_CLI} check` };
  } finally { rmSync(stage, { recursive: true, force: true }); }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const args = process.argv.slice(2);
    const value = (name) => args[args.indexOf(name) + 1];
    if (args.length !== 4 || !args.includes('--project') || !args.includes('--brainstorming')) {
      throw new Error('用法：node init-environment.mjs --project <项目根目录> --brainstorming <技能目录>');
    }
    console.log(JSON.stringify(initializeEnvironment(value('--project'), value('--brainstorming')), null, 2));
  } catch (error) {
    console.error(`环境初始化失败：${error.message}`);
    process.exitCode = 1;
  }
}
