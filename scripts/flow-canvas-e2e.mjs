#!/usr/bin/env node
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(scriptDir, '..');
const rqbBin =
  process.env.RQB_E2E_BIN ||
  path.join(root, 'target', 'debug', process.platform === 'win32' ? 'rqb.exe' : 'rqb');
const timeoutMs = Number(process.env.RQB_E2E_TIMEOUT_MS || 30_000);
const keepWorkspace = process.env.RQB_E2E_KEEP_WORKSPACE === '1';
const artifactDir = process.env.RQB_E2E_ARTIFACT_DIR || path.join(root, 'target', 'e2e-artifacts', 'flow-canvas');

function fail(message, detail = '') {
  console.error(`flow canvas e2e failed: ${message}`);
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
    throw new Error(
      `Playwright is not installed. Run: cd web && npm install\n${error.message}`,
    );
  }
}

function createWorkspace(baseUrl) {
  const workspace = mkdtempSync(path.join(os.tmpdir(), 'rqb-flow-e2e-'));
  const apiDocs = path.join(workspace, 'api-docs');
  mkdirSync(path.join(apiDocs, '_shared'), { recursive: true });
  mkdirSync(path.join(apiDocs, 'apis', 'posts'), { recursive: true });
  mkdirSync(path.join(apiDocs, 'apis', 'users'), { recursive: true });
  mkdirSync(path.join(apiDocs, 'flows'), { recursive: true });

  writeFileSync(
    path.join(apiDocs, 'reqbook.md'),
    `---
name: flow-canvas-e2e
version: 1
default-env: dev
---
# Flow canvas E2E
`,
  );

  writeFileSync(
    path.join(apiDocs, '_shared', 'env.md'),
    `# Environments

## dev

\`\`\`yaml
baseUrl: ${baseUrl}
\`\`\`
`,
  );

  writeFileSync(
    path.join(apiDocs, 'apis', 'posts', 'create-post.md'),
    `---
resource: posts
protocol: http
method: POST
path: /posts
tags: [e2e]
version: 1
auth: none
timeout: 2000
---
# Create post

## Request

\`\`\`http
POST {{baseUrl}}/posts
Content-Type: application/json

{"title":"Reqbook E2E","userId":42}
\`\`\`

## Expected response

\`\`\`http
HTTP/1.1 201 Created
Content-Type: application/json

{"id":"post_123","userId":42}
\`\`\`
`,
  );

  writeFileSync(
    path.join(apiDocs, 'apis', 'users', 'get-user-by-id.md'),
    `---
resource: users
protocol: http
method: GET
path: /users/:id
tags: [e2e]
version: 1
auth: none
timeout: 2000
---
# Get user by id

## Request

\`\`\`http
GET {{baseUrl}}/users/:id
Accept: application/json
\`\`\`

## Expected response

\`\`\`http
HTTP/1.1 200 OK
Content-Type: application/json

{"id":42,"name":"Ada Lovelace"}
\`\`\`
`,
  );

  writeFileSync(
    path.join(apiDocs, 'flows', 'e2e-flow.md'),
    `---
type: pipeline
name: e2e-flow
description: Browser flow canvas E2E fixture
continue-on-error: false
parallel: false
---

# Flow canvas E2E

## Steps

1. **Create post** -> \`apis/posts/create-post.md\`
   - Capture: \`response.body.id\` as \`postId\`
   - Capture: \`response.body.userId\` as \`id\`
2. **Get user by id** -> \`apis/users/get-user-by-id.md\`
   - Inject: \`id\`
`,
  );

  return workspace;
}

function startFixtureApi() {
  const requests = [];
  const server = http.createServer((req, res) => {
    requests.push({ method: req.method, url: req.url });
    res.setHeader('Content-Type', 'application/json');

    if (req.method === 'POST' && req.url === '/posts') {
      res.statusCode = 201;
      res.end(JSON.stringify({ id: 'post_123', userId: 42 }));
      return;
    }

    if (req.method === 'GET' && req.url === '/users/42') {
      res.statusCode = 200;
      res.end(JSON.stringify({ id: 42, name: 'Ada Lovelace' }));
      return;
    }

    res.statusCode = 404;
    res.end(JSON.stringify({ error: 'not found' }));
  });

  return new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const address = server.address();
      resolve({
        server,
        requests,
        baseUrl: `http://127.0.0.1:${address.port}`,
      });
    });
  });
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

async function closeServer(server) {
  await new Promise((resolve) => server.close(resolve));
}

function resetArtifactDir() {
  rmSync(artifactDir, { recursive: true, force: true });
  mkdirSync(artifactDir, { recursive: true });
}

async function writeFailureArtifacts({ page, logs, fixture, workspace, error }) {
  resetArtifactDir();
  writeFileSync(path.join(artifactDir, 'error.txt'), `${error?.stack || error?.message || error}\n`);
  writeFileSync(path.join(artifactDir, 'preview.log'), stripAnsi(logs.join('')));
  writeFileSync(
    path.join(artifactDir, 'fixture-requests.json'),
    JSON.stringify(fixture?.requests ?? [], null, 2),
  );
  writeFileSync(
    path.join(artifactDir, 'meta.json'),
    JSON.stringify({
      workspace: workspace ?? null,
      rqb_bin: rqbBin,
      kept_workspace: keepWorkspace,
      generated_at: new Date().toISOString(),
    }, null, 2),
  );
  if (page) {
    try {
      await page.screenshot({ path: path.join(artifactDir, 'flow-canvas.png'), fullPage: true });
      writeFileSync(path.join(artifactDir, 'flow-canvas.html'), await page.content());
    } catch (screenshotError) {
      writeFileSync(
        path.join(artifactDir, 'screenshot-error.txt'),
        `${screenshotError?.stack || screenshotError?.message || screenshotError}\n`,
      );
    }
  }
}

const { chromium } = loadPlaywright();
let fixture;
let workspace;
let preview;
let browser;
let page;
let previewLogs = [];

try {
  if (!existsSync(rqbBin)) {
    throw new Error(`rqb binary not found at ${rqbBin}. Run: cargo build --locked`);
  }

  fixture = await startFixtureApi();
  workspace = createWorkspace(fixture.baseUrl);

  preview = spawn(rqbBin, ['serve', workspace, '--host', '127.0.0.1', '--port', '0', '--env', 'dev'], {
    cwd: root,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  preview.stdout.on('data', chunk => previewLogs.push(chunk.toString('utf8')));
  preview.stderr.on('data', chunk => previewLogs.push(chunk.toString('utf8')));

  const previewUrl = await waitForPreview(preview, previewLogs);

  browser = await chromium.launch({ headless: true });
  page = await browser.newPage({ viewport: { width: 1360, height: 900 } });
  await page.goto(`${previewUrl}/flows/flows/e2e-flow.md`, { waitUntil: 'networkidle' });
  await page.getByTestId('flow-canvas-page').waitFor({ timeout: 10_000 });

  const title = await page.getByTestId('flow-title-input').inputValue();
  assert(title === 'Flow canvas E2E', `unexpected flow title: ${title}`);
  assert(await page.getByTestId('flow-node').count() === 2, 'expected two flow nodes');

  await page.getByTestId('flow-run').click();
  await page.getByTestId('flow-run-summary').waitFor({ timeout: 15_000 });
  const summary = await page.getByTestId('flow-run-summary').innerText();
  assert(summary.includes('Passed'), `flow summary did not pass: ${summary}`);

  const statuses = await page.getByTestId('flow-node').evaluateAll(nodes =>
    nodes.map(node => node.getAttribute('data-node-status')),
  );
  assert(statuses.length === 2, `expected two node statuses, got ${statuses.length}`);
  assert(statuses.every(status => status === 'ok'), `expected all nodes ok, got ${statuses.join(', ')}`);

  const fixtureCalls = fixture.requests.map(request => `${request.method} ${request.url}`);
  assert(
    fixtureCalls.includes('POST /posts') && fixtureCalls.includes('GET /users/42'),
    `fixture API did not receive expected calls: ${fixtureCalls.join(', ')}`,
  );

  rmSync(artifactDir, { recursive: true, force: true });
  console.log(JSON.stringify({
    ok: true,
    preview_url: previewUrl,
    fixture_url: fixture.baseUrl,
    workspace,
    checks: ['open-flow-canvas', 'render-nodes', 'run-flow-click', 'ui-summary-passed', 'node-status-ok', 'captured-inject-hit-fixture'],
  }, null, 2));
} catch (error) {
  await writeFailureArtifacts({ page, logs: previewLogs, fixture, workspace, error });
  if (/Executable doesn't exist|browserType\.launch/.test(error.message)) {
    fail(`${error.message}\nRun: cd web && npx playwright install chromium`);
  } else {
    fail(`${error.message}\nArtifacts: ${artifactDir}`);
  }
} finally {
  if (browser) await browser.close().catch(() => {});
  if (preview) await stopChild(preview);
  if (fixture) await closeServer(fixture.server);
  if (workspace && !keepWorkspace) rmSync(workspace, { recursive: true, force: true });
}
