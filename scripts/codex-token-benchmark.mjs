#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const root = process.cwd();
const codexBin = process.env.CODEX_BIN || 'codex';
const runs = Number(
  process.env.RQB_AGENT_TOKEN_BENCH_RUNS || process.env.RQB_CODEX_TOKEN_BENCH_RUNS || 1,
);
const timeoutMs = Number(
  process.env.RQB_AGENT_TOKEN_BENCH_TIMEOUT_MS ||
    process.env.RQB_CODEX_TOKEN_BENCH_TIMEOUT_MS ||
    180_000,
);
const model = process.env.CODEX_MODEL;
const dryRun = process.argv.includes('--dry-run');
const scenariosDir = path.join(root, 'benchmarks', 'token', 'codex', 'scenarios');
const outRoot = path.join(root, 'target', 'token-benchmarks', 'codex');
const stamp = new Date().toISOString().replace(/[:.]/g, '-');
const outDir = path.join(outRoot, stamp);

const tokenKeys = {
  input_tokens: ['input_tokens', 'prompt_tokens', 'inputTokens', 'promptTokens'],
  output_tokens: ['output_tokens', 'completion_tokens', 'outputTokens', 'completionTokens'],
  total_tokens: ['total_tokens', 'totalTokens'],
  cached_tokens: [
    'cached_tokens',
    'cached_input_tokens',
    'cache_read_input_tokens',
    'cachedTokens',
    'cachedInputTokens',
  ],
  reasoning_tokens: [
    'reasoning_tokens',
    'reasoning_output_tokens',
    'reasoningTokens',
    'reasoningOutputTokens',
  ],
};

const scenarioLabels = {
  'source-only': 'Without Reqbook (source only)',
  reqbook: 'With Reqbook specs',
};

function fail(message) {
  console.error(message);
  process.exit(1);
}

function readScenario(name) {
  const file = path.join(scenariosDir, `${name}.md`);
  if (!existsSync(file)) {
    fail(`missing scenario file: ${file}`);
  }
  return readFileSync(file, 'utf8');
}

function numberFor(object, keys) {
  for (const key of keys) {
    const value = object[key];
    if (typeof value === 'number' && Number.isFinite(value)) {
      return value;
    }
  }
  return undefined;
}

function normalizeUsage(object) {
  const usage = {};
  for (const [field, keys] of Object.entries(tokenKeys)) {
    const value = numberFor(object, keys);
    if (value !== undefined) {
      usage[field] = value;
    }
  }

  if (object.input_tokens_details && typeof object.input_tokens_details === 'object') {
    const cached = numberFor(object.input_tokens_details, ['cached_tokens', 'cachedTokens']);
    if (cached !== undefined) {
      usage.cached_tokens = cached;
    }
  }

  if (object.output_tokens_details && typeof object.output_tokens_details === 'object') {
    const reasoning = numberFor(object.output_tokens_details, [
      'reasoning_tokens',
      'reasoningTokens',
    ]);
    if (reasoning !== undefined) {
      usage.reasoning_tokens = reasoning;
    }
  }

  const hasUsage = Object.values(usage).some((value) => typeof value === 'number');
  if (!hasUsage) {
    return null;
  }

  if (usage.total_tokens === undefined) {
    const input = usage.input_tokens || 0;
    const output = usage.output_tokens || 0;
    if (input || output) {
      usage.total_tokens = input + output;
    }
  }

  return usage;
}

function collectUsageCandidates(value, pathParts = []) {
  if (!value || typeof value !== 'object') {
    return [];
  }

  const candidates = [];
  const direct = normalizeUsage(value);
  if (direct) {
    candidates.push({ path: pathParts.join('.') || '$', usage: direct });
  }

  if (Array.isArray(value)) {
    value.forEach((item, index) => {
      candidates.push(...collectUsageCandidates(item, [...pathParts, String(index)]));
    });
  } else {
    for (const [key, child] of Object.entries(value)) {
      candidates.push(...collectUsageCandidates(child, [...pathParts, key]));
    }
  }

  return candidates;
}

function parseUsage(stdout) {
  const lines = stdout.split(/\r?\n/).filter((line) => line.trim());
  const candidates = [];

  lines.forEach((line, lineIndex) => {
    try {
      const event = JSON.parse(line);
      for (const candidate of collectUsageCandidates(event)) {
        candidates.push({ ...candidate, line: lineIndex + 1 });
      }
    } catch (_) {
      // Codex --json should be JSONL, but keep raw logs even if a warning slips in.
    }
  });

  if (candidates.length === 0) {
    return null;
  }

  candidates.sort((a, b) => {
    const aTotal = a.usage.total_tokens || 0;
    const bTotal = b.usage.total_tokens || 0;
    if (aTotal !== bTotal) {
      return bTotal - aTotal;
    }
    return b.line - a.line;
  });

  return candidates[0];
}

function parseErrorMessage(stdout, stderr) {
  const messages = [];
  const lines = stdout.split(/\r?\n/).filter((line) => line.trim());
  for (const line of lines) {
    try {
      const event = JSON.parse(line);
      if (typeof event.message === 'string' && event.type === 'error') {
        messages.push(event.message);
      }
      if (event.error && typeof event.error.message === 'string') {
        messages.push(event.error.message);
      }
    } catch (_) {
      // Keep scanning JSONL; stderr is handled below.
    }
  }

  if (messages.length > 0) {
    return messages[messages.length - 1];
  }

  const stderrLines = stderr.split(/\r?\n/).filter((line) => line.trim());
  return stderrLines.length > 0 ? stderrLines[stderrLines.length - 1] : null;
}

function codexVersion() {
  const result = spawnSync(codexBin, ['--version'], { encoding: 'utf8' });
  if (result.status !== 0) {
    return 'unknown';
  }
  return result.stdout.trim() || result.stderr.trim() || 'unknown';
}

function codexArgs() {
  const args = ['--sandbox', 'read-only', '--ask-for-approval', 'never', '--cd', root];
  if (model) {
    args.push('--model', model);
  }
  args.push('exec', '--json', '--ephemeral', '--ignore-rules', '-');
  return args;
}

function uncachedTokens(usage) {
  if (!usage) {
    return null;
  }
  const input = usage.input_tokens;
  const output = usage.output_tokens;
  if (!Number.isFinite(input) || !Number.isFinite(output)) {
    return null;
  }
  const cached = Number.isFinite(usage.cached_tokens) ? usage.cached_tokens : 0;
  return Math.max(0, input - cached) + output;
}

function formatReduction(percent) {
  if (!Number.isFinite(percent)) {
    return 'n/a';
  }
  return percent >= 0 ? `${percent}% fewer` : `${Math.abs(percent)}% more`;
}

function runCodex(scenarioName, runIndex, prompt) {
  const scenarioOut = path.join(outDir, scenarioName, `run-${runIndex}`);
  mkdirSync(scenarioOut, { recursive: true });
  writeFileSync(path.join(scenarioOut, 'prompt.md'), prompt);

  const started = process.hrtime.bigint();
  const result = spawnSync(codexBin, codexArgs(), {
    cwd: root,
    input: prompt,
    encoding: 'utf8',
    maxBuffer: 50 * 1024 * 1024,
    timeout: timeoutMs,
  });
  const durationMs = Number(process.hrtime.bigint() - started) / 1_000_000;

  writeFileSync(path.join(scenarioOut, 'events.jsonl'), result.stdout || '');
  writeFileSync(path.join(scenarioOut, 'stderr.log'), result.stderr || '');

  const usageCandidate = parseUsage(result.stdout || '');
  const errorMessage = result.status === 0 ? null : parseErrorMessage(result.stdout || '', result.stderr || '');
  const run = {
    scenario: scenarioName,
    run: runIndex,
    status: result.status,
    signal: result.signal,
    duration_ms: Number(durationMs.toFixed(1)),
    usage: usageCandidate?.usage || null,
    usage_source: usageCandidate
      ? `events.jsonl line ${usageCandidate.line} path ${usageCandidate.path}`
      : null,
    error_message: errorMessage,
    output_dir: scenarioOut,
  };

  writeFileSync(path.join(scenarioOut, 'result.json'), `${JSON.stringify(run, null, 2)}\n`);
  return run;
}

function summarize(results, meta) {
  const grouped = new Map();
  for (const result of results) {
    if (!grouped.has(result.scenario)) {
      grouped.set(result.scenario, []);
    }
    grouped.get(result.scenario).push(result);
  }

  const scenarioRows = [...grouped.entries()].map(([name, runsForScenario]) => {
    const successful = runsForScenario.filter((run) => run.status === 0 && run.usage);
    const totals = successful.map((run) => run.usage.total_tokens).filter(Number.isFinite);
    const uncachedTotals = successful
      .map((run) => uncachedTokens(run.usage))
      .filter(Number.isFinite);
    const meanTotal =
      totals.length > 0 ? totals.reduce((sum, value) => sum + value, 0) / totals.length : null;
    const meanUncached =
      uncachedTotals.length > 0
        ? uncachedTotals.reduce((sum, value) => sum + value, 0) / uncachedTotals.length
        : null;
    return {
      scenario: name,
      label: scenarioLabels[name] || name,
      runs: runsForScenario.length,
      successful_usage_runs: successful.length,
      failures: runsForScenario
        .filter((run) => run.status !== 0 || !run.usage)
        .map((run) => ({
          run: run.run,
          status: run.status,
          signal: run.signal,
          error_message: run.error_message,
        })),
      mean_total_tokens: meanTotal == null ? null : Number(meanTotal.toFixed(1)),
      mean_uncached_tokens: meanUncached == null ? null : Number(meanUncached.toFixed(1)),
      totals,
      uncached_totals: uncachedTotals,
    };
  });

  const byName = Object.fromEntries(scenarioRows.map((row) => [row.scenario, row]));
  let comparison = null;
  if (byName['source-only']?.mean_total_tokens && byName.reqbook?.mean_total_tokens) {
    const sourceTotal = byName['source-only'].mean_total_tokens;
    const reqbookTotal = byName.reqbook.mean_total_tokens;
    const sourceUncached = byName['source-only'].mean_uncached_tokens;
    const reqbookUncached = byName.reqbook.mean_uncached_tokens;
    comparison = {
      source_only_mean_total_tokens: sourceTotal,
      reqbook_mean_total_tokens: reqbookTotal,
      total_token_delta: Number((sourceTotal - reqbookTotal).toFixed(1)),
      total_reduction_percent: Number((((sourceTotal - reqbookTotal) / sourceTotal) * 100).toFixed(1)),
      total_ratio: Number((sourceTotal / reqbookTotal).toFixed(2)),
      source_only_mean_uncached_tokens: sourceUncached,
      reqbook_mean_uncached_tokens: reqbookUncached,
      uncached_token_delta:
        sourceUncached && reqbookUncached ? Number((sourceUncached - reqbookUncached).toFixed(1)) : null,
      uncached_reduction_percent:
        sourceUncached && reqbookUncached
          ? Number((((sourceUncached - reqbookUncached) / sourceUncached) * 100).toFixed(1))
          : null,
      uncached_ratio:
        sourceUncached && reqbookUncached ? Number((sourceUncached / reqbookUncached).toFixed(2)) : null,
    };
  }

  return { meta, scenarios: scenarioRows, comparison, runs: results };
}

function printMarkdown(summary) {
  console.log('# Agent Token Benchmark (Codex)\n');
  console.log(`Captured: ${summary.meta.captured}`);
  console.log(`Machine: ${summary.meta.machine}`);
  console.log(`Codex: ${summary.meta.codex_version}`);
  console.log(`Model: ${summary.meta.model}`);
  console.log(`Runs per scenario: ${summary.meta.runs_per_scenario}`);
  console.log(`Fixture: ${summary.meta.fixture}`);
  console.log(`Output: ${summary.meta.output_dir}\n`);
  console.log('| Scenario | Successful usage runs | Mean total tokens | Mean uncached tokens | Run totals |');
  console.log('|---|---:|---:|---:|---|');
  for (const row of summary.scenarios) {
    const total = row.mean_total_tokens == null ? 'n/a' : String(row.mean_total_tokens);
    const uncached = row.mean_uncached_tokens == null ? 'n/a' : String(row.mean_uncached_tokens);
    const totals = row.totals.length ? row.totals.join(', ') : 'n/a';
    console.log(
      `| ${row.label} | ${row.successful_usage_runs}/${row.runs} | ${total} | ${uncached} | ${totals} |`,
    );
  }
  if (summary.comparison) {
    if (summary.comparison.uncached_reduction_percent != null) {
      console.log(
        `\nUncached-token comparison: Reqbook used ` +
          `${formatReduction(summary.comparison.uncached_reduction_percent)} tokens ` +
          `(${summary.comparison.uncached_ratio}x without / with Reqbook).`,
      );
    }
    console.log(
      `Total-token comparison: Reqbook used ` +
        `${formatReduction(summary.comparison.total_reduction_percent)} total tokens ` +
        `(${summary.comparison.total_ratio}x without / with Reqbook).`,
    );
  } else {
    console.log('\nMeasured reduction: n/a; Codex JSON output did not expose token usage for every scenario.');
  }
  const failures = summary.scenarios.flatMap((row) =>
    row.failures.map((failure) => ({ scenario: row.label, ...failure })),
  );
  if (failures.length > 0) {
    console.log('\nFailures:');
    for (const failure of failures) {
      const message = failure.error_message || 'usage not reported';
      console.log(
        `- ${failure.scenario} run ${failure.run}: status ${failure.status ?? 'n/a'}, ${message}`,
      );
    }
  }
}

if (!existsSync(scenariosDir)) {
  fail(`missing scenarios directory: ${scenariosDir}`);
}

const scenarioNames = readdirSync(scenariosDir)
  .filter((file) => file.endsWith('.md'))
  .map((file) => path.basename(file, '.md'))
  .sort((a, b) => (a === 'source-only' ? -1 : b === 'source-only' ? 1 : a.localeCompare(b)));

if (scenarioNames.length === 0) {
  fail(`no scenarios found in ${scenariosDir}`);
}

if (dryRun) {
  console.log(`Codex command: ${codexBin} ${codexArgs().join(' ')}`);
  console.log(`Scenarios: ${scenarioNames.join(', ')}`);
  console.log(`Runs per scenario: ${runs}`);
  process.exit(0);
}

mkdirSync(outDir, { recursive: true });

const meta = {
  captured: new Date().toISOString(),
  machine: `${os.type()} ${os.release()} ${os.arch()}`,
  codex_version: codexVersion(),
  model: model || 'default Codex config',
  runs_per_scenario: runs,
  fixture: 'examples/agent-token-api',
  output_dir: outDir,
  parser_note:
    'Token usage is parsed from the largest cumulative usage object in Codex JSONL output.',
};

const results = [];
for (const scenarioName of scenarioNames) {
  const prompt = readScenario(scenarioName);
  for (let runIndex = 1; runIndex <= runs; runIndex += 1) {
    results.push(runCodex(scenarioName, runIndex, prompt));
  }
}

const summary = summarize(results, meta);
writeFileSync(path.join(outDir, 'summary.json'), `${JSON.stringify(summary, null, 2)}\n`);

const markdown = [];
const originalLog = console.log;
console.log = (line = '') => {
  markdown.push(line);
  originalLog(line);
};
printMarkdown(summary);
console.log = originalLog;
writeFileSync(path.join(outDir, 'summary.md'), `${markdown.join('\n')}\n`);
