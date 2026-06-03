#!/usr/bin/env node
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, statSync } from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';

const root = process.cwd();
const bin = process.env.RQB_BENCH_BIN || path.join(root, 'target', 'release', process.platform === 'win32' ? 'rqb.exe' : 'rqb');
const iterations = Number(process.env.RQB_BENCH_ITERATIONS || 30);
const port = Number(process.env.RQB_BENCH_PORT || 7799);

function fail(message) {
  console.error(message);
  process.exit(1);
}

function run(label, command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: 'utf8',
    stdio: options.stdio || 'pipe',
  });
  if (result.status !== 0) {
    const stderr = result.stderr ? `\n${result.stderr}` : '';
    fail(`${label} failed with exit ${result.status}${stderr}`);
  }
  return result;
}

function ms(start) {
  return Number(process.hrtime.bigint() - start) / 1_000_000;
}

function stats(samples) {
  const sorted = [...samples].sort((a, b) => a - b);
  const sum = sorted.reduce((acc, value) => acc + value, 0);
  const pct = (p) => sorted[Math.min(sorted.length - 1, Math.floor((sorted.length - 1) * p))];
  return {
    min: sorted[0],
    mean: sum / sorted.length,
    p95: pct(0.95),
  };
}

function measureCommand(label, args, count = iterations) {
  const samples = [];
  for (let i = 0; i < count; i += 1) {
    const started = process.hrtime.bigint();
    const result = spawnSync(bin, args, { cwd: root, stdio: 'pipe' });
    if (result.status !== 0) {
      fail(`${label} failed on iteration ${i + 1}`);
    }
    samples.push(ms(started));
  }
  return stats(samples);
}

function requestOnce(url) {
  return new Promise((resolve, reject) => {
    const req = http.get(url, (res) => {
      res.resume();
      res.on('end', () => resolve(res.statusCode || 0));
    });
    req.on('error', reject);
    req.setTimeout(500, () => {
      req.destroy(new Error('timeout'));
    });
  });
}

async function measureWebFirstResponse() {
  const child = spawn(bin, ['serve', '--host', '127.0.0.1', '--port', String(port)], {
    cwd: root,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const started = process.hrtime.bigint();
  const deadline = Date.now() + 7000;
  let firstResponse = null;

  while (Date.now() < deadline) {
    try {
      const status = await requestOnce(`http://127.0.0.1:${port}/`);
      if (status >= 200 && status < 500) {
        firstResponse = ms(started);
        break;
      }
    } catch (_) {
      await new Promise((resolve) => setTimeout(resolve, 30));
    }
  }

  child.kill('SIGTERM');
  await new Promise((resolve) => child.once('exit', resolve));

  if (firstResponse == null) {
    fail('web first response failed: server did not answer within 7s');
  }
  return firstResponse;
}

function fmtMs(value) {
  return `${value.toFixed(value < 10 ? 2 : 1)} ms`;
}

function fmtBytes(bytes) {
  const mib = bytes / 1024 / 1024;
  return `${mib.toFixed(2)} MiB`;
}

if (!existsSync(bin)) {
  fail(`release binary not found at ${bin}. Run: cargo build --release --locked`);
}

run('version check', bin, ['version']);

const binarySize = statSync(bin).size;
const help = measureCommand('rqb --help', ['--help']);
const validateFile = measureCommand('rqb validate endpoint', ['validate', 'examples/jsonplaceholder/api-docs/posts/get-post-by-id.md']);
const validateProject = measureCommand('rqb validate collection', ['validate', 'examples/jsonplaceholder/api-docs/']);
const webFirstResponse = await measureWebFirstResponse();

const rows = [
  ['Default binary size', '< 10 MiB', fmtBytes(binarySize), 'release binary'],
  ['Cold start, `rqb --help` mean', '< 20 ms', fmtMs(help.mean), `${iterations} runs; p95 ${fmtMs(help.p95)}`],
  ['Validate one endpoint mean', '< 25 ms', fmtMs(validateFile.mean), `${iterations} runs; p95 ${fmtMs(validateFile.p95)}`],
  ['Validate example collection mean', '< 100 ms', fmtMs(validateProject.mean), `${iterations} runs; p95 ${fmtMs(validateProject.p95)}`],
  ['Web first response', '< 100 ms', fmtMs(webFirstResponse), `localhost:${port}`],
];

console.log(`# Reqbook benchmark results\n`);
console.log(`Date: ${new Date().toISOString()}`);
console.log(`Machine: ${os.type()} ${os.release()} ${os.arch()}`);
console.log(`Binary: ${bin}\n`);
console.log('| Metric | Target | Measured | Notes |');
console.log('|---|---:|---:|---|');
for (const row of rows) {
  console.log(`| ${row[0]} | ${row[1]} | ${row[2]} | ${row[3]} |`);
}
