import type { AdHocRequest, AdHocResponse, ExecResult, FlowData, FlowEntry, FlowRunResult, GitBranchesData, ImportResult, IndexData, RuntimeExecOptions, ScanProjectResult, SpecData, ValidateResult, VarsData, WorkspaceEntry } from './types';

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

  getVariables: (env?: string) => {
    const query = env ? `?env=${encodeURIComponent(env)}` : '';
    return fetch(`${BASE}/variables${query}`).then(r => json<VarsData>(r));
  },

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

  getWorkspaceCurrent: () =>
    fetch(`${BASE}/workspace/current`).then(r => json<WorkspaceEntry>(r)),

  getWorkspaceRecent: () =>
    fetch(`${BASE}/workspace/recent`).then(r => json<WorkspaceEntry[]>(r)),

  getWorkspaceAll: () =>
    fetch(`${BASE}/workspace/all`).then(r => json<WorkspaceEntry[]>(r)),

  openWorkspace: (path: string) =>
    fetch(`${BASE}/workspace/open`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ path }),
    }).then(r => json<{ status: string; name: string }>(r)),

  createWorkspace: (path: string, name?: string) =>
    fetch(`${BASE}/workspace/create`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ path, name }),
    }).then(r => json<{ status: string; name: string }>(r)),

  pickFolder: async (): Promise<string | null> => {
    const res = await fetch(`${BASE}/pick-folder`);
    if (res.status === 404) return null;
    const data = await json<{ path: string | null }>(res);
    return data.path ?? null;
  },

  getGitBranches: () =>
    fetch(`${BASE}/git/branches`).then(r => json<GitBranchesData>(r)),

  checkoutGitBranch: (branch: string) =>
    fetch(`${BASE}/git/checkout`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ branch }),
    }).then(r => json<GitBranchesData>(r)),
};
