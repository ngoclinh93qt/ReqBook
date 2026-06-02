import { useMemo, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { api } from '../api';
import type { AdHocResponse, VarsData } from '../types';
import { Icon, JsonEditor, highlight } from '../ui';

const rid = () => Math.random().toString(36).slice(2, 9);
type KVRow = { id: string; name: string; value: string };
type ReqTab = 'headers' | 'body' | 'vars' | 'save';
type RequestInitState = {
  method: string;
  url: string;
  headers: [string, string][];
  body?: string;
};

const HTTP_METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'];
const BODY_METHODS = new Set(['POST', 'PUT', 'PATCH']);

function kvFromRecord(rec: Record<string, string>): KVRow[] {
  return Object.entries(rec).map(([name, value]) => ({ id: rid(), name, value }));
}

export function RequestPage({ env, varsData: _varsData }: {
  env: string;
  varsData: VarsData | null;
}) {
  const navigate = useNavigate();
  const location = useLocation();
  const init = location.state as RequestInitState | null;

  const [title, setTitle] = useState('Untitled request');
  const [method, setMethod] = useState(init?.method ?? 'GET');
  const [url, setUrl] = useState(init?.url ?? '');
  const [headerRows, setHeaderRows] = useState<KVRow[]>(() =>
    init?.headers?.map(([name, value]) => ({ id: rid(), name, value })) ?? []
  );
  const [varRows, setVarRows] = useState<KVRow[]>([]);
  const [body, setBody] = useState(init?.body ?? '');
  const [saveAs, setSaveAs] = useState('');
  const [reqTab, setReqTab] = useState<ReqTab>(init?.body ? 'body' : 'headers');
  const [varsOpen, setVarsOpen] = useState(true);

  const [running, setRunning] = useState(false);
  const [curlParsing, setCurlParsing] = useState(false);
  const [result, setResult] = useState<AdHocResponse | null>(null);
  const [resultTab, setResultTab] = useState<'response' | 'headers' | 'request'>('response');
  const [saveMsg, setSaveMsg] = useState('');
  const [error, setError] = useState('');

  const needsBody = BODY_METHODS.has(method);
  const safeReqTab: ReqTab = (!needsBody && reqTab === 'body') ? 'headers' : reqTab;

  const variableNames = useMemo(() => {
    const names = new Set<string>();
    const scan = (text: string) => {
      for (const match of text.matchAll(/\{\{([a-zA-Z0-9_]+)\}\}/g)) names.add(match[1]);
    };
    scan(url);
    scan(body);
    headerRows.forEach(row => {
      scan(row.name);
      scan(row.value);
    });
    return Array.from(names);
  }, [body, headerRows, url]);

  async function handleUrlPaste(e: React.ClipboardEvent<HTMLInputElement>) {
    const text = e.clipboardData.getData('text');
    if (!text.trimStart().startsWith('curl ')) return;
    e.preventDefault();
    setCurlParsing(true);
    try {
      const parsed = await api.parseCurl(text);
      setMethod(parsed.method);
      setUrl(parsed.url);
      setHeaderRows(kvFromRecord(parsed.headers));
      if (parsed.body) {
        setBody(parsed.body);
        setReqTab('body');
      } else if (Object.keys(parsed.headers).length > 0) {
        setReqTab('headers');
      }
    } catch {
      setUrl(text);
    } finally {
      setCurlParsing(false);
    }
  }

  async function send() {
    if (!url.trim()) return;
    setRunning(true);
    setResult(null);
    setError('');
    setSaveMsg('');
    try {
      const headers = Object.fromEntries(
        headerRows.filter(r => r.name.trim()).map(r => [r.name.trim(), r.value])
      );
      const vars = Object.fromEntries(
        varRows.filter(r => r.name.trim()).map(r => [r.name.trim(), r.value])
      );
      const res = await api.sendRequest({
        method,
        url: url.trim(),
        headers,
        body: (needsBody && body.trim()) ? body.trim() : undefined,
        vars,
        env,
        save_as: saveAs.trim() || undefined,
      });
      setResult(res);
      setResultTab('response');
      if (res.saved_path) setSaveMsg(`Saved: ${res.saved_path}`);
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  }

  return (
    <div className="req-page">
      <div className="rp-crumb">
        <button className="rp-back" onClick={() => navigate('/')}>
          <svg width="11" height="11" viewBox="0 0 11 11" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><line x1="9" y1="5.5" x2="2" y2="5.5" /><polyline points="5,3 2,5.5 5,8" /></svg>
          Collections
        </button>
        <span className="sep">/</span>
        <span className="rp-rel">new request</span>
        <span className="chip"><span className="dot" style={{ background: 'var(--accent)' }} />Draft</span>
      </div>

      <div className="rp-titles">
        <input
          className="rp-title-input"
          value={title}
          placeholder="Untitled request"
          onChange={event => setTitle(event.target.value)}
        />
        <p className="rp-desc">Build a request by hand, or paste a curl command into the URL bar to auto-fill method, headers, and body.</p>
      </div>

      <div className="rp-bar">
        <select className={`rp-method-select m-${method.toLowerCase()}`} value={method} onChange={event => setMethod(event.target.value)}>
          {HTTP_METHODS.map(item => <option key={item} value={item}>{item}</option>)}
        </select>
        <div className="rp-url ad-hoc">
          <input
            className="rp-url-input"
            value={url}
            placeholder="https://api.example.com/users  ·  or paste a curl command"
            onChange={event => setUrl(event.target.value)}
            onPaste={handleUrlPaste}
            onKeyDown={event => { if (event.key === 'Enter') send(); }}
            spellCheck={false}
          />
        </div>
        <button className="btn-primary rp-send" onClick={send} disabled={running || curlParsing || !url.trim()}>
          {running ? <><span className="pulse-dot" />Sending...</> : curlParsing ? <>Parsing...</> : <><Icon.play />Send</>}
        </button>
      </div>

      <div className={`var-strip ${varsOpen ? 'open' : ''}`}>
        <button className="vs-head" onClick={() => setVarsOpen(open => !open)}>
          <span className="chev"><Icon.chev open={varsOpen} /></span>
          <Icon.vars />
          <span className="vs-title">Variables</span>
          <span className="vs-count">{variableNames.length + varRows.length}</span>
          <span className="vs-hint">resolved against <b>{env}</b></span>
        </button>
        {varsOpen && (
          <div className="vs-body">
            {variableNames.length === 0 && varRows.length === 0 && (
              <div className="vs-empty">No variables detected. Use <code>{'{{name}}'}</code> in the URL, headers, or body.</div>
            )}
            {variableNames.map(name => (
              <div className="vrow" key={name}>
                <div className="vrow-name"><span className="br">{'{{'}</span>{name}<span className="br">{'}}'}</span></div>
                <input className="vrow-input" value={varRows.find(row => row.name === name)?.value ?? ''} placeholder="runtime value" onChange={event => {
                  const value = event.target.value;
                  setVarRows(rows => {
                    const exists = rows.some(row => row.name === name);
                    if (exists) return rows.map(row => row.name === name ? { ...row, value } : row);
                    return [...rows, { id: rid(), name, value }];
                  });
                }} />
                <span className="src runtime"><span className="dot" />runtime</span>
                <span />
              </div>
            ))}
            {varRows.filter(row => !variableNames.includes(row.name)).map(row => (
              <div className="vrow" key={row.id}>
                <input className="vrow-input name" value={row.name} placeholder="name" onChange={event => setVarRows(rows => rows.map(item => item.id === row.id ? { ...item, name: event.target.value } : item))} />
                <input className="vrow-input" value={row.value} placeholder="value" onChange={event => setVarRows(rows => rows.map(item => item.id === row.id ? { ...item, value: event.target.value } : item))} />
                <span className="src runtime"><span className="dot" />runtime</span>
                <button className="row-del always" onClick={() => setVarRows(rows => rows.filter(item => item.id !== row.id))}><Icon.x /></button>
              </div>
            ))}
            <button className="add-row" onClick={() => setVarRows(rows => [...rows, { id: rid(), name: '', value: '' }])}><Icon.plus />Add variable</button>
          </div>
        )}
      </div>

      <div className="rp-split">
        <section className="rp-pane">
          <div className="rp-pane-h">
            <div className="tabs flush">
              {(['headers', ...(needsBody ? ['body'] : []), 'vars', 'save'] as ReqTab[]).map(tab => (
                <button key={tab} className={`tab ${safeReqTab === tab ? 'is-on' : ''}`} onClick={() => setReqTab(tab)}>
                  {tab === 'vars' ? 'Variables' : tab[0].toUpperCase() + tab.slice(1)}
                  {tab === 'headers' && headerRows.length > 0 && <span className="badge">{headerRows.length}</span>}
                  {tab === 'vars' && varRows.length > 0 && <span className="badge">{varRows.length}</span>}
                </button>
              ))}
            </div>
          </div>
          <div className="rp-pane-b">
            {safeReqTab === 'headers' && (
              <div className="kv-block">
                {headerRows.length === 0 && <div className="kv-none">No headers yet.</div>}
                {headerRows.map(row => (
                  <div className="kvrow" key={row.id}>
                    <div className="kvrow-name"><input className="kvi mono hdr" value={row.name} placeholder="Header-Name" onChange={event => setHeaderRows(rows => rows.map(item => item.id === row.id ? { ...item, name: event.target.value } : item))} /></div>
                    <div className="kvrow-val"><input className="kvi mono" value={row.value} placeholder="value" onChange={event => setHeaderRows(rows => rows.map(item => item.id === row.id ? { ...item, value: event.target.value } : item))} /></div>
                    <span />
                    <button className="row-del always" onClick={() => setHeaderRows(rows => rows.filter(item => item.id !== row.id))}><Icon.x /></button>
                  </div>
                ))}
                <button className="add-row" onClick={() => setHeaderRows(rows => [...rows, { id: rid(), name: '', value: '' }])}><Icon.plus />Add header</button>
              </div>
            )}
            {safeReqTab === 'body' && needsBody && (
              <div className="body-block">
                <div className="bb-head"><span className="chip"><span className="dot" style={{ background: 'var(--info)' }} />JSON</span></div>
                <JsonEditor value={body} onChange={setBody} placeholder={'{\n  "key": "value"\n}'} minHeight={240} variableNames={variableNames} />
              </div>
            )}
            {safeReqTab === 'vars' && (
              <div className="kv-block">
                {varRows.length === 0 && <div className="kv-none">No runtime variables added.</div>}
                {varRows.map(row => (
                  <div className="kvrow" key={row.id}>
                    <div className="kvrow-name"><input className="kvi mono" value={row.name} placeholder="name" onChange={event => setVarRows(rows => rows.map(item => item.id === row.id ? { ...item, name: event.target.value } : item))} /></div>
                    <div className="kvrow-val"><input className="kvi mono" value={row.value} placeholder="value" onChange={event => setVarRows(rows => rows.map(item => item.id === row.id ? { ...item, value: event.target.value } : item))} /></div>
                    <span />
                    <button className="row-del always" onClick={() => setVarRows(rows => rows.filter(item => item.id !== row.id))}><Icon.x /></button>
                  </div>
                ))}
                <button className="add-row" onClick={() => setVarRows(rows => [...rows, { id: rid(), name: '', value: '' }])}><Icon.plus />Add variable</button>
              </div>
            )}
            {safeReqTab === 'save' && (
              <div className="kv-block">
                <div className="kv-sub">Save as spec</div>
                <input
                  className="input flush mono save-path-input"
                  value={saveAs}
                  placeholder="apis/users/get-users.md  (leave empty to save to scratch)"
                  onChange={event => setSaveAs(event.target.value)}
                />
              </div>
            )}
          </div>
        </section>

        <section className="rp-pane response">
          <div className="rp-pane-h">
            <span className="rp-pane-label">Response</span>
            {result?.response && (
              <span className={`resp-pill ${result.response.status < 400 ? 'ok' : 'fail'}`}>
                {result.response.status < 400 ? <Icon.check /> : <Icon.cross />}
                {result.response.status}
                <span className="dim">· {result.duration_ms}ms</span>
              </span>
            )}
          </div>
          <div className="rp-pane-b resp-b">
            {!result && !running && !error && (
              <div className="resp-empty">
                <div className="resp-empty-ic"><Icon.play /></div>
                <div className="resp-empty-t">Ready to send</div>
                <div className="resp-empty-s">Send this {method} request against <b>{env}</b> to inspect body, headers, and outgoing request details.</div>
                <button className="btn-primary" onClick={send} disabled={!url.trim()}><Icon.play />Send request</button>
              </div>
            )}
            {running && (
              <div className="resp-empty">
                <span className="pulse-dot" style={{ width: 10, height: 10 }} />
                <div className="resp-empty-t" style={{ marginTop: 12 }}>Sending {method}...</div>
                <div className="resp-empty-s mono">{url}</div>
              </div>
            )}
            {error && (
              <div className="result-card embedded">
                <div className="result-status fail"><span className="verdict"><Icon.cross />Error</span></div>
                <pre className="code fail-text">{error}</pre>
              </div>
            )}
            {result && !running && (
              <AdHocResultCard
                result={result}
                resultTab={resultTab}
                setResultTab={setResultTab}
                saveMsg={saveMsg}
              />
            )}
          </div>
        </section>
      </div>
    </div>
  );
}

function AdHocResultCard({ result, resultTab, setResultTab, saveMsg }: {
  result: AdHocResponse;
  resultTab: 'response' | 'headers' | 'request';
  setResultTab: (tab: 'response' | 'headers' | 'request') => void;
  saveMsg: string;
}) {
  const tabs = [
    { id: 'response' as const, label: 'Body' },
    { id: 'headers' as const, label: 'Headers', badge: Object.keys(result.response?.headers ?? {}).length },
    { id: 'request' as const, label: 'Request' },
  ];

  return (
    <div className="result-card embedded">
      <div className={`result-status ${result.response && result.response.status < 400 ? 'ok' : 'fail'}`}>
        <span className="verdict">
          {result.response && result.response.status < 400 ? <Icon.check /> : <Icon.cross />}
          {result.response?.status ?? 'No response'}
        </span>
        <span className="sep">·</span>
        <span>{result.duration_ms}ms</span>
        {saveMsg && <><span className="sep">·</span><span className="ok-text">{saveMsg}</span></>}
      </div>
      <div className="tabs flush">
        {tabs.map(tab => (
          <button key={tab.id} className={`tab ${resultTab === tab.id ? 'is-on' : ''}`} onClick={() => setResultTab(tab.id)}>
            {tab.label}
            {tab.badge != null && tab.badge > 0 && <span className="badge">{tab.badge}</span>}
          </button>
        ))}
      </div>
      {resultTab === 'response' && (
        <pre className="code" dangerouslySetInnerHTML={{ __html: highlight(formatBody(result.response?.body ?? result.error ?? '')) }} />
      )}
      {resultTab === 'headers' && (
        <div className="headers-list">
          {Object.entries(result.response?.headers ?? {}).map(([key, value]) => (
            <div className="kv no-del" key={key}>
              <div className="k info-text">{key}</div>
              <div className="header-value">{value}</div>
            </div>
          ))}
        </div>
      )}
      {resultTab === 'request' && (
        <pre className="code" dangerouslySetInnerHTML={{ __html: highlight(formatReq(result)) }} />
      )}
    </div>
  );
}

function formatBody(body: string) {
  try { return JSON.stringify(JSON.parse(body), null, 2); } catch { return body || 'No response body.'; }
}

function formatReq(result: AdHocResponse) {
  if (!result.request) return 'No request captured.';
  const hdrs = Object.entries(result.request.headers).map(([k, v]) => `${k}: ${v}`).join('\n');
  return `${result.request.method} ${result.request.url}\n${hdrs}\n\n${result.request.body}`;
}
