import { useMemo, useRef, useState, type KeyboardEvent } from 'react';

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
  folder: () => <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round"><path d="M2 4.5A1.5 1.5 0 0 1 3.5 3h3l1.5 2H13a1.5 1.5 0 0 1 1.5 1.5v6A1.5 1.5 0 0 1 13 14H3a1.5 1.5 0 0 1-1.5-1.5V4.5Z" /></svg>,
  clock: () => <svg width="13" height="13" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round"><circle cx="7" cy="7" r="5.5" /><polyline points="7,4 7,7 9.5,8.5" /></svg>,
  grid: () => <svg width="13" height="13" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.3"><rect x="2" y="2" width="4" height="4" rx="1" /><rect x="8" y="2" width="4" height="4" rx="1" /><rect x="2" y="8" width="4" height="4" rx="1" /><rect x="8" y="8" width="4" height="4" rx="1" /></svg>,
  bolt: () => <svg width="11" height="11" viewBox="0 0 12 12" fill="currentColor"><path d="M7 1 L2 7 H5.5 L4.5 11 L10 5 H6.5 Z" /></svg>,
  bolt2: () => <svg width="12" height="12" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinejoin="round"><path d="M7.5 1.5 3 8h3.5L6 12.5 11 6H7z" /></svg>,
  scan: () => <svg width="13" height="13" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round"><path d="M2 5V3.5A1.5 1.5 0 0 1 3.5 2H5" /><path d="M9 2h1.5A1.5 1.5 0 0 1 12 3.5V5" /><path d="M12 9v1.5a1.5 1.5 0 0 1-1.5 1.5H9" /><path d="M5 12H3.5A1.5 1.5 0 0 1 2 10.5V9" /><line x1="2" y1="7" x2="12" y2="7" /></svg>,
  sync: () => <svg width="13" height="13" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.35" strokeLinecap="round" strokeLinejoin="round"><path d="M12 7a5 5 0 0 1-8.5 3.5" /><path d="M2 7a5 5 0 0 1 8.5-3.5" /><polyline points="11.5,1 11.5,3.5 9,3.5" /><polyline points="2.5,13 2.5,10.5 5,10.5" /></svg>,
  branch: () => <svg width="13" height="13" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.35" strokeLinecap="round" strokeLinejoin="round"><circle cx="4" cy="3" r="1.5" /><circle cx="10" cy="4" r="1.5" /><circle cx="4" cy="11" r="1.5" /><path d="M4 4.5v5" /><path d="M4 7h2.5A3.5 3.5 0 0 0 10 5.5" /></svg>,
  dots: () => <svg width="13" height="13" viewBox="0 0 14 14" fill="currentColor" aria-hidden><circle cx="3" cy="7" r="1.3" /><circle cx="7" cy="7" r="1.3" /><circle cx="11" cy="7" r="1.3" /></svg>,
  flow: () => <svg width="13" height="13" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round"><circle cx="3.5" cy="3.5" r="1.8" /><circle cx="10.5" cy="10.5" r="1.8" /><path d="M5 4.5C8 5 9 6 9.5 9" /></svg>,
  trash: () => <svg width="13" height="13" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><path d="M2.5 4h9" /><path d="M5 4V2.5h4V4" /><path d="M3.5 4l.5 7.5h6l.5-7.5" /></svg>,
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

type HighlightOptions = {
  knownVariables?: ReadonlySet<string>;
};

export function highlight(text: string, options: HighlightOptions = {}) {
  const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  const hVar = (s: string) => s.replace(/\{\{([a-zA-Z0-9_]+)\}\}/g, (_, name) => {
    const className = options.knownVariables && !options.knownVariables.has(name) ? 'v var-missing' : 'v';
    return `<span class="${className}">{{${name}}}</span>`;
  });
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

const TVAR_RE = /\{\{([a-zA-Z0-9_]+)\}\}/g;
const VAR_CHAR_RE = /[a-zA-Z0-9_]/;

function variableTokenAtCursor(text: string, cursor: number) {
  const before = text.slice(0, cursor);
  const match = before.match(/\{\{([a-zA-Z0-9_]*)$/);
  if (!match) return null;

  let end = cursor;
  while (end < text.length && VAR_CHAR_RE.test(text[end])) end += 1;
  if (text.slice(end, end + 2) === '}}') end += 2;

  return {
    start: cursor - match[0].length,
    end,
    prefix: match[1] ?? '',
  };
}

function jsonValidateWithVars(text: string): boolean {
  try { JSON.parse(text.replace(TVAR_RE, '0')); return true; } catch { return false; }
}

function jsonFormatWithVars(text: string): string | null {
  let qi = 0, bi = 0;
  const qmap = new Map<string, string>();
  const bmap = new Map<string, string>();
  // Pass 1: replace "{{varName}}" (whole quoted string) with a string sentinel
  let result = text.replace(/"(\{\{[a-zA-Z0-9_]+\}\})"/g, (_, inner) => {
    const key = `"__TVQ${qi++}__"`;
    qmap.set(key, inner.slice(2, -2));
    return key;
  });
  // Pass 2: replace remaining bare {{varName}} with a string sentinel
  result = result.replace(/\{\{([a-zA-Z0-9_]+)\}\}/g, (_, name) => {
    const key = `"__TVB${bi++}__"`;
    bmap.set(key, name);
    return key;
  });
  try { result = JSON.stringify(JSON.parse(result), null, 2); } catch { return null; }
  for (const [key, name] of qmap) result = result.replace(key, `"{{${name}}}"`);
  for (const [key, name] of bmap) result = result.replace(key, `{{${name}}}`);
  return result;
}

export function JsonEditor({ value, onChange, placeholder, minHeight = 120, language = 'json', variableNames }: {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  minHeight?: number;
  language?: string;
  variableNames?: readonly string[];
}) {
  const [copied, setCopied] = useState(false);
  const [focused, setFocused] = useState(false);
  const [cursor, setCursor] = useState(0);
  const [activeSuggestion, setActiveSuggestion] = useState(0);
  const [dismissedTokenStart, setDismissedTokenStart] = useState<number | null>(null);
  const [scroll, setScroll] = useState({ left: 0, top: 0 });
  const inputRef = useRef<HTMLTextAreaElement | null>(null);
  const knownVariables = useMemo(() => new Set((variableNames ?? []).filter(Boolean)), [variableNames]);
  const sortedVariables = useMemo(() => Array.from(knownVariables).sort((a, b) => a.localeCompare(b)), [knownVariables]);
  const validation = useMemo(() => {
    if (language !== 'json' || !value.trim()) return null;
    return { ok: jsonValidateWithVars(value) };
  }, [language, value]);
  const overlay = useMemo(() => {
    const text = value + (value.endsWith('\n') ? '' : '\n');
    return highlight(text, variableNames ? { knownVariables } : {});
  }, [knownVariables, value, variableNames]);
  const activeToken = useMemo(() => variableTokenAtCursor(value, cursor), [cursor, value]);
  const suggestions = useMemo(() => {
    if (!activeToken || sortedVariables.length === 0) return [];
    const query = activeToken.prefix.toLowerCase();
    const starts = sortedVariables.filter(name => name.toLowerCase().startsWith(query));
    const contains = query
      ? sortedVariables.filter(name => !starts.includes(name) && name.toLowerCase().includes(query))
      : [];
    return [...starts, ...contains].slice(0, 8);
  }, [activeToken, sortedVariables]);
  const showSuggestions = focused && suggestions.length > 0 && activeToken?.start !== dismissedTokenStart;
  const selectedSuggestion = suggestions[Math.min(activeSuggestion, Math.max(0, suggestions.length - 1))];
  const suggestionPosition = useMemo(() => {
    const before = value.slice(0, cursor);
    const line = (before.match(/\n/g) ?? []).length;
    const lineStart = before.lastIndexOf('\n') + 1;
    const col = cursor - lineStart;
    const rawLeft = 52 + col * 7.6 - scroll.left;
    const rawTop = 12 + (line + 1) * 20 - scroll.top;
    const maxLeft = Math.max(52, (inputRef.current?.clientWidth ?? 320) - 270);
    const maxTop = Math.max(40, (inputRef.current?.clientHeight ?? minHeight) - 12);
    return {
      left: Math.max(48, Math.min(rawLeft, maxLeft)),
      top: Math.max(34, Math.min(rawTop, maxTop)),
    };
  }, [cursor, minHeight, scroll.left, scroll.top, value]);
  const lines = (value.match(/\n/g) ?? []).length + 1;
  const gutter = Array.from({ length: lines }, (_, index) => index + 1).join('\n');

  function syncCursor(target: HTMLTextAreaElement) {
    const nextCursor = target.selectionStart;
    if (nextCursor !== cursor) {
      const nextToken = variableTokenAtCursor(target.value, nextCursor);
      if (!nextToken || nextToken.start !== dismissedTokenStart) setDismissedTokenStart(null);
    }
    setCursor(nextCursor);
    setScroll({ left: target.scrollLeft, top: target.scrollTop });
  }

  function format() {
    const formatted = jsonFormatWithVars(value);
    if (formatted != null) onChange(formatted);
  }
  function copy() {
    navigator.clipboard?.writeText(value).then(() => {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1200);
    }).catch(() => {});
  }
  function applySuggestion(name: string) {
    const textarea = inputRef.current;
    const nextCursor = textarea?.selectionStart ?? cursor;
    const token = variableTokenAtCursor(value, nextCursor);
    if (!token) return;

    const inserted = `{{${name}}}`;
    const next = `${value.slice(0, token.start)}${inserted}${value.slice(token.end)}`;
    const selection = token.start + inserted.length;
    onChange(next);
    setCursor(selection);
    setActiveSuggestion(0);
    window.requestAnimationFrame(() => {
      inputRef.current?.focus();
      inputRef.current?.setSelectionRange(selection, selection);
    });
  }
  function handleKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (!showSuggestions) return;

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setActiveSuggestion(index => (index + 1) % suggestions.length);
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      setActiveSuggestion(index => (index - 1 + suggestions.length) % suggestions.length);
    } else if ((event.key === 'Enter' || event.key === 'Tab') && selectedSuggestion) {
      event.preventDefault();
      applySuggestion(selectedSuggestion);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      setDismissedTokenStart(activeToken?.start ?? null);
    }
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
        <pre className="je-overlay" aria-hidden="true" dangerouslySetInnerHTML={{ __html: overlay }} />
        <textarea
          ref={inputRef}
          className="je-input"
          value={value}
          onChange={event => {
            onChange(event.target.value);
            setDismissedTokenStart(null);
            syncCursor(event.target);
            setActiveSuggestion(0);
          }}
          onFocus={event => {
            setFocused(true);
            syncCursor(event.currentTarget);
          }}
          onBlur={() => setFocused(false)}
          onClick={event => syncCursor(event.currentTarget)}
          onKeyDown={handleKeyDown}
          onKeyUp={event => syncCursor(event.currentTarget)}
          onScroll={event => syncCursor(event.currentTarget)}
          onSelect={event => syncCursor(event.currentTarget)}
          spellCheck={false}
          placeholder={placeholder}
          style={{ minHeight }}
        />
        {showSuggestions && (
          <div className="je-suggestions" style={suggestionPosition}>
            {suggestions.map((name, index) => (
              <button
                key={name}
                className={`je-suggestion ${name === selectedSuggestion && index === Math.min(activeSuggestion, suggestions.length - 1) ? 'is-active' : ''}`}
                onMouseDown={event => {
                  event.preventDefault();
                  applySuggestion(name);
                }}
              >
                <span className="br">{'{{'}</span>{name}<span className="br">{'}}'}</span>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

export function useOutsideClose(onClose: () => void) {
  const ref = useRef<HTMLDivElement | null>(null);
  return { ref, onClose };
}
