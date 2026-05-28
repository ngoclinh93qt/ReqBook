import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { api } from '../api';
import { saveRun } from '../hooks/useRunResults';
import type { ExecResult, RuntimeExecOptions, SpecData, VarsData } from '../types';
import { Icon, JsonEditor, MethodBadge, parseRequest, PathStr, highlight, uniqueMatches } from '../ui';

type VarRow = { id: string; name: string; override: string; locked: boolean };
type ParamRow = { id: string; name: string; value: string; locked: boolean };
type HeaderRow = { id: string; name: string; value: string; enabled: boolean; locked: boolean };

const rid = () => Math.random().toString(36).slice(2, 9);

export function SpecPage({ env, varsData, browserVars }: {
  env: string;
  varsData: VarsData | null;
  browserVars: Record<string, string>;
}) {
  const { '*': relPath = '' } = useParams();
  const navigate = useNavigate();
  const [spec, setSpec] = useState<SpecData | null>(null);
  const [error, setError] = useState('');
  const [tab, setTab] = useState<'request' | 'expected' | 'tests' | 'source'>('request');
  const [resultTab, setResultTab] = useState<'diff' | 'response' | 'headers' | 'request'>('diff');
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<ExecResult | null>(null);
  const [showEdit, setShowEdit] = useState(false);
  const [sourceText, setSourceText] = useState('');
  const [saveMsg, setSaveMsg] = useState('');

  const loadSpec = useCallback(async () => {
    try {
      const next = await api.getSpec(relPath);
      setSpec(next);
      setSourceText(next.raw_source);
      setError('');
    } catch (e) {
      setError(String(e));
    }
  }, [relPath]);

  useEffect(() => { loadSpec(); }, [loadSpec]);

  if (error) return <div className="spec-wrap"><button className="btn" onClick={() => navigate('/')}>Back</button><div className="empty fail-text">{error}</div></div>;
  if (!spec) return <div className="spec-wrap"><div className="empty">Loading spec…</div></div>;

  async function saveSource() {
    if (!spec) return;
    setSaveMsg('');
    try {
      await api.saveSpec(relPath, sourceText);
      setSaveMsg('Saved');
      setShowEdit(false);
      await loadSpec();
    } catch (e) {
      setSaveMsg(String(e));
    }
  }

  const envVars = varsData?.vars ?? {};
  const sourceTab = sourceText || spec.raw_source;
  const tabContent = {
    request: spec.request,
    expected: spec.expected_response,
    tests: spec.tests ?? '# No tests yet\n',
    source: sourceTab,
  };

  return (
    <div className="spec-wrap">
      <header className="spec-head">
        <div className="top-row">
          <button className="btn ghost sm back-btn" onClick={() => navigate('/')}>← Back</button>
          <MethodBadge method={spec.method} />
          <span className="file-chip">{spec.rel_path}</span>
          <div className="top-actions">
            <button className="btn ghost sm" onClick={() => setShowEdit(open => !open)}><Icon.edit /> Edit source</button>
          </div>
        </div>
        <h1>{spec.title}</h1>
        <p className="desc">{spec.description}</p>
        <div className="url-row">
          <span className="url"><span className="base">{envVars.baseUrl ?? envVars.base_url ?? '{{baseUrl}}'}</span><PathStr path={spec.path} /></span>
          <button className="copy-url" onClick={() => navigator.clipboard?.writeText(spec.path)}><Icon.copy /> Copy</button>
        </div>
      </header>

      <div className="split">
        <div>
          <div className="card">
            <div className="tabs">
              {(['request', 'expected', 'tests', 'source'] as const).map(item => (
                <button key={item} className={`tab ${tab === item ? 'is-on' : ''}`} onClick={() => setTab(item)}>{label(item)}</button>
              ))}
              <div className="tab-actions"><button className="btn ghost sm icon" onClick={() => navigator.clipboard?.writeText(tabContent[tab])}><Icon.copy /></button></div>
            </div>
            <pre className="code" dangerouslySetInnerHTML={{ __html: highlight(tabContent[tab]) }} />
          </div>

          {showEdit && (
            <div className="card edit-card">
              <div className="card-h"><Icon.edit /><span>Edit markdown source</span><span className="tag">{spec.rel_path}</span></div>
              <div className="edit-body">
                <textarea className="input mono source-editor" value={sourceText} onChange={e => setSourceText(e.target.value)} spellCheck={false} />
                <div className="edit-actions">
                  {saveMsg && <span className={saveMsg === 'Saved' ? 'ok-text' : 'fail-text'}>{saveMsg}</span>}
                  <button className="btn sm" onClick={() => { setSourceText(spec.raw_source); setShowEdit(false); }}>Cancel</button>
                  <button className="btn-primary btn-sm-primary" onClick={saveSource}>Save</button>
                </div>
              </div>
            </div>
          )}
        </div>

        <Runner
          spec={spec}
          env={env}
          envVars={envVars}
          browserVars={browserVars}
          running={running}
          setRunning={setRunning}
          result={result}
          setResult={setResult}
          resultTab={resultTab}
          setResultTab={setResultTab}
          relPath={relPath}
        />
      </div>
    </div>
  );
}

function Runner({ spec, env, envVars, browserVars, running, setRunning, result, setResult, resultTab, setResultTab, relPath }: {
  spec: SpecData;
  env: string;
  envVars: Record<string, string>;
  browserVars: Record<string, string>;
  running: boolean;
  setRunning: (running: boolean) => void;
  result: ExecResult | null;
  setResult: (result: ExecResult | null) => void;
  resultTab: 'diff' | 'response' | 'headers' | 'request';
  setResultTab: (tab: 'diff' | 'response' | 'headers' | 'request') => void;
  relPath: string;
}) {
  const detectedVars = useMemo(() => uniqueMatches(spec.request, /\{\{([a-zA-Z0-9_]+)\}\}/g), [spec.request]);
  const detectedParams = useMemo(() => uniqueMatches(spec.path, /:([a-zA-Z0-9_*]+)/g), [spec.path]);
  const parsed = useMemo(() => parseRequest(spec.request), [spec.request]);
  const [varRows, setVarRows] = useState<VarRow[]>([]);
  const [paramRows, setParamRows] = useState<ParamRow[]>([]);
  const [headerRows, setHeaderRows] = useState<HeaderRow[]>([]);
  const [bodyText, setBodyText] = useState('');
  const [bodyDirty, setBodyDirty] = useState(false);
  const [curlOpen, setCurlOpen] = useState(false);

  useEffect(() => {
    setVarRows(detectedVars.map(name => ({ id: rid(), name, override: '', locked: true })));
    setParamRows(detectedParams.map(name => ({ id: rid(), name, value: '', locked: true })));
    setHeaderRows(parsed.headers.map(header => ({ ...header, locked: true })));
    setBodyText(parsed.body);
    setBodyDirty(false);
    setResult(null);
  }, [detectedParams, detectedVars, parsed.body, parsed.headers, setResult, spec.rel_path]);

  // Auto-detect vars typed into body / headers / params and add them to varRows
  useEffect(() => {
    const allText = [
      bodyDirty ? bodyText : '',
      ...headerRows.map(row => row.value),
      ...paramRows.map(row => row.value),
    ].join('\n');
    const found = uniqueMatches(allText, /\{\{([a-zA-Z0-9_]+)\}\}/g);
    if (found.length === 0) return;
    setVarRows(rows => {
      const existing = new Set(rows.map(r => r.name));
      const missing = found.filter(name => !existing.has(name));
      if (missing.length === 0) return rows;
      return [...rows, ...missing.map(name => ({ id: rid(), name, override: '', locked: false }))];
    });
  }, [bodyDirty, bodyText, headerRows, paramRows]);

  function valueOf(row: VarRow) {
    if (row.override) return { value: row.override, src: 'runtime' };
    if (browserVars[row.name] != null) return { value: browserVars[row.name], src: 'browser' };
    if (envVars[row.name] != null) return { value: envVars[row.name], src: 'env' };
    return { value: '', src: 'missing' };
  }

  function buildOptions(): RuntimeExecOptions {
    const vars = Object.fromEntries(varRows.filter(row => row.name).map(row => [row.name, valueOf(row).value]));
    const path_params = Object.fromEntries(paramRows.filter(row => row.name && row.value).map(row => [row.name, resolve(row.value)]));
    const headers = Object.fromEntries(headerRows.filter(row => row.enabled && row.name).map(row => [row.name, resolve(row.value)]));
    return { vars, path_params, headers, body: bodyDirty ? bodyText : undefined };
  }

  function resolve(text: string) {
    return text.replace(/\{\{([a-zA-Z0-9_]+)\}\}/g, (_, name) => {
      const row = varRows.find(item => item.name === name);
      if (row) return valueOf(row).value || `{{${name}}}`;
      return browserVars[name] ?? envVars[name] ?? `{{${name}}}`;
    });
  }

  async function run() {
    setRunning(true);
    setResult(null);
    try {
      const execution = await api.execSpec(relPath, buildOptions());
      saveRun(relPath, {
        status: execution.response?.status ?? null,
        passed: execution.diff.passed,
        duration_ms: execution.duration_ms,
      });
      setResult(execution);
      setResultTab('diff');
    } catch (e) {
      saveRun(relPath, { status: null, passed: false, duration_ms: 0 });
      setResult({ duration_ms: 0, diff: { passed: false }, error: String(e) });
      setResultTab('diff');
    } finally {
      setRunning(false);
    }
  }

  const resultTabs = [
    { id: 'diff', label: 'Diff' },
    { id: 'response', label: 'Body' },
    { id: 'headers', label: 'Headers', badge: Object.keys(result?.response?.headers ?? {}).length },
    { id: 'request', label: 'Request' },
  ] as const;

  return (
    <div>
      <div className="card">
        <div className="card-h"><span className="run-dot" /><span>Run this endpoint</span><span className="tag">env <b>{env}</b></span></div>

        <div className="run-section">
          <div className="sec-h"><span>Variables</span><span className="tag">{varRows.length} resolved live</span></div>
          {varRows.map(row => {
            const { value, src } = valueOf(row);
            return (
              <div className="kv" key={row.id}>
                {row.locked ? <div className="k"><span className="br">{'{{'}</span>{row.name}<span className="br">{'}}'}</span></div> : <input className="input flush mono" value={row.name} placeholder="name" onChange={e => setVarRows(rows => rows.map(item => item.id === row.id ? { ...item, name: e.target.value } : item))} />}
                <input className="input flush mono" value={row.override || value} placeholder={src === 'missing' ? '— required —' : ''} onChange={e => setVarRows(rows => rows.map(item => item.id === row.id ? { ...item, override: e.target.value } : item))} />
                <div className={`src ${src}`}><span className="dot" />{src}</div>
                <button className="row-del" onClick={() => setVarRows(rows => rows.filter(item => item.id !== row.id))}><Icon.x /></button>
              </div>
            );
          })}
          <button className="add-row" onClick={() => setVarRows(rows => [...rows, { id: rid(), name: '', override: '', locked: false }])}><Icon.plus /> Add variable</button>
        </div>

        <div className="run-section">
          <div className="sec-h"><span>Path params</span><span className="tag">{paramRows.length}</span></div>
          {paramRows.length === 0 && <div className="hint-line">No path params.</div>}
          {paramRows.map(row => (
            <div className="kv" key={row.id}>
              {row.locked ? <div className="k"><span className="br">:</span>{row.name}</div> : <input className="input flush mono" value={row.name} placeholder="name" onChange={e => setParamRows(rows => rows.map(item => item.id === row.id ? { ...item, name: e.target.value } : item))} />}
              <input className="input flush mono" value={row.value} placeholder="— required —" onChange={e => setParamRows(rows => rows.map(item => item.id === row.id ? { ...item, value: e.target.value } : item))} />
              <div className="src runtime"><span className="dot" />runtime</div>
              <button className="row-del" onClick={() => setParamRows(rows => rows.filter(item => item.id !== row.id))}><Icon.x /></button>
            </div>
          ))}
          <button className="add-row" onClick={() => setParamRows(rows => [...rows, { id: rid(), name: '', value: '', locked: false }])}><Icon.plus /> Add param</button>
        </div>

        <div className="run-section">
          <div className="sec-h"><span>Headers</span><span className="tag">{headerRows.length}</span></div>
          {headerRows.map(row => (
            <div className="kv hdr" key={row.id}>
              <input className="input flush mono info-input" value={row.name} placeholder="Header-Name" onChange={e => setHeaderRows(rows => rows.map(item => item.id === row.id ? { ...item, name: e.target.value } : item))} />
              <input className="input flush mono" value={row.value} placeholder="value" onChange={e => setHeaderRows(rows => rows.map(item => item.id === row.id ? { ...item, value: e.target.value } : item))} />
              <button className="row-del always" onClick={() => setHeaderRows(rows => rows.filter(item => item.id !== row.id))}><Icon.x /></button>
            </div>
          ))}
          <div className="inline-actions">
            <button className="add-row" onClick={() => setHeaderRows(rows => [...rows, { id: rid(), name: '', value: '', enabled: true, locked: false }])}><Icon.plus /> Add header</button>
            <HeaderSuggestions
              existing={headerRows}
              onPick={(name, value) => setHeaderRows(rows => [...rows, { id: rid(), name, value, enabled: true, locked: false }])}
            />
          </div>
        </div>

        {(parsed.body || !['GET', 'DELETE', 'HEAD'].includes(spec.method)) && (
          <div className="run-section">
            <div className="sec-h"><span>Body</span><span className="tag">{bodyDirty ? 'edited' : 'from spec'}</span></div>
            <JsonEditor value={bodyText} onChange={value => { setBodyText(value); setBodyDirty(value !== parsed.body); }} placeholder="Empty body" minHeight={140} />
            {bodyDirty && <button className="btn ghost sm reset-body" onClick={() => { setBodyText(parsed.body); setBodyDirty(false); }}>Reset to spec body</button>}
          </div>
        )}

        <div className="run-actions">
          <button className="btn-primary run-button" onClick={run} disabled={running}>{running ? <><span className="pulse-dot" />Sending request…</> : <><Icon.play />Send {spec.method} {spec.path}</>}</button>
          <button className="btn" onClick={() => setCurlOpen(true)} title="Copy as curl"><Icon.copy /> Copy as curl</button>
        </div>
      </div>

      {running && !result && <div className="card result-card"><div className="empty compact"><span className="pulse-dot" />Sending {spec.method}…</div></div>}
      {result && <ResultCard result={result} resultTab={resultTab} setResultTab={setResultTab} tabs={resultTabs} spec={spec} />}
      {curlOpen && <CurlPreview curl={buildCurl(spec, envVars, buildOptions())} onClose={() => setCurlOpen(false)} />}
    </div>
  );
}

function HeaderSuggestions({ existing, onPick }: {
  existing: HeaderRow[];
  onPick: (name: string, value: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const common = [
    { name: 'Accept', value: 'application/json' },
    { name: 'Content-Type', value: 'application/json' },
    { name: 'Authorization', value: 'Bearer {{token}}' },
    { name: 'X-Request-Id', value: '{{requestId}}' },
    { name: 'Idempotency-Key', value: '{{requestId}}' },
    { name: 'X-Workspace-Id', value: '{{workspaceId}}' },
    { name: 'If-Match', value: '{{etag}}' },
    { name: 'Prefer', value: 'return=minimal' },
  ];
  const taken = new Set(existing.map(header => header.name.toLowerCase()).filter(Boolean));
  const available = common.filter(header => !taken.has(header.name.toLowerCase()));
  return (
    <div className="common-headers">
      <button className="add-row solid" onClick={() => setOpen(value => !value)}>
        Common headers
      </button>
      {open && (
        <div className="menu header-menu">
          {available.length === 0 && <div className="menu-empty">All common headers added.</div>}
          {available.map(header => (
            <button
              key={header.name}
              className="menu-item header-option"
              onClick={() => {
                onPick(header.name, header.value);
                setOpen(false);
              }}
            >
              <span className="header-name">{header.name}</span>
              <span className="end">{header.value}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function ResultCard({ result, resultTab, setResultTab, tabs, spec }: {
  result: ExecResult;
  resultTab: 'diff' | 'response' | 'headers' | 'request';
  setResultTab: (tab: 'diff' | 'response' | 'headers' | 'request') => void;
  tabs: readonly { id: 'diff' | 'response' | 'headers' | 'request'; label: string; badge?: number }[];
  spec: SpecData;
}) {
  const passed = result.diff?.passed ?? false;
  return (
    <div className="card result-card">
      <div className={`result-status ${passed ? 'ok' : 'fail'}`}>
        <span className="verdict">{passed ? <Icon.check /> : <Icon.cross />}{passed ? 'Passed' : 'Failed'}</span>
        <span className="sep">·</span><span>{result.response?.status ?? 'No response'}</span>
        <span className="sep">·</span><span>{result.duration_ms}ms</span>
      </div>
      <div className="tabs">
        {tabs.map(tab => <button key={tab.id} className={`tab ${resultTab === tab.id ? 'is-on' : ''}`} onClick={() => setResultTab(tab.id)}>{tab.label}{tab.badge != null && <span className="badge">{tab.badge}</span>}</button>)}
      </div>
      {resultTab === 'diff' && <pre className="code">{formatDiff(result, spec)}</pre>}
      {resultTab === 'response' && <pre className="code" dangerouslySetInnerHTML={{ __html: highlight(formatBody(result.response?.body ?? result.error ?? '')) }} />}
      {resultTab === 'headers' && <div className="headers-list">{Object.entries(result.response?.headers ?? {}).map(([key, value]) => <div className="kv no-del" key={key}><div className="k info-text">{key}</div><div className="header-value">{value}</div></div>)}</div>}
      {resultTab === 'request' && <pre className="code" dangerouslySetInnerHTML={{ __html: highlight(formatRequest(result)) }} />}
    </div>
  );
}

function CurlPreview({ curl, onClose }: { curl: string; onClose: () => void }) {
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={e => e.stopPropagation()}>
        <div className="modal-h"><Icon.copy /><div><h2>Copy as curl</h2><div className="sub">Generated with current runtime overrides.</div></div><button className="btn icon" onClick={onClose}><Icon.x /></button></div>
        <div className="modal-b"><pre className="code">{curl}</pre></div>
        <div className="modal-f"><div className="note">Unresolved variables remain as placeholders.</div><button className="btn-primary btn-sm-primary" onClick={() => navigator.clipboard?.writeText(curl)}>Copy</button></div>
      </div>
    </div>
  );
}

function label(tab: string) {
  return tab === 'expected' ? 'Expected' : tab[0].toUpperCase() + tab.slice(1);
}

function formatBody(body: string) {
  try { return JSON.stringify(JSON.parse(body), null, 2); } catch { return body || 'No response body.'; }
}

function formatDiff(result: ExecResult, _spec: SpecData) {
  const lines = [`passed: ${result.diff.passed ? 'true' : 'false'}`];
  if (result.diff.status) lines.push(`status: ${result.diff.status}`);
  if (result.diff.headers?.length) lines.push(`headers:\n${result.diff.headers.map(item => `  - ${item}`).join('\n')}`);
  if (result.diff.body) lines.push(`body: ${result.diff.body}`);
  if (result.error) lines.push(`error: ${result.error}`);
  return lines.join('\n');
}

function formatRequest(result: ExecResult) {
  if (!result.request) return 'No request captured.';
  const headers = Object.entries(result.request.headers).map(([key, value]) => `${key}: ${value}`).join('\n');
  return `${result.request.method} ${result.request.url}\n${headers}\n\n${result.request.body}`;
}

function buildCurl(spec: SpecData, envVars: Record<string, string>, options: RuntimeExecOptions) {
  let url = `${envVars.baseUrl ?? envVars.base_url ?? '{{baseUrl}}'}${spec.path}`;
  for (const [key, value] of Object.entries(options.path_params)) url = url.replace(`:${key}`, value);
  const parts = [`curl -X ${spec.method} '${url}'`];
  for (const [key, value] of Object.entries(options.headers)) parts.push(`  -H '${key}: ${value}'`);
  if (options.body) parts.push(`  -d '${options.body.replace(/'/g, "'\\''")}'`);
  return parts.join(' \\\n');
}
