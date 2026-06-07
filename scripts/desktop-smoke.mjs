#!/usr/bin/env node
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const root = process.cwd();
const desktopBin =
  process.env.RQB_DESKTOP_BIN ||
  path.join(root, 'target', 'debug', process.platform === 'win32' ? 'rqb-desktop.exe' : 'rqb-desktop');
const timeoutMs = Number(process.env.RQB_DESKTOP_TIMEOUT_MS || 30_000);
const keepApp = process.env.RQB_DESKTOP_SMOKE_KEEP_APP === '1';
const keepWorkspace = process.env.RQB_DESKTOP_SMOKE_KEEP_WORKSPACE === '1';

function fail(message, detail = '') {
  console.error(`desktop smoke failed: ${message}`);
  if (detail) {
    console.error(detail.trim());
  }
  process.exitCode = 1;
}

function stripAnsi(value) {
  return value.replace(/\x1b\[[0-9;]*m/g, '');
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function createSmokeWorkspace() {
  const workspace = mkdtempSync(path.join(os.tmpdir(), 'rqb-desktop-smoke-'));
  const apiDocs = path.join(workspace, 'api-docs');
  mkdirSync(path.join(apiDocs, '_shared'), { recursive: true });
  mkdirSync(path.join(apiDocs, 'apis', 'health'), { recursive: true });
  mkdirSync(path.join(apiDocs, 'flows'), { recursive: true });

  writeFileSync(
    path.join(apiDocs, 'reqbook.md'),
    `---
name: desktop-smoke
version: 1
default-env: dev
---
# Desktop smoke

Temporary Reqbook workspace used by scripts/desktop-smoke.mjs.
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
tags: [smoke]
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

  writeFileSync(
    path.join(apiDocs, 'flows', 'desktop-smoke-flow.md'),
    `---
type: pipeline
name: desktop-smoke-flow
description: Desktop smoke flow used by scripts/desktop-smoke.mjs
continue-on-error: false
parallel: false
---

# Desktop smoke flow

## Steps

1. **Health check** -> \`apis/health/get-health.md\`
`,
  );

  return workspace;
}

async function request(url, options = {}) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 5_000);
  try {
    const response = await fetch(url, {
      ...options,
      signal: controller.signal,
    });
    const text = await response.text();
    return { response, text };
  } finally {
    clearTimeout(timer);
  }
}

async function requestJson(url, options = {}) {
  const { response, text } = await request(url, options);
  if (!response.ok) {
    throw new Error(`${response.status} ${response.statusText}: ${text}`);
  }
  return text ? JSON.parse(text) : null;
}

async function requestText(url, options = {}) {
  const { response, text } = await request(url, options);
  if (!response.ok) {
    throw new Error(`${response.status} ${response.statusText}: ${text}`);
  }
  return { text, headers: response.headers };
}

async function requestStatus(url, options = {}) {
  const { response, text } = await request(url, options);
  return { status: response.status, text };
}

function firstSetCookie(headers) {
  if (typeof headers.getSetCookie === 'function') {
    const values = headers.getSetCookie();
    if (values.length > 0) return values[0];
  }
  return headers.get('set-cookie') || '';
}

function extractSessionCookie(headers) {
  const setCookie = firstSetCookie(headers);
  const cookie = setCookie.split(';')[0];
  assert(
    cookie.startsWith('rqb_write_token=') && cookie.length > 'rqb_write_token='.length,
    `desktop session cookie was not issued: ${setCookie}`,
  );
  return cookie;
}

async function waitForPreview(child, logs) {
  const deadline = Date.now() + timeoutMs;
  let serverUrl = null;

  while (Date.now() < deadline) {
    const logText = stripAnsi(logs.join(''));
    const match = logText.match(/Preview:\s+(http:\/\/127\.0\.0\.1:\d+)/);
    if (match) {
      serverUrl = match[1];
      break;
    }

    if (child.exitCode != null) {
      throw new Error(`desktop app exited before preview server was ready\n${logText}`);
    }

    await delay(50);
  }

  if (!serverUrl) {
    throw new Error(`timed out waiting for preview server\n${stripAnsi(logs.join(''))}`);
  }

  while (Date.now() < deadline) {
    try {
      await requestJson(`${serverUrl}/api/workspace/current`);
      return serverUrl;
    } catch (_) {
      await delay(100);
    }
  }

  throw new Error(`preview server did not answer /api/workspace/current at ${serverUrl}`);
}

async function stopChild(child) {
  if (keepApp || child.exitCode != null) {
    return;
  }

  try {
    if (process.platform !== 'win32' && child.pid) {
      process.kill(-child.pid, 'SIGTERM');
    } else {
      child.kill('SIGTERM');
    }
  } catch (_) {
    child.kill('SIGTERM');
  }

  await Promise.race([once(child, 'exit'), delay(3_000)]);

  if (child.exitCode == null) {
    try {
      if (process.platform !== 'win32' && child.pid) {
        process.kill(-child.pid, 'SIGKILL');
      } else {
        child.kill('SIGKILL');
      }
    } catch (_) {
      child.kill('SIGKILL');
    }
  }
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

if (typeof fetch !== 'function') {
  fail('Node.js 18+ is required because this script uses global fetch');
  process.exit();
}

if (!existsSync(desktopBin)) {
  fail(
    `desktop binary not found at ${desktopBin}`,
    'Run: cd web && npm ci && npm run build\nThen: cargo build --locked -p rqb-desktop',
  );
  process.exit();
}

const workspace = createSmokeWorkspace();
const logs = [];
const child = spawn(desktopBin, [], {
  cwd: root,
  detached: process.platform !== 'win32',
  env: { ...process.env, RQB_DESKTOP_SMOKE: '1' },
  stdio: ['ignore', 'pipe', 'pipe'],
});

child.stdout.on('data', (chunk) => {
  logs.push(chunk.toString('utf8'));
});
child.stderr.on('data', (chunk) => {
  logs.push(chunk.toString('utf8'));
});

try {
  const serverUrl = await waitForPreview(child, logs);
  const home = await requestText(`${serverUrl}/`);
  const sessionCookie = extractSessionCookie(home.headers);

  const blocked = await requestStatus(`${serverUrl}/api/workspace/open`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ path: workspace }),
  });
  assert(
    blocked.status === 403,
    `workspace open without desktop session should be forbidden, got ${blocked.status}: ${blocked.text}`,
  );

  const opened = await requestJson(`${serverUrl}/api/workspace/open`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Cookie: sessionCookie,
    },
    body: JSON.stringify({ path: workspace }),
  });
  assert(opened?.status === 'ok', `workspace open did not return ok: ${JSON.stringify(opened)}`);

  const current = await requestJson(`${serverUrl}/api/workspace/current`);
  assert(current?.path === workspace, `workspace/current path mismatch: ${JSON.stringify(current)}`);

  const index = await requestJson(`${serverUrl}/api/index`);
  assert(index?.project_name === 'desktop-smoke', `unexpected project name: ${JSON.stringify(index)}`);
  assert(Array.isArray(index?.groups), 'index.groups must be an array');
  assert(
    index.groups.some((group) => group.resource === 'health'),
    `index did not include the smoke endpoint: ${JSON.stringify(index.groups)}`,
  );

  const flows = await requestJson(`${serverUrl}/api/flows`);
  assert(Array.isArray(flows?.flows), 'flows.flows must be an array');
  assert(
    flows.flows.some((flow) => flow.name === 'desktop-smoke-flow'),
    `flows did not include the smoke flow: ${JSON.stringify(flows)}`,
  );

  console.log(JSON.stringify({
    ok: true,
    server_url: serverUrl,
    workspace,
    checks: [
      'spawn-desktop',
      'serve-spa',
      'desktop-session-cookie',
      'unauthenticated-write-blocked',
      'workspace-open',
      'workspace-current',
      'index',
      'flows',
    ],
  }, null, 2));
} catch (error) {
  fail(error.message, stripAnsi(logs.join('')));
} finally {
  await stopChild(child);
  if (!keepWorkspace) {
    rmSync(workspace, { recursive: true, force: true });
  }
}
