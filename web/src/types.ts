export interface SpecEntry {
  method: string;
  path: string;
  title: string;
  rel_path: string;
}

export interface ResourceGroup {
  resource: string;
  specs: SpecEntry[];
}

export interface IndexData {
  project_name: string;
  groups: ResourceGroup[];
  spec_count: number;
  version: string;
  mock_mode?: boolean;
}

export interface SpecData {
  title: string;
  method: string;
  path: string;
  description: string;
  request: string;
  expected_response: string;
  tests: string | null;
  rel_path: string;
  env: string;
  raw_source: string;
  version: string;
}

export interface VarsData {
  env: string;
  vars: Record<string, string>;
  envs: string[];
}

export interface ExecResult {
  request?: {
    method: string;
    url: string;
    headers: Record<string, string>;
    body: string;
  };
  duration_ms: number;
  diff: {
    passed: boolean;
    status?: string | null;
    headers?: string[];
    body?: string | null;
  };
  response?: {
    status: number;
    body: string;
    headers: Record<string, string>;
  };
  error?: string;
  mock?: boolean;
}

export interface RuntimeExecOptions {
  vars: Record<string, string>;
  path_params: Record<string, string>;
  headers: Record<string, string>;
  body?: string;
}

export interface ImportResult {
  status: 'created' | 'exists';
  rel_path?: string;
  path?: string;
  spec?: string;
  message?: string;
  error?: string;
}

export interface ScanRoute {
  method: string;
  path: string;
  title: string;
  resource: string;
  exists: boolean;
}

export interface ScanProjectResult {
  project_name: string;
  routes_found: number;
  missing_count: number;
  existing_count: number;
  duration_ms: number;
  routes: ScanRoute[];
  written: string[];
}

export interface FlowEntry {
  name: string;
  title: string;
  rel_path: string;
  steps: number;
}

export interface FlowCapture {
  source: string;
  name: string;
}

export interface FlowStep {
  name: string;
  endpoint: string;
  inject: string[];
  capture: FlowCapture[];
  assert: string[];
}

export interface FlowData {
  name: string;
  title: string;
  description?: string | null;
  rel_path: string;
  raw_source: string;
  steps: FlowStep[];
}

export interface FlowRunResult {
  steps: Array<{
    name: string;
    endpoint: string;
    execution?: ExecResult | null;
    error?: string | null;
  }>;
  captures: Record<string, string>;
  passed: boolean;
}

export interface ValidateResult {
  valid: boolean;
  kind: 'api' | 'flow' | 'unknown';
  path: string;
  error?: string | null;
}

export interface AdHocRequest {
  method: string;
  url: string;
  headers: Record<string, string>;
  body?: string;
  vars: Record<string, string>;
  env: string;
  save_as?: string;
}

export interface AdHocResponse extends ExecResult {
  saved_path?: string | null;
}
