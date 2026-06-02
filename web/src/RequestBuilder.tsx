import { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { api } from './api';
import { highlight, Icon, MethodBadge } from './ui';

function looksLikeCurl(text: string) {
  return /^\s*curl\b/i.test(text || '');
}

function rid() { return Math.random().toString(36).slice(2, 9); }

type KvRow = { id: string; key: string; value: string; on: boolean };

export type RequestInitState = {
  method: string;
  url: string;
  headers: [string, string][];
  body?: string;
};

// ── KvEditor — Postman-style key/value rows ───────────────────────────────────
export function KvEditor({ rows, setRows, keyPlaceholder = 'key', valuePlaceholder = 'value', mono = false }: {
  rows: KvRow[];
  setRows: (fn: (rows: KvRow[]) => KvRow[]) => void;
  keyPlaceholder?: string;
  valuePlaceholder?: string;
  mono?: boolean;
}) {
  function update(id: string, patch: Partial<KvRow>) {
    setRows(prev => {
      let next = prev.map(r => r.id === id ? { ...r, ...patch } : r);
      next = next.filter((r, i) => r.key || r.value || i === next.length - 1);
      const last = next[next.length - 1];
      if (last && (last.key || last.value)) next.push({ id: rid(), key: '', value: '', on: true });
      return next;
    });
  }
  function remove(id: string) {
    setRows(prev => {
      const next = prev.filter(r => r.id !== id);
      return next.length ? next : [{ id: rid(), key: '', value: '', on: true }];
    });
  }
  return (
    <div className="kv-editor">
      <div className="kv-head">
        <span /><span>Key</span><span>Value</span><span />
      </div>
      {rows.map(r => {
        const blank = !r.key && !r.value;
        return (
          <div key={r.id} className={`kv-row${blank ? ' is-blank' : ''}`}>
            <button
              className={`kv-check${r.on ? ' on' : ''}`}
              onClick={() => update(r.id, { on: !r.on })}
              disabled={blank}
              title={r.on ? 'Enabled' : 'Disabled'}
            >{r.on && !blank ? <Icon.check /> : null}</button>
            <input
              className={`kv-input${mono ? ' mono' : ''}`}
              value={r.key}
              onChange={e => update(r.id, { key: e.target.value })}
              placeholder={keyPlaceholder}
              spellCheck={false}
            />
            <input
              className="kv-input mono"
              value={r.value}
              onChange={e => update(r.id, { value: e.target.value })}
              placeholder={valuePlaceholder}
              spellCheck={false}
            />
            <button className="kv-del" onClick={() => remove(r.id)} disabled={blank} title="Remove">
              <Icon.x />
            </button>
          </div>
        );
      })}
    </div>
  );
}

// ── RequestBuilderModal ───────────────────────────────────────────────────────
const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE'];

export function RequestBuilderModal({ onClose }: { onClose: () => void }) {
  const navigate = useNavigate();
  const [method, setMethod] = useState('GET');
  const [url, setUrl] = useState('');
  const [params, setParams] = useState<KvRow[]>([{ id: rid(), key: '', value: '', on: true }]);
  const [headers, setHeaders] = useState<KvRow[]>([
    { id: rid(), key: 'Authorization', value: 'Bearer {{token}}', on: true },
    { id: rid(), key: '', value: '', on: true },
  ]);
  const [body, setBody] = useState('');
  const [tab, setTab] = useState<'params' | 'headers' | 'body'>('params');
  const [flash, setFlash] = useState(false);
  const [methodOpen, setMethodOpen] = useState(false);
  const [parsing, setParsing] = useState(false);
  const urlRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    urlRef.current?.focus();
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [onClose]);

  const bodyAllowed = method !== 'GET' && method !== 'DELETE';

  useEffect(() => {
    if (!bodyAllowed && tab === 'body') setTab('params');
  }, [bodyAllowed, tab]);

  async function applyCurlText(text: string) {
    setParsing(true);
    try {
      const parsed = await api.parseCurl(text);
      setMethod(parsed.method);
      setUrl(parsed.url);
      const hRows: KvRow[] = Object.entries(parsed.headers).map(([key, value]) => ({ id: rid(), key, value, on: true }));
      hRows.push({ id: rid(), key: '', value: '', on: true });
      setHeaders(hRows);
      if (parsed.body) { setBody(parsed.body); setTab('body'); }
      setFlash(true);
      setTimeout(() => setFlash(false), 2600);
    } catch {}
    finally { setParsing(false); }
  }

  async function onUrlPaste(e: React.ClipboardEvent<HTMLInputElement>) {
    const text = e.clipboardData.getData('text');
    if (!looksLikeCurl(text)) return;
    e.preventDefault();
    setUrl(text);
    await applyCurlText(text);
  }

  const activeParams = params.filter(p => p.on && p.key.trim());
  const activeHeaders = headers.filter(h => h.on && h.key.trim());

  const previewUrl = useMemo(() => {
    if (!url || looksLikeCurl(url)) return '';
    let u = url;
    if (activeParams.length) {
      u += (u.includes('?') ? '&' : '?') + activeParams.map(p =>
        `${encodeURIComponent(p.key)}=${encodeURIComponent(p.value)}`
      ).join('&');
    }
    return u;
  }, [url, params, activeParams]);

  const valid = url.trim().length > 0 && !looksLikeCurl(url);

  function openInRunner() {
    const state: RequestInitState = {
      method,
      url: previewUrl || url,
      headers: activeHeaders.map(h => [h.key, h.value]),
      body: bodyAllowed ? body : undefined,
    };
    onClose();
    navigate('/request', { state });
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal rb-modal" style={{ width: 'min(840px, 100%)' }} onClick={e => e.stopPropagation()}>
        <div className="modal-h">
          <span className="modal-icon"><Icon.bolt2 /></span>
          <div>
            <h2>New request</h2>
            <div className="sub">Build a request by hand, or paste a curl command to auto-fill.</div>
          </div>
          <button className="btn icon" style={{ marginLeft: 'auto' }} onClick={onClose}><Icon.x /></button>
        </div>

        <div className="modal-b" style={{ padding: 18 }}>
          <div className="rb-bar">
            <div className="rb-method">
              <button
                className={`rb-method-btn m-${method.toLowerCase()}`}
                onClick={() => setMethodOpen(o => !o)}
              >
                {method}
                <svg width="9" height="9" viewBox="0 0 10 10" fill="none" style={{ marginLeft: 2 }}>
                  <path d="M2 4 L5 7 L8 4" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
                </svg>
              </button>
              {methodOpen && (
                <div className="rb-method-menu" onMouseLeave={() => setMethodOpen(false)}>
                  {METHODS.map(m => (
                    <button
                      key={m}
                      className={`m-${m.toLowerCase()}${m === method ? ' is-on' : ''}`}
                      onClick={() => { setMethod(m); setMethodOpen(false); }}
                    >{m}</button>
                  ))}
                </div>
              )}
            </div>
            <input
              ref={urlRef}
              className="rb-url"
              value={url}
              onChange={e => setUrl(e.target.value)}
              onPaste={onUrlPaste}
              placeholder="Paste a curl command, or type an endpoint URL…"
              spellCheck={false}
            />
            {url && (
              <button className="rb-clear" onClick={() => setUrl('')}><Icon.x /></button>
            )}
          </div>

          {flash && (
            <div className="rb-flash fade-in">
              <Icon.check /> Parsed curl — filled method, headers{body ? ', and body' : ''}. Review below.
            </div>
          )}
          {previewUrl && !flash && (
            <div className="rb-preview">
              <span className="lab">{'>'}</span>
              <MethodBadge method={method} />
              <span className="url mono" dangerouslySetInnerHTML={{ __html: highlight(previewUrl) }} />
            </div>
          )}

          <div className="rb-tabs">
            {(['params', 'headers', 'body'] as const).map(t => (
              <button
                key={t}
                className={`rb-tab${tab === t ? ' is-on' : ''}`}
                onClick={() => setTab(t)}
                disabled={t === 'body' && !bodyAllowed}
              >
                {t === 'params' ? 'Params' : t === 'headers' ? 'Headers' : 'Body'}
                {t === 'params' && activeParams.length > 0 && <span className="badge">{activeParams.length}</span>}
                {t === 'headers' && activeHeaders.length > 0 && <span className="badge">{activeHeaders.length}</span>}
                {t === 'body' && body && bodyAllowed && <span className="badge">1</span>}
              </button>
            ))}
          </div>

          <div className="rb-pane">
            {tab === 'params' && (
              <KvEditor rows={params} setRows={setParams} keyPlaceholder="param" valuePlaceholder="value" />
            )}
            {tab === 'headers' && (
              <KvEditor rows={headers} setRows={setHeaders} keyPlaceholder="Header-Name" valuePlaceholder="value" mono />
            )}
            {tab === 'body' && bodyAllowed && (
              <BodyEditor value={body} onChange={setBody} />
            )}
            {tab === 'body' && !bodyAllowed && (
              <div className="rb-empty">{method} requests don't send a body.</div>
            )}
          </div>

          {parsing && (
            <div className="rb-flash fade-in" style={{ background: 'var(--bg-soft)', color: 'var(--fg-3)' }}>
              <span className="pulse-dot" style={{ background: 'var(--accent)' }} /> Parsing curl…
            </div>
          )}
        </div>

        <div className="modal-f">
          <div className="note">
            {valid
              ? <><span className="pulse-dot" style={{ background: 'var(--ok)', display: 'inline-block', verticalAlign: 'middle', marginRight: 6 }} />Ready — opens in the runner</>
              : 'Enter a URL or paste a curl command'}
          </div>
          <button className="btn sm" onClick={onClose}>Cancel</button>
          <button className="btn-primary btn-sm-primary" onClick={openInRunner} disabled={!valid}>
            <Icon.bolt /> Open in runner
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Simple body editor ────────────────────────────────────────────────────────
function BodyEditor({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const [copied, setCopied] = useState(false);
  const text = value || '';
  const lines = (text.match(/\n/g) ?? []).length + 1;
  const gutter = Array.from({ length: lines }, (_, i) => i + 1).join('\n');
  const overlay = useMemo(() => highlight(text + (text.endsWith('\n') ? '' : '\n')), [text]);

  function copy() {
    navigator.clipboard?.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    }).catch(() => {});
  }

  return (
    <div className="json-editor">
      <div className="je-toolbar">
        <span>JSON</span>
        <span style={{ color: 'var(--fg-4)', textTransform: 'none', letterSpacing: 0, fontSize: 11 }}>
          {lines} {lines === 1 ? 'line' : 'lines'} · {new Blob([text]).size} B
        </span>
        <div className="right">
          <button className="btn" onClick={copy}>{copied ? 'Copied' : 'Copy'}</button>
        </div>
      </div>
      <div className="je-body">
        <div className="je-gutter">{gutter}</div>
        <pre className="je-overlay" aria-hidden dangerouslySetInnerHTML={{ __html: overlay }} />
        <textarea
          className="je-input"
          value={text}
          onChange={e => onChange(e.target.value)}
          spellCheck={false}
          placeholder={'{\n  "key": "value"\n}'}
          style={{ minHeight: 150 }}
        />
      </div>
    </div>
  );
}
