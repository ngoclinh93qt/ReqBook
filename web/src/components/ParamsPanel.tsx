import { useEffect, useState } from 'react';
import type { VarsData } from '../types';

function extractVars(text: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const m of text.matchAll(/\{\{([a-zA-Z0-9_]+)\}\}/g)) {
    if (!seen.has(m[1])) { seen.add(m[1]); out.push(m[1]); }
  }
  return out;
}

interface Props {
  requestText: string;
  browserVars: Record<string, string>;
  envVars: VarsData | null;
  onChange: (overrides: Record<string, string>) => void;
}

export function ParamsPanel({ requestText, browserVars, envVars, onChange }: Props) {
  const vars = extractVars(requestText);
  const [values, setValues] = useState<Record<string, string>>(() => {
    const init: Record<string, string> = {};
    for (const v of vars) {
      init[v] = browserVars[v] ?? envVars?.vars[v] ?? '';
    }
    return init;
  });

  // re-sync when browser/env vars change
  useEffect(() => {
    setValues(prev => {
      const next = { ...prev };
      for (const v of vars) {
        if (next[v] === undefined) next[v] = browserVars[v] ?? envVars?.vars[v] ?? '';
      }
      return next;
    });
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [browserVars, envVars]);

  useEffect(() => { onChange(values); }, [values, onChange]);

  if (vars.length === 0) return null;

  function source(name: string) {
    if (browserVars[name] !== undefined) return { label: 'browser', color: '#15803d', bg: '#dcfce7' };
    if (envVars?.vars[name] !== undefined) return { label: 'env', color: '#1d4ed8', bg: '#dbeafe' };
    return { label: 'unset', color: '#dc2626', bg: '#fee2e2' };
  }

  return (
    <div style={{ background: '#fff', border: '1px solid #e8e8e8', borderRadius: 8, padding: '.85rem 1.1rem', marginBottom: '.9rem' }}>
      <div style={{ fontSize: '.75rem', fontWeight: 600, textTransform: 'uppercase', letterSpacing: '.07em', color: '#888', marginBottom: '.7rem' }}>
        Parameters
      </div>
      {vars.map(name => {
        const src = source(name);
        const defaultVal = browserVars[name] ?? envVars?.vars[name] ?? '';
        const isOverriding = values[name] !== defaultVal && values[name] !== '';
        return (
          <div key={name} style={{ display: 'flex', alignItems: 'center', gap: '.6rem', marginBottom: '.45rem' }}>
            <span style={{ fontFamily: 'monospace', fontSize: '.78rem', color: '#444', minWidth: 130, maxWidth: 160, flexShrink: 0, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
              title={`{{${name}}}`}>
              {`{{${name}}}`}
            </span>
            <input
              type="text"
              value={values[name] ?? ''}
              placeholder={`value for ${name}`}
              onChange={e => setValues(prev => ({ ...prev, [name]: e.target.value }))}
              style={{
                flex: 1, border: `1px solid ${isOverriding ? '#f59e0b' : '#d4d4d4'}`,
                background: isOverriding ? '#fffbeb' : '#fff',
                borderRadius: 5, padding: '.3rem .6rem',
                fontFamily: 'monospace', fontSize: '.82rem', outline: 'none', minWidth: 0,
              }}
            />
            <span style={{ fontSize: '.7rem', padding: '.1rem .35rem', borderRadius: 3, flexShrink: 0, background: src.bg, color: src.color }}>
              {src.label}
            </span>
          </div>
        );
      })}
      <p style={{ fontSize: '.74rem', color: '#aaa', marginTop: '.55rem', paddingTop: '.55rem', borderTop: '1px solid #f3f3f3' }}>
        Values set here apply to this run only. Save to browser variables to persist.
      </p>
    </div>
  );
}
