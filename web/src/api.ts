import type { AdHocRequest, AdHocResponse, ExecResult, FlowData, FlowEntry, FlowRunResult, ImportResult, IndexData, RuntimeExecOptions, ScanProjectResult, SpecData, ValidateResult, VarsData } from './types';

const BASE = '/api';

async function json<T>(res: Response): Promise<T> {
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error((err as { error?: string }).error ?? res.statusText);
  }
  return res.json();
}

export const api = {
  getIndex: () => fetch(`${BASE}/index`).then(r => json<IndexData>(r)),

  getSpec: (relPath: string) =>
    fetch(`${BASE}/spec/${relPath}`).then(r => json<SpecData>(r)),

  getFlows: () => fetch(`${BASE}/flows`).then(r => json<{ flows: FlowEntry[] }>(r)),

  getFlow: (relPath: string) =>
    fetch(`${BASE}/flow/${relPath}`).then(r => json<FlowData>(r)),

  saveFlow: (relPath: string, content: string) =>
    fetch(`${BASE}/flow/${relPath}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'text/plain; charset=utf-8' },
      body: content,
    }).then(r => json<{ status: string }>(r)),

  runFlow: (relPath: string) =>
    fetch(`${BASE}/flow/${relPath}`, { method: 'POST' }).then(r => json<FlowRunResult>(r)),

  validate: (relPath: string) =>
    fetch(`${BASE}/validate/${relPath}`).then(r => json<ValidateResult>(r)),

  execSpec: (relPath: string, options: RuntimeExecOptions) =>
    fetch(`${BASE}/exec/${relPath}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(options),
    }).then(r => json<ExecResult>(r)),

  getVariables: () => fetch(`${BASE}/variables`).then(r => json<VarsData>(r)),

  saveVariables: (env: string, vars: Record<string, string>) =>
    fetch(`${BASE}/variables`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ env, vars }),
    }).then(r => json<{ status: string }>(r)),

  saveSpec: (relPath: string, content: string) =>
    fetch(`${BASE}/spec/${relPath}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'text/plain; charset=utf-8' },
      body: content,
    }).then(r => json<{ status: string }>(r)),

  parseCurl: (curlText: string) =>
    fetch(`${BASE}/parse-curl`, {
      method: 'POST',
      headers: { 'Content-Type': 'text/plain' },
      body: curlText,
    }).then(r => json<{ method: string; url: string; headers: Record<string, string>; body?: string }>(r)),

  importCurl: (curlText: string) =>
    fetch(`${BASE}/import/curl`, {
      method: 'POST',
      headers: { 'Content-Type': 'text/plain' },
      body: curlText,
    }).then(r => json<ImportResult>(r)),

  scanProject: () => fetch(`${BASE}/scan/project`).then(r => json<ScanProjectResult>(r)),

  importProjectRoutes: () =>
    fetch(`${BASE}/scan/project`, { method: 'POST' }).then(r => json<ScanProjectResult>(r)),

  sendRequest: (req: AdHocRequest) =>
    fetch(`${BASE}/request`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(req),
    }).then(r => json<AdHocResponse>(r)),
};
