import { useMemo, useRef, useState } from 'react';

export const Icon = {
  play: () => <svg width="11" height="11" viewBox="0 0 11 11" fill="currentColor" aria-hidden><path d="M2.5 1.5 L9 5.5 L2.5 9.5 Z" /></svg>,
  chev: ({ open }: { open: boolean }) => (
    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" className={open ? 'rot-90' : ''}><polyline points="3.5,2 7,5 3.5,8" /></svg>
  ),
  arr: () => <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><line x1="2.5" y1="6" x2="9.5" y2="6" /><polyline points="6.5,3 9.5,6 6.5,9" /></svg>,
  sun: () => <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5"><circle cx="8" cy="8" r="3" /><line x1="8" y1="1.5" x2="8" y2="3" strokeLinecap="round" /><line x1="8" y1="13" x2="8" y2="14.5" strokeLinecap="round" /><line x1="1.5" y1="8" x2="3" y2="8" strokeLinecap="round" /><line x1="13" y1="8" x2="14.5" y2="8" strokeLinecap="round" /><line x1="3.3" y1="3.3" x2="4.4" y2="4.4" strokeLinecap="round" /><line x1="11.6" y1="11.6" x2="12.7" y2="12.7" strokeLinecap="round" /><line x1="12.7" y1="3.3" x2="11.6" y2="4.4" strokeLinecap="round" /><line x1="4.4" y1="11.6" x2="3.3" y2="12.7" strokeLinecap="round" /></svg>,
  moon: () => <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round"><path d="M13.5 9.5A6 6 0 1 1 6.5 2.5a5 5 0 0 0 7 7Z" /></svg>,
  search: () => <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5"><circle cx="7" cy="7" r="4.5" /><line x1="10.5" y1="10.5" x2="14" y2="14" strokeLinecap="round" /></svg>,
  copy: () => <svg width="13" height="13" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.3"><rect x="4" y="4" width="7.5" height="7.5" rx="1.5" /><path d="M2.5 9V3.5A1.5 1.5 0 0 1 4 2H9" /></svg>,
  edit: () => <svg width="13" height="13" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round"><path d="M10 2.5l1.5 1.5L5 10.5H3.5V9z" /></svg>,
  plus: () => <svg width="11" height="11" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round"><line x1="6" y1="2.5" x2="6" y2="9.5" /><line x1="2.5" y1="6" x2="9.5" y2="6" /></svg>,
  x: () => <svg width="11" height="11" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"><line x1="3" y1="3" x2="9" y2="9" /><line x1="9" y1="3" x2="3" y2="9" /></svg>,
  vars: () => <svg width="13" height="13" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.3"><path d="M5 2.5C3.5 2.5 3 4 3 5.5v3C3 10 3.5 11.5 5 11.5M9 2.5c1.5 0 2 1.5 2 3v3c0 1.5-.5 3-2 3" strokeLinecap="round" /><path d="M5.5 7H8.5" strokeLinecap="round" /></svg>,
  filter: () => <svg width="12" height="12" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round"><path d="M2 3h10M4 7h6M6 11h2" /></svg>,
  check: () => <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><polyline points="2.5,6.5 5,9 9.5,3.5" /></svg>,
  cross: () => <svg width="11" height="11" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round"><line x1="3" y1="3" x2="9" y2="9" /><line x1="9" y1="3" x2="3" y2="9" /></svg>,
};

export function MethodBadge({ method }: { method: string }) {
  return <span className={`method ${method.toLowerCase()}`}>{method}</span>;
}

export function PathStr({ path }: { path: string }) {
  const parts = [];
  const re = /(:[a-zA-Z0-9_*]+|\{\{[a-zA-Z0-9_]+\}\})/g;
  let last = 0;
  let key = 0;
  for (const match of path.matchAll(re)) {
    if (match.index > last) parts.push(<span key={key++}>{path.slice(last, match.index)}</span>);
    parts.push(<span key={key++} className="param">{match[0]}</span>);
    last = match.index + match[0].length;
  }
  if (last < path.length) parts.push(<span key={key++}>{path.slice(last)}</span>);
  return <>{parts}</>;
}

export function highlight(text: string) {
  const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  const hVar = (s: string) => s.replace(/\{\{([a-zA-Z0-9_]+)\}\}/g, '<span class="v">{{$1}}</span>');
  const hParam = (s: string) => s.replace(/(:[a-zA-Z0-9_*]+)(?=[/\s?\\.]|$)/g, '<span class="v">$1</span>');
  return text.split('\n').map(line => {
    if (/^(GET|POST|PUT|PATCH|DELETE|HEAD|OPTIONS)\s/.test(line)) {
      const m = line.match(/^(\S+)\s+(.*)$/);
      return m ? `<span class="k">${m[1]}</span> ${hParam(hVar(esc(m[2])))}` : esc(line);
    }
    if (/^HTTP\/1\.1/.test(line)) {
      const m = line.match(/^(HTTP\/1\.1)\s+(\d+)\s*(.*)$/);
      return m ? `<span class="p">${m[1]}</span> <span class="${Number(m[2]) < 400 ? 'k' : 'b'}">${m[2]} ${esc(m[3])}</span>` : esc(line);
    }
    if (/^[A-Za-z][A-Za-z0-9-]*:\s/.test(line)) {
      const idx = line.indexOf(':');
      return `<span class="n">${esc(line.slice(0, idx))}</span><span class="p">:</span>${hVar(esc(line.slice(idx + 1)))}`;
    }
    let out = esc(line);
    out = out.replace(/"([^"\\]*(?:\\.[^"\\]*)*)"/g, '<span class="s">"$1"</span>');
    out = out.replace(/(:\s*)(-?\d+(?:\.\d+)?|true|false|null)(\b)/g, '$1<span class="b">$2</span>$3');
    out = out.replace(/<span class="s">"([^"]+)"<\/span>(\s*:)/g, '<span class="n">"$1"</span>$2');
    return hVar(out);
  }).join('\n');
}

export function parseRequest(reqText: string) {
  const [head, body = ''] = reqText.split(/\n\n/);
  const lines = head.split('\n');
  const requestLine = lines[0] ?? '';
  const headers = lines.slice(1).map((line, index) => {
    const idx = line.indexOf(':');
    return idx > 0 ? { id: `h-${index}`, name: line.slice(0, idx).trim(), value: line.slice(idx + 1).trim(), enabled: true } : null;
  }).filter(Boolean) as { id: string; name: string; value: string; enabled: boolean }[];
  return { requestLine, headers, body: body.replace(/\n+$/, '') };
}

export function uniqueMatches(text: string, pattern: RegExp) {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const match of text.matchAll(pattern)) {
    if (!seen.has(match[1])) {
      seen.add(match[1]);
      out.push(match[1]);
    }
  }
  return out;
}

export function JsonEditor({ value, onChange, placeholder, minHeight = 120, language = 'json' }: {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  minHeight?: number;
  language?: string;
}) {
  const [copied, setCopied] = useState(false);
  const validation = useMemo(() => {
    if (language !== 'json' || !value.trim()) return null;
    try {
      JSON.parse(value);
      return { ok: true, msg: '' };
    } catch (e) {
      return { ok: false, msg: String(e) };
    }
  }, [language, value]);
  const lines = (value.match(/\n/g) ?? []).length + 1;
  const gutter = Array.from({ length: lines }, (_, index) => index + 1).join('\n');
  function format() {
    try { onChange(JSON.stringify(JSON.parse(value), null, 2)); } catch {}
  }
  function copy() {
    navigator.clipboard?.writeText(value).then(() => {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1200);
    }).catch(() => {});
  }
  return (
    <div className="json-editor with-toolbar">
      <div className="je-toolbar">
        <span>{language.toUpperCase()}</span>
        <span className="muted">{lines} {lines === 1 ? 'line' : 'lines'}</span>
        {validation && <span className={validation.ok ? 'ok-text' : 'fail-text'}>{validation.ok ? 'valid' : 'invalid'}</span>}
        <div className="right">
          {language === 'json' && <button className="btn" onClick={format} disabled={validation?.ok === false}>Format</button>}
          <button className="btn" onClick={copy}>{copied ? 'Copied' : 'Copy'}</button>
        </div>
      </div>
      <div className="je-body">
        <div className="je-gutter">{gutter}</div>
        <pre className="je-overlay" aria-hidden="true" dangerouslySetInnerHTML={{ __html: highlight(value + (value.endsWith('\n') ? '' : '\n')) }} />
        <textarea className="je-input" value={value} onChange={e => onChange(e.target.value)} spellCheck={false} placeholder={placeholder} style={{ minHeight }} />
      </div>
    </div>
  );
}

export function useOutsideClose(onClose: () => void) {
  const ref = useRef<HTMLDivElement | null>(null);
  return { ref, onClose };
}
