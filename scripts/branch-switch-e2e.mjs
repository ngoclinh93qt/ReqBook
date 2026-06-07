#!/usr/bin/env node
import { execFileSync, spawn } from 'node:child_process';
import { once } from 'node:events';
import { createRequire } from 'node:module';
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(scriptDir, '..');
const rqbBin =
  process.env.RQB_E2E_BIN ||
  path.join(root, 'target', 'debug', process.platform === 'win32' ? 'rqb.exe' : 'rqb');
const timeoutMs = Number(process.env.RQB_E2E_TIMEOUT_MS || 30_000);
const keepWorkspace = process.env.RQB_E2E_KEEP_WORKSPACE === '1';
const artifactDir = process.env.RQB_E2E_ARTIFACT_DIR || path.join(root, 'target', 'e2e-artifacts', 'branch-switch');

function fail(message, detail = '') {
  console.error(`branch switch e2e failed: ${message}`);
  if (detail) console.error(detail.trim());
  process.exitCode = 1;
}

function stripAnsi(value) {
  return value.replace(/\x1b\[[0-9;]*m/g, '');
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function loadPlaywright() {
  try {
    const require = createRequire(import.meta.url);
    const resolved = require.resolve('playwright', { paths: [path.join(root, 'web')] });
    return require(resolved);
  } catch (error) {
    throw new Error(`Playwright is not installed. Run: cd web && npm install\n${error.message}`);
  }
}

function runGit(cwd, args) {
  return execFileSync('git', args, {
    cwd,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
}

function writeWorkspaceFiles(workspace, projectName) {
  const apiDocs = path.join(workspace, 'api-docs');
  mkdirSync(path.join(apiDocs, '_shared'), { recursive: true });
  mkdirSync(path.join(apiDocs, 'apis', 'health'), { recursive: true });
  writeFileSync(
    path.join(apiDocs, 'reqbook.md'),
    `---
name: ${projectName}
version: 1
default-env: dev
---
# ${projectName}
`,
  );
  writeFileSync(
    path.join(apiDocs, '_shared', 'env.md'),
    `# Environments

## dev

\`\`\`yaml
baseUrl: http://127.0.0.1:9
\`\`\`
`,
  );
  writeFileSync(
    path.join(apiDocs, 'apis', 'health', 'get-health.md'),
    `---
resource: health
protocol: http
method: GET
path: /health
tags: [e2e]
version: 1
auth: none
timeout: 1000
---
# Health check

## Request

\`\`\`http
GET {{baseUrl}}/health
\`\`\`

## Expected response

\`\`\`http
HTTP/1.1 200 OK
Content-Type: application/json

{"ok":true}
\`\`\`
`,
  );
}

function addFeatureEndpoint(workspace) {
  const apiDocs = path.join(workspace, 'api-docs');
  mkdirSync(path.join(apiDocs, 'apis', 'users'), { recursive: true });
  writeWorkspaceFiles(workspace, 'branch-switch-feature');
  writeFileSync(
    path.join(apiDocs, 'apis', 'users', 'get-users.md'),
    `---
resource: users
protocol: http
method: GET
path: /users
tags: [e2e]
version: 1
auth: none
timeout: 1000
---
# List users

## Request

\`\`\`http
GET {{baseUrl}}/users
\`\`\`

## Expected response

\`\`\`http
HTTP/1.1 200 OK
Content-Type: application/json

[]
\`\`\`
`,
  );
}

function createWorkspace() {
  const workspace = mkdtempSync(path.join(os.tmpdir(), 'rqb-branch-e2e-'));
  writeWorkspaceFiles(workspace, 'branch-switch-main');
  runGit(workspace, ['init', '-b', 'main']);
  runGit(workspace, ['config', 'user.email', 'e2e@example.com']);
  runGit(workspace, ['config', 'user.name', 'Reqbook E2E']);
  runGit(workspace, ['add', 'api-docs']);
  runGit(workspace, ['commit', '-m', 'main api docs']);
  runGit(workspace, ['switch', '-c', 'feature/branch-e2e']);
  addFeatureEndpoint(workspace);
  runGit(workspace, ['add', 'api-docs']);
  runGit(workspace, ['commit', '-m', 'feature api docs']);
  runGit(workspace, ['switch', 'main']);
  writeFileSync(path.join(workspace, 'api-docs', 'reqbook.md'), `---
name: dirty-main
version: 1
default-env: dev
---
# Dirty local edit
`);
  return workspace;
}

async function waitForPreview(child, logs) {
  const deadline = Date.now() + timeoutMs;
  let previewUrl = null;
  while (Date.now() < deadline) {
    const logText = stripAnsi(logs.join(''));
    const match = logText.match(/Preview:\s+(http:\/\/127\.0\.0\.1:\d+)/);
    if (match) {
      previewUrl = match[1];
      break;
    }
    if (child.exitCode != null) {
      throw new Error(`preview exited before it was ready\n${logText}`);
    }
    await delay(50);
  }
  if (!previewUrl) {
    throw new Error(`timed out waiting for preview server\n${stripAnsi(logs.join(''))}`);
  }
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${previewUrl}/api/index`);
      if (response.ok) return previewUrl;
    } catch (_) {
      await delay(100);
    }
  }
  throw new Error(`preview server did not answer /api/index at ${previewUrl}`);
}

async function stopChild(child) {
  if (child.exitCode != null) return;
  child.kill('SIGTERM');
  await Promise.race([once(child, 'exit'), delay(3_000)]);
  if (child.exitCode == null) child.kill('SIGKILL');
}

function resetArtifactDir() {
  rmSync(artifactDir, { recursive: true, force: true });
  mkdirSync(artifactDir, { recursive: true });
}

async function writeFailureArtifacts({ page, logs, workspace, error }) {
  resetArtifactDir();
  writeFileSync(path.join(artifactDir, 'error.txt'), `${error?.stack || error?.message || error}\n`);
  writeFileSync(path.join(artifactDir, 'preview.log'), stripAnsi(logs.join('')));
  writeFileSync(path.join(artifactDir, 'git-status.txt'), runGit(workspace, ['status', '--short']).toString());
  if (page) {
    await page.screenshot({ path: path.join(artifactDir, 'page.png'), fullPage: true }).catch(() => {});
  }
}

if (typeof fetch !== 'function') {
  fail('Node.js 18+ is required because this script uses global fetch');
  process.exit();
}

if (!existsSync(rqbBin)) {
  fail(`rqb binary not found at ${rqbBin}`, 'Run: cargo build --locked');
  process.exit();
}

const workspace = createWorkspace();
const logs = [];
const child = spawn(rqbBin, ['serve', workspace, '--port=0'], {
  cwd: root,
  stdio: ['ignore', 'pipe', 'pipe'],
});
child.stdout.on('data', (chunk) => logs.push(chunk.toString()));
child.stderr.on('data', (chunk) => logs.push(chunk.toString()));

let browser;
let page;

try {
  const { chromium } = loadPlaywright();
  const previewUrl = await waitForPreview(child, logs);
  browser = await chromium.launch({ headless: true });
  page = await browser.newPage({ viewport: { width: 1280, height: 820 } });
  const dialogs = [];
  page.on('dialog', async (dialog) => {
    dialogs.push(dialog.message());
    await dialog.accept();
  });

  await page.goto(previewUrl, { waitUntil: 'domcontentloaded' });
  await page.waitForSelector('[data-testid="git-branch-current"]', { timeout: timeoutMs });
  await page.getByTestId('git-branch-current').click();
  await page.getByTestId('git-branch-option-feature/branch-e2e').click();
  await page.waitForSelector('[data-testid="git-branch-error"]', { timeout: timeoutMs });
  assert(dialogs.length >= 1, 'dirty worktree checkout did not ask for confirmation');
  assert(
    (await page.getByTestId('git-branch-error').innerText()).length > 0,
    'checkout error was not rendered in the status bar',
  );

  runGit(workspace, ['restore', 'api-docs/reqbook.md']);
  await page.getByTestId('git-branch-current').click();
  await page.waitForTimeout(300);
  await page.getByTestId('git-branch-option-feature/branch-e2e').click();
  await page.waitForFunction(
    () => document.querySelector('[data-testid="git-branch-current"]')?.textContent?.includes('feature/branch-e2e'),
    null,
    { timeout: timeoutMs },
  );

  const index = await (await fetch(`${previewUrl}/api/index`)).json();
  assert(index.spec_count === 2, `expected branch index to expose 2 specs, got ${index.spec_count}`);
  await page.waitForFunction(
    () => Array.from(document.querySelectorAll('.sb-group-name')).some((el) => el.textContent === 'users'),
    null,
    { timeout: timeoutMs },
  );

  console.log('branch switch e2e passed');
  console.log('- dirty worktree confirmation');
  console.log('- checkout error surfaced');
  console.log('- successful checkout refreshed index/sidebar');
} catch (error) {
  fail(error.message);
  await writeFailureArtifacts({ page, logs, workspace, error }).catch((artifactError) => {
    console.error(`failed to write artifacts: ${artifactError.message}`);
  });
} finally {
  if (browser) await browser.close().catch(() => {});
  await stopChild(child);
  if (!keepWorkspace) rmSync(workspace, { recursive: true, force: true });
  else console.log(`kept workspace: ${workspace}`);
}
