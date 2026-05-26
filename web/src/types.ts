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
  duration_ms: number;
  diff: { passed: boolean };
  response?: {
    status: number;
    body: string;
    headers: Record<string, string>;
  };
  error?: string;
}

export interface ImportResult {
  status: 'created' | 'exists';
  rel_path?: string;
  path?: string;
  spec?: string;
  message?: string;
  error?: string;
}
