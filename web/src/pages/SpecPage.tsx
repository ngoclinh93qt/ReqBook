import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { api } from '../api';
import { saveRun } from '../hooks/useRunResults';
import type { ExecResult, RuntimeExecOptions, SpecData, VarsData } from '../types';
import { Icon, JsonEditor, parseRequest, highlight, uniqueMatches } from '../ui';

type VarRow = { id: string; name: string; override: string; locked: boolean };
type ParamRow = { id: string; name: string; value: string; locked: boolean };
type HeaderRow = { id: string; name: string; value: string; enabled: boolean; locked: boolean };
type RequestTab = 'params' | 'headers' | 'body' | 'expected' | 'tests' | 'source';

const rid = () => Math.random().toString(36).slice(2, 9);
const HTTP_METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'];
const hasBody = (method: string) => !['GET', 'DELETE', 'HEAD', 'OPTIONS'].includes(method);

export function SpecPage({ env, varsData, browserVars, mockMode }: {
  env: string;
  varsData: VarsData | null;
  browserVars: Record<string, string>;
  mockMode?: boolean;
}) {
  const { '*': relPath = '' } = useParams();
  const navigate = useNavigate();
  const [spec, setSpec] = useState<SpecData | null>(null);
  const [error, setError] = useState('');
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

  async function saveSource() {
    if (!spec) return;
    setSaveMsg('');
    try {
      await api.saveSpec(relPath, sourceText);
      setSaveMsg('Saved');
      await loadSpec();
    } catch (e) {
      setSaveMsg(String(e));
    }
  }

  if (error) {
    return (
      <div className="req-page">
        <button className="btn" onClick={() => navigate('/')}>Back</button>
        <div className="empty fail-text">{error}</div>
      </div>
    );
  }
  if (!spec) return <div className="req-page"><div className="empty">Loading spec...</div></div>;

  return (
    <SpecWorkspace
      spec={spec}
      relPath={relPath}
      env={env}
      envVars={varsData?.vars ?? {}}
      browserVars={browserVars}
      mockMode={mockMode}
      sourceText={sourceText}
      setSourceText={setSourceText}
      saveSource={saveSource}
      saveMsg={saveMsg}
      navigateHome={() => navigate('/')}
    />
  );
}

function SpecWorkspace({ spec, relPath, env, envVars, browserVars, mockMode, sourceText, setSourceText, saveSource, saveMsg, navigateHome }: {
  spec: SpecData;
  relPath: string;
  env: string;
  envVars: Record<string, string>;
  browserVars: Record<string, string>;
  mockMode?: boolean;
  sourceText: string;
  setSourceText: (value: string) => void;
  saveSource: () => Promise<void>;
  saveMsg: string;
  navigateHome: () => void;
}) {
  const isMock = !!mockMode;
  const [reqTab, setReqTab] = useState<RequestTab>('params');
  const [resultTab, setResultTab] = useState<'diff' | 'response' | 'headers' | 'request'>('diff');
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<ExecResult | null>(null);
  const [varsOpen, setVarsOpen] = useState(true);
  const [varRows, setVarRows] = useState<VarRow[]>([]);
  const [paramRows, setParamRows] = useState<ParamRow[]>([]);
  const [headerRows, setHeaderRows] = useState<HeaderRow[]>([]);
  const [bodyText, setBodyText] = useState('');
  const [bodyDirty, setBodyDirty] = useState(false);
  const [curlOpen, setCurlOpen] = useState(false);

  const detectedVars = useMemo(() => uniqueMatches(spec.request, /\{\{([a-zA-Z0-9_]+)\}\}/g), [spec.request]);
  const detectedParams = useMemo(() => uniqueMatches(spec.path, /:([a-zA-Z0-9_*]+)/g), [spec.path]);
  const parsed = useMemo(() => parseRequest(spec.request), [spec.request]);

  useEffect(() => {
    setVarRows(detectedVars.map(name => ({ id: rid(), name, override: '', locked: true })));
    setParamRows(detectedParams.map(name => ({ id: rid(), name, value: '', locked: true })));
    setHeaderRows(parsed.headers.map(header => ({ ...header, locked: true })));
    setBodyText(parsed.body);
    setBodyDirty(false);
    setResult(null);
    setReqTab('params');
    setResultTab('diff');
  }, [detectedParams, detectedVars, parsed.body, parsed.headers, spec.rel_path]);

  useEffect(() => {
    const allText = [
      bodyDirty ? bodyText : '',
      ...headerRows.map(row => row.value),
      ...paramRows.map(row => row.value),
    ].join('\n');
    const found = uniqueMatches(allText, /\{\{([a-zA-Z0-9_]+)\}\}/g);
    if (found.length === 0) return;
    setVarRows(rows => {
      const existing = new Set(rows.map(row => row.name));
      const missing = found.filter(name => !existing.has(name));
      if (missing.length === 0) return rows;
      return [...rows, ...missing.map(name => ({ id: rid(), name, override: '', locked: false }))];
    });
  }, [bodyDirty, bodyText, headerRows, paramRows]);

  const availableVariableNames = useMemo(() => {
    const names = new Set<string>();
    for (const row of varRows) {
      const name = row.name.trim();
      if (name) names.add(name);
    }
    for (const name of Object.keys(browserVars)) {
      if (name.trim()) names.add(name.trim());
    }
    for (const name of Object.keys(envVars)) {
      if (name.trim()) names.add(name.trim());
    }
    return Array.from(names).sort((a, b) => a.localeCompare(b));
  }, [browserVars, envVars, varRows]);

  function valueOf(row: VarRow) {
    if (row.override) return { value: row.override, src: 'runtime' };
    if (browserVars[row.name] != null) return { value: browserVars[row.name], src: 'browser' };
    if (envVars[row.name] != null) return { value: envVars[row.name], src: 'env' };
    return { value: '', src: 'missing' };
  }

  function resolve(text: string) {
    return text.replace(/\{\{([a-zA-Z0-9_]+)\}\}/g, (_, name) => {
      const row = varRows.find(item => item.name === name);
      if (row) return valueOf(row).value || `{{${name}}}`;
      return browserVars[name] ?? envVars[name] ?? `{{${name}}}`;
    });
  }

  function buildOptions(): RuntimeExecOptions {
    const referencedVars = new Set(detectedVars);
    const overrideText = [
      bodyDirty ? bodyText : '',
      ...headerRows.map(row => row.value),
      ...paramRows.map(row => row.value),
    ].join('\n');
    for (const name of uniqueMatches(overrideText, /\{\{([a-zA-Z0-9_]+)\}\}/g)) referencedVars.add(name);

    const vars: Record<string, string> = {};
    for (const name of referencedVars) {
      if (browserVars[name] != null) vars[name] = browserVars[name];
    }
    for (const row of varRows) {
      if (row.name) vars[row.name] = valueOf(row).value;
    }
    const path_params = Object.fromEntries(paramRows.filter(row => row.name && row.value).map(row => [row.name, resolve(row.value)]));
    const headers = Object.fromEntries(headerRows.filter(row => row.enabled && row.name).map(row => [row.name, resolve(row.value)]));
    return { vars, path_params, headers, body: bodyDirty ? bodyText : undefined };
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

  const baseUrl = envVars.baseUrl ?? envVars.base_url ?? '{{baseUrl}}';
  const resolvedPath = paramRows.reduce((path, row) => row.name && row.value ? path.replace(`:${row.name}`, resolve(row.value)) : path, spec.path);
  const displayUrl = `${baseUrl}${resolvedPath}`;
  const missingVars = varRows.filter(row => valueOf(row).src === 'missing').length;
  const canSend = !!spec.path;

  const reqTabs: { id: RequestTab; label: string; badge?: string | number | null }[] = [
    { id: 'params', label: 'Params', badge: paramRows.length || null },
    { id: 'headers', label: 'Headers', badge: headerRows.filter(row => row.enabled && row.name).length || null },
    ...(hasBody(spec.method) || parsed.body ? [{ id: 'body' as const, label: 'Body', badge: bodyText ? '1' : null }] : []),
    { id: 'expected', label: 'Expected' },
    { id: 'tests', label: 'Tests' },
    { id: 'source', label: 'Source' },
  ];

  return (
    <div className="req-page">
      <div className="rp-crumb">
        <button className="rp-back" onClick={navigateHome}>
          <svg width="11" height="11" viewBox="0 0 11 11" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><line x1="9" y1="5.5" x2="2" y2="5.5" /><polyline points="5,3 2,5.5 5,8" /></svg>
          Collections
        </button>
        <span className="sep">/</span>
        <span className="rp-rel">{spec.rel_path}</span>
        <div className="rp-crumb-r">
          <button className="btn icon" title="Copy as curl" onClick={() => setCurlOpen(true)}><Icon.copy /></button>
        </div>
      </div>

      <div className="rp-titles">
        <h1 className="rp-title">{spec.title}</h1>
        {spec.description && <p className="rp-desc">{spec.description}</p>}
      </div>

      <div className="rp-bar">
        <select className={`rp-method-select m-${spec.method.toLowerCase()}`} value={spec.method} disabled>
          {HTTP_METHODS.map(item => <option key={item} value={item}>{item}</option>)}
        </select>
        <div className="rp-url">
          <span className="base">{baseUrl}</span>
          <input className="rp-url-input" value={resolvedPath} readOnly spellCheck={false} />
        </div>
        <button className="btn-primary rp-send" onClick={run} disabled={running || !canSend}>
          {running ? <><span className="pulse-dot" />{isMock ? 'Loading mock...' : 'Sending...'}</> : <><Icon.play />{isMock ? 'Mock' : 'Send'}</>}
        </button>
      </div>

      <div className={`var-strip ${varsOpen ? 'open' : ''}`}>
        <button className="vs-head" onClick={() => setVarsOpen(open => !open)}>
          <span className="chev"><Icon.chev open={varsOpen} /></span>
          <Icon.vars />
          <span className="vs-title">Variables</span>
          <span className="vs-count">{varRows.length}</span>
          {missingVars > 0 && <span className="vs-missing">{missingVars} missing</span>}
          <span className="vs-hint">resolved against <b>{env}</b></span>
        </button>
        {varsOpen && (
          <div className="vs-body">
            {varRows.length === 0 && <div className="vs-empty">No variables detected. Use <code>{'{{name}}'}</code> in the URL, headers, or body.</div>}
            {varRows.map(row => {
              const { value, src } = valueOf(row);
              return (
                <div className="vrow" key={row.id}>
                  {row.locked ? (
                    <div className="vrow-name"><span className="br">{'{{'}</span>{row.name}<span className="br">{'}}'}</span></div>
                  ) : (
                    <input className="vrow-input name" value={row.name} placeholder="name" onChange={event => setVarRows(rows => rows.map(item => item.id === row.id ? { ...item, name: event.target.value } : item))} />
                  )}
                  <input className="vrow-input" value={row.override || value} placeholder={src === 'missing' ? '-- required --' : ''} onChange={event => setVarRows(rows => rows.map(item => item.id === row.id ? { ...item, override: event.target.value } : item))} />
                  <span className={`src ${src}`}><span className="dot" />{src}</span>
                  <button className="row-del always" onClick={() => setVarRows(rows => rows.filter(item => item.id !== row.id))}><Icon.x /></button>
                </div>
              );
            })}
            <button className="add-row" onClick={() => setVarRows(rows => [...rows, { id: rid(), name: '', override: '', locked: false }])}><Icon.plus />Add variable</button>
          </div>
        )}
      </div>

      <div className="rp-split">
        <section className="rp-pane">
          <div className="rp-pane-h">
            <div className="tabs flush">
              {reqTabs.map(tab => (
                <button key={tab.id} className={`tab ${reqTab === tab.id ? 'is-on' : ''}`} onClick={() => setReqTab(tab.id)}>
                  {tab.label}{tab.badge != null && <span className="badge">{tab.badge}</span>}
                </button>
              ))}
            </div>
          </div>
          <div className="rp-pane-b">
            {reqTab === 'params' && (
              <div className="kv-block">
                <div className="kv-sub">Path params</div>
                {paramRows.length === 0 && <div className="kv-none">No <code>:params</code> in this path.</div>}
                {paramRows.map(row => (
                  <div className="kvrow" key={row.id}>
                    <div className="kvrow-name">{row.locked ? <span className="pn"><span className="br">:</span>{row.name}</span> : <input className="kvi mono" value={row.name} placeholder="name" onChange={event => setParamRows(rows => rows.map(item => item.id === row.id ? { ...item, name: event.target.value } : item))} />}</div>
                    <div className="kvrow-val"><input className="kvi mono" value={row.value} placeholder="-- required --" onChange={event => setParamRows(rows => rows.map(item => item.id === row.id ? { ...item, value: event.target.value } : item))} /></div>
                    <span className="src runtime"><span className="dot" />runtime</span>
                    <button className="row-del always" onClick={() => setParamRows(rows => rows.filter(item => item.id !== row.id))}><Icon.x /></button>
                  </div>
                ))}
                <button className="add-row" onClick={() => setParamRows(rows => [...rows, { id: rid(), name: '', value: '', locked: false }])}><Icon.plus />Add param</button>
              </div>
            )}

            {reqTab === 'headers' && (
              <div className="kv-block">
                {headerRows.map(row => (
                  <div className={`kvrow has-check ${row.enabled ? '' : 'dim'}`} key={row.id}>
                    <button className={`kv-check ${row.enabled ? 'on' : ''}`} onClick={() => setHeaderRows(rows => rows.map(item => item.id === row.id ? { ...item, enabled: !item.enabled } : item))}>{row.enabled ? <Icon.check /> : null}</button>
                    <div className="kvrow-name"><input className="kvi mono hdr" value={row.name} placeholder="Header-Name" onChange={event => setHeaderRows(rows => rows.map(item => item.id === row.id ? { ...item, name: event.target.value } : item))} /></div>
                    <div className="kvrow-val"><input className="kvi mono" value={row.value} placeholder="value" onChange={event => setHeaderRows(rows => rows.map(item => item.id === row.id ? { ...item, value: event.target.value } : item))} /></div>
                    <span />
                    <button className="row-del always" onClick={() => setHeaderRows(rows => rows.filter(item => item.id !== row.id))}><Icon.x /></button>
                  </div>
                ))}
                <div className="inline-actions">
                  <button className="add-row" onClick={() => setHeaderRows(rows => [...rows, { id: rid(), name: '', value: '', enabled: true, locked: false }])}><Icon.plus />Add header</button>
                  <HeaderSuggestions
                    existing={headerRows}
                    onPick={(name, value) => setHeaderRows(rows => [...rows, { id: rid(), name, value, enabled: true, locked: false }])}
                  />
                </div>
              </div>
            )}

            {reqTab === 'body' && (
              <div className="body-block">
                <div className="bb-head">
                  <span className="chip"><span className="dot" style={{ background: 'var(--info)' }} />JSON</span>
                  {bodyDirty && <span className="edited-dot">edited</span>}
                  {parsed.body && bodyDirty && <button className="btn ghost sm" style={{ marginLeft: 'auto' }} onClick={() => { setBodyText(parsed.body); setBodyDirty(false); }}>Reset to spec body</button>}
                </div>
                <JsonEditor value={bodyText} onChange={value => { setBodyText(value); setBodyDirty(value !== parsed.body); }} placeholder="Empty body" minHeight={240} variableNames={availableVariableNames} />
              </div>
            )}

            {reqTab === 'expected' && <CodeOrEmpty value={spec.expected_response} empty="No expected response defined." />}
            {reqTab === 'tests' && <CodeOrEmpty value={spec.tests ?? ''} empty="No tests yet." />}
            {reqTab === 'source' && (
              <div className="source-edit-pane">
                <textarea className="input mono source-editor" value={sourceText} onChange={event => setSourceText(event.target.value)} spellCheck={false} />
              </div>
            )}
          </div>
        </section>

        <section className="rp-pane response">
          <div className="rp-pane-h">
            <span className="rp-pane-label">Response</span>
            {result?.response && (
              <span className={`resp-pill ${result.diff.passed ? 'ok' : 'fail'}`}>
                {result.diff.passed ? <Icon.check /> : <Icon.cross />}
                {result.response.status}
                <span className="dim">· {result.duration_ms}ms</span>
              </span>
            )}
          </div>
          <div className="rp-pane-b resp-b">
            {!result && !running && (
              <div className="resp-empty">
                <div className="resp-empty-ic"><Icon.play /></div>
                <div className="resp-empty-t">Ready to send</div>
                <div className="resp-empty-s">Hit Send to run this {spec.method} request against <b>{env}</b> and inspect the diff against the expected response.</div>
                <button className="btn-primary" onClick={run} disabled={!canSend}><Icon.play />Send request</button>
              </div>
            )}
            {running && (
              <div className="resp-empty">
                <span className="pulse-dot" style={{ width: 10, height: 10 }} />
                <div className="resp-empty-t" style={{ marginTop: 12 }}>{isMock ? 'Loading mock...' : `Sending ${spec.method}...`}</div>
                <div className="resp-empty-s mono">{displayUrl}</div>
              </div>
            )}
            {result && !running && (
              <ResultCard result={result} resultTab={resultTab} setResultTab={setResultTab} spec={spec} />
            )}
          </div>
        </section>
      </div>

      {reqTab === 'source' && (
        <div className="rp-savebar">
          <span className="note">{saveMsg === 'Saved' ? <span className="ok-text"><Icon.check />Saved</span> : saveMsg ? <span className="fail-text">{saveMsg}</span> : 'Editing markdown source'}</span>
          <button className="btn" onClick={() => setSourceText(spec.raw_source)}>Discard</button>
          <button className="btn-primary btn-sm-primary" onClick={() => saveSource()}><Icon.check />Save changes</button>
        </div>
      )}

      {curlOpen && <CurlPreview curl={buildCurl(spec, envVars, buildOptions())} onClose={() => setCurlOpen(false)} />}
    </div>
  );
}

function CodeOrEmpty({ value, empty }: { value: string; empty: string }) {
  if (!value.trim()) return <div className="kv-none" style={{ padding: 16 }}>{empty}</div>;
  return <pre className="code" dangerouslySetInnerHTML={{ __html: highlight(value) }} />;
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

function ResultCard({ result, resultTab, setResultTab, spec }: {
  result: ExecResult;
  resultTab: 'diff' | 'response' | 'headers' | 'request';
  setResultTab: (tab: 'diff' | 'response' | 'headers' | 'request') => void;
  spec: SpecData;
}) {
  const passed = result.diff?.passed ?? false;
  const tabs = [
    { id: 'diff' as const, label: 'Diff' },
    { id: 'response' as const, label: 'Body' },
    { id: 'headers' as const, label: 'Headers', badge: Object.keys(result.response?.headers ?? {}).length },
    { id: 'request' as const, label: 'Request' },
  ];
  return (
    <div className="result-card embedded">
      <div className={`result-status ${passed ? 'ok' : 'fail'}`}>
        <span className="verdict">{passed ? <Icon.check /> : <Icon.cross />}{passed ? 'Passed' : 'Failed'}</span>
        {result.mock && <span className="mock-badge">MOCK</span>}
        <span className="sep">·</span><span>{result.response?.status ?? 'No response'}</span>
        <span className="sep">·</span><span>{result.duration_ms}ms</span>
      </div>
      <div className="tabs flush">
        {tabs.map(tab => <button key={tab.id} className={`tab ${resultTab === tab.id ? 'is-on' : ''}`} onClick={() => setResultTab(tab.id)}>{tab.label}{tab.badge != null && tab.badge > 0 && <span className="badge">{tab.badge}</span>}</button>)}
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
      <div className="modal" onClick={event => event.stopPropagation()}>
        <div className="modal-h"><Icon.copy /><div><h2>Copy as curl</h2><div className="sub">Generated with current runtime overrides.</div></div><button className="btn icon" onClick={onClose}><Icon.x /></button></div>
        <div className="modal-b"><pre className="code">{curl}</pre></div>
        <div className="modal-f"><div className="note">Unresolved variables remain as placeholders.</div><button className="btn-primary btn-sm-primary" onClick={() => navigator.clipboard?.writeText(curl)}>Copy</button></div>
      </div>
    </div>
  );
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
