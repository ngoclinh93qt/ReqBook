import type { ExecResult, ImportResult, IndexData, SpecData, VarsData } from './types';

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

  execSpec: (relPath: string, vars: Record<string, string>) =>
    fetch(`${BASE}/exec/${relPath}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ vars }),
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

  importCurl: (curlText: string) =>
    fetch(`${BASE}/import/curl`, {
      method: 'POST',
      headers: { 'Content-Type': 'text/plain' },
      body: curlText,
    }).then(r => json<ImportResult>(r)),
};
