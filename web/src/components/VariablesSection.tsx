import { useState, useCallback } from 'react';
import { api } from '../api';
import type { VarsData } from '../types';

interface Props {
  onVarsLoaded?: (data: VarsData) => void;
}

export function VariablesSection({ onVarsLoaded }: Props) {
  const [open, setOpen] = useState(false);
  const [data, setData] = useState<VarsData | null>(null);
  const [rows, setRows] = useState<[string, string][]>([]);
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState('');

  async function handleOpen() {
    if (!open && !data) {
      const d = await api.getVariables().catch(() => null);
      if (d) {
        setData(d);
        setRows(Object.entries(d.vars));
        onVarsLoaded?.(d);
      }
    }
    setOpen(o => !o);
  }

  const handleSave = useCallback(async () => {
    if (!data) return;
    setSaving(true);
    setMsg('');
    try {
      const vars = Object.fromEntries(rows.filter(([k]) => k.trim()));
      await api.saveVariables(data.env, vars);
      setMsg('✓ Saved');
      const d = { ...data, vars };
      setData(d);
      onVarsLoaded?.(d);
    } catch (e: unknown) {
      setMsg(String(e));
    } finally {
      setSaving(false);
    }
  }, [data, rows, onVarsLoaded]);

  return (
    <div style={{ marginTop: '1.5rem', borderTop: '1px solid #eee', paddingTop: '1.75rem' }}>
      <button onClick={handleOpen}
        style={{ background: 'none', border: '1px solid #e5e5e5', borderRadius: 6, padding: '.45rem .9rem', fontSize: '.8rem', color: '#555', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '.4rem' }}>
        <span style={{ fontSize: '.65rem', transition: 'transform .2s', transform: open ? 'rotate(90deg)' : 'none' }}>▶</span>
        ⚙️ Environment variables {data ? `(${data.env})` : ''}
      </button>
      {open && (
        <div style={{ marginTop: '1rem' }}>
          <p style={{ fontSize: '.8rem', color: '#888', marginBottom: '.75rem' }}>
            Non-sensitive variables from <code>api-docs/_shared/env.md</code>. Saved to disk.
          </p>
          <div>
            {rows.map(([k, v], i) => (
              <div key={i} style={{ display: 'flex', gap: '.5rem', marginBottom: '.4rem' }}>
                <input value={k} placeholder="key"
                  onChange={e => setRows(r => r.map((row, j) => j === i ? [e.target.value, row[1]] : row))}
                  style={{ flex: 1, border: '1px solid #d4d4d4', borderRadius: 5, padding: '.3rem .6rem', fontFamily: 'monospace', fontSize: '.82rem', outline: 'none' }} />
                <input value={v} placeholder="value"
                  onChange={e => setRows(r => r.map((row, j) => j === i ? [row[0], e.target.value] : row))}
                  style={{ flex: 2, border: '1px solid #d4d4d4', borderRadius: 5, padding: '.3rem .6rem', fontFamily: 'monospace', fontSize: '.82rem', outline: 'none' }} />
                <button onClick={() => setRows(r => r.filter((_, j) => j !== i))}
                  style={{ background: 'none', border: '1px solid #e5e5e5', borderRadius: 5, padding: '.3rem .6rem', cursor: 'pointer', color: '#999' }}>✕</button>
              </div>
            ))}
          </div>
          <div style={{ display: 'flex', gap: '.5rem', marginTop: '.6rem', alignItems: 'center' }}>
            <button onClick={() => setRows(r => [...r, ['', '']])}
              style={{ background: 'none', border: '1px dashed #d4d4d4', borderRadius: 5, padding: '.3rem .75rem', fontSize: '.8rem', color: '#888', cursor: 'pointer' }}>
              + Add variable
            </button>
            <button onClick={handleSave} disabled={saving}
              style={{ background: '#6366f1', color: '#fff', border: 'none', borderRadius: 5, padding: '.35rem .85rem', fontSize: '.8rem', fontWeight: 600, cursor: 'pointer' }}>
              {saving ? 'Saving…' : 'Save'}
            </button>
            {msg && <span style={{ fontSize: '.78rem', color: msg.startsWith('✓') ? '#15803d' : '#dc2626' }}>{msg}</span>}
          </div>
        </div>
      )}
    </div>
  );
}
