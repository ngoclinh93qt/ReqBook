import { useState } from 'react';
import { useLocation } from 'react-router-dom';
import { api } from '../api';
import type { AdHocResponse, VarsData } from '../types';
import { Icon, highlight } from '../ui';
import type { RequestInitState } from '../RequestBuilder';

const rid = () => Math.random().toString(36).slice(2, 9);
type KVRow = { id: string; name: string; value: string };
type ReqTab = 'headers' | 'body' | 'vars' | 'save';

const HTTP_METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'];
const BODY_METHODS = new Set(['POST', 'PUT', 'PATCH']);

function kvFromRecord(rec: Record<string, string>): KVRow[] {
  return Object.entries(rec).map(([name, value]) => ({ id: rid(), name, value }));
}

export function RequestPage({ env }: {
  env: string;
  varsData: VarsData | null;
}) {
  const location = useLocation();
  const init = location.state as RequestInitState | null;

  const [method, setMethod] = useState(init?.method ?? 'GET');
  const [url, setUrl] = useState(init?.url ?? '');
  const [headerRows, setHeaderRows] = useState<KVRow[]>(() =>
    init?.headers?.map(([name, value]) => ({ id: rid(), name, value })) ?? []
  );
  const [varRows, setVarRows] = useState<KVRow[]>([]);
  const [body, setBody] = useState(init?.body ?? '');
  const [saveAs, setSaveAs] = useState('');
  const [reqTab, setReqTab] = useState<ReqTab>(init?.body ? 'body' : init?.headers?.length ? 'headers' : 'headers');

  const [running, setRunning] = useState(false);
  const [curlParsing, setCurlParsing] = useState(false);
  const [result, setResult] = useState<AdHocResponse | null>(null);
  const [resultTab, setResultTab] = useState<'response' | 'headers' | 'request'>('response');
  const [saveMsg, setSaveMsg] = useState('');
  const [error, setError] = useState('');

  const needsBody = BODY_METHODS.has(method);

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

  const safeReqTab: ReqTab = (!needsBody && reqTab === 'body') ? 'headers' : reqTab;

  return (
    <div className="spec-wrap">
      <div className="card" style={{ marginBottom: 16 }}>

        {/* ── Method · URL · Send ── */}
        <div style={{ display: 'flex', gap: 6, padding: '10px 12px', alignItems: 'center' }}>
          <select
            className="input flush mono"
            value={method}
            onChange={e => setMethod(e.target.value)}
            style={{ width: '6.8rem', flexShrink: 0 }}
          >
            {HTTP_METHODS.map(m => <option key={m} value={m}>{m}</option>)}
          </select>
          <input
            className="input flush mono"
            value={url}
            placeholder="https://api.example.com/users  ·  or paste a curl command"
            onChange={e => setUrl(e.target.value)}
            onPaste={handleUrlPaste}
            onKeyDown={e => { if (e.key === 'Enter') send(); }}
            style={{ flex: 1, minWidth: 0 }}
          />
          <button
            className="btn-primary"
            onClick={send}
            disabled={running || curlParsing || !url.trim()}
            style={{ flexShrink: 0, whiteSpace: 'nowrap' }}
          >
            {running
              ? <><span className="pulse-dot" />Sending…</>
              : curlParsing
                ? <>Parsing…</>
                : <><Icon.play />Send</>}
          </button>
        </div>

        {/* ── Tabs ── */}
        <div className="tabs" style={{ borderTop: '1px solid var(--c-border)' }}>
          <button
            className={`tab ${safeReqTab === 'headers' ? 'is-on' : ''}`}
            onClick={() => setReqTab('headers')}
          >
            Headers
            {headerRows.length > 0 && <span className="badge">{headerRows.length}</span>}
          </button>
          {needsBody && (
            <button
              className={`tab ${safeReqTab === 'body' ? 'is-on' : ''}`}
              onClick={() => setReqTab('body')}
            >
              Body
            </button>
          )}
          <button
            className={`tab ${safeReqTab === 'vars' ? 'is-on' : ''}`}
            onClick={() => setReqTab('vars')}
          >
            Variables
            {varRows.length > 0 && <span className="badge">{varRows.length}</span>}
          </button>
          <button
            className={`tab ${safeReqTab === 'save' ? 'is-on' : ''}`}
            onClick={() => setReqTab('save')}
          >
            Save
          </button>
        </div>

        {/* ── Tab content ── */}
        <div style={{ padding: '8px 12px 12px' }}>
          {safeReqTab === 'headers' && (
            <>
              <div className="kv-list">
                {headerRows.map(row => (
                  <div className="kv hdr" key={row.id}>
                    <input className="input flush mono info-input" value={row.name} placeholder="Header-Name"
                      onChange={e => setHeaderRows(rows => rows.map(r => r.id === row.id ? { ...r, name: e.target.value } : r))} />
                    <input className="input flush mono" value={row.value} placeholder="value"
                      onChange={e => setHeaderRows(rows => rows.map(r => r.id === row.id ? { ...r, value: e.target.value } : r))} />
                    <button className="row-del always" onClick={() => setHeaderRows(rows => rows.filter(r => r.id !== row.id))}><Icon.x /></button>
                  </div>
                ))}
              </div>
              <button className="add-row" onClick={() => setHeaderRows(rows => [...rows, { id: rid(), name: '', value: '' }])}>
                <Icon.plus /> Add header
              </button>
            </>
          )}

          {safeReqTab === 'body' && needsBody && (
            <textarea
              className="input mono source-editor"
              value={body}
              onChange={e => setBody(e.target.value)}
              placeholder='{"key": "value"}'
              rows={7}
              spellCheck={false}
            />
          )}

          {safeReqTab === 'vars' && (
            <>
              <div className="kv-list">
                {varRows.map(row => (
                  <div className="kv" key={row.id}>
                    <input className="input flush mono" value={row.name} placeholder="name"
                      onChange={e => setVarRows(rows => rows.map(r => r.id === row.id ? { ...r, name: e.target.value } : r))} />
                    <input className="input flush mono" value={row.value} placeholder="value"
                      onChange={e => setVarRows(rows => rows.map(r => r.id === row.id ? { ...r, value: e.target.value } : r))} />
                    <button className="row-del always" onClick={() => setVarRows(rows => rows.filter(r => r.id !== row.id))}><Icon.x /></button>
                  </div>
                ))}
              </div>
              <button className="add-row" onClick={() => setVarRows(rows => [...rows, { id: rid(), name: '', value: '' }])}>
                <Icon.plus /> Add variable
              </button>
            </>
          )}

          {safeReqTab === 'save' && (
            <input
              className="input flush mono"
              value={saveAs}
              placeholder="apis/users/get-users.md  (leave empty to save to scratch)"
              onChange={e => setSaveAs(e.target.value)}
            />
          )}
        </div>
      </div>

      {/* ── Response ── */}
      {running && !result && (
        <div className="card result-card">
          <div className="empty compact"><span className="pulse-dot" />Sending {method}…</div>
        </div>
      )}
      {error && (
        <div className="card result-card">
          <div className="result-status fail"><span className="verdict"><Icon.cross />Error</span></div>
          <pre className="code fail-text">{error}</pre>
        </div>
      )}
      {result && (
        <AdHocResultCard
          result={result}
          resultTab={resultTab}
          setResultTab={setResultTab}
          saveMsg={saveMsg}
        />
      )}
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

  const statusOk = result.response && result.response.status < 400;

  return (
    <div className="card result-card">
      <div className={`result-status ${statusOk ? 'ok' : 'fail'}`}>
        <span className="verdict">
          {statusOk ? <Icon.check /> : <Icon.cross />}
          {result.response?.status ?? 'No response'}
        </span>
        <span className="sep">·</span>
        <span>{result.duration_ms}ms</span>
        {saveMsg && <><span className="sep">·</span><span className="ok-text">{saveMsg}</span></>}
      </div>
      <div className="tabs">
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
