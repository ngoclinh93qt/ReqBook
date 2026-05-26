import { useState } from 'react';

interface Props {
  vars: Record<string, string>;
  onSave: (vars: Record<string, string>) => void;
}

export function BrowserVarsSection({ vars, onSave }: Props) {
  const [open, setOpen] = useState(false);
  const [rows, setRows] = useState<[string, string][]>(() => Object.entries(vars));
  const [saved, setSaved] = useState(false);

  function handleSave() {
    onSave(Object.fromEntries(rows.filter(([k]) => k.trim())));
    setSaved(true);
    setTimeout(() => setSaved(false), 1500);
  }

  return (
    <div style={{ marginTop: '1.5rem', borderTop: '1px solid #eee', paddingTop: '1.75rem' }}>
      <button onClick={() => setOpen(o => !o)}
        style={{ background: 'none', border: '1px solid #e5e5e5', borderRadius: 6, padding: '.45rem .9rem', fontSize: '.8rem', color: '#555', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '.4rem' }}>
        <span style={{ fontSize: '.65rem', transition: 'transform .2s', transform: open ? 'rotate(90deg)' : 'none' }}>▶</span>
        🔑 Browser variables {vars.token ? '· 🔒 token set' : ''}
      </button>
      {open && (
        <div style={{ marginTop: '1rem' }}>
          <p style={{ fontSize: '.8rem', color: '#888', marginBottom: '.75rem' }}>
            Stored in your browser only — never written to disk. Use for tokens and API keys.<br />
            Set <code>token</code> here to fill <code>{'{{'+'token'+'}}'}</code> in requests.
          </p>
          <div>
            {rows.map(([k, v], i) => (
              <div key={i} style={{ display: 'flex', gap: '.5rem', marginBottom: '.4rem' }}>
                <input value={k} placeholder="key (e.g. token)"
                  onChange={e => setRows(r => r.map((row, j) => j === i ? [e.target.value, row[1]] : row))}
                  style={{ flex: 1, border: '1px solid #d4d4d4', borderRadius: 5, padding: '.3rem .6rem', fontFamily: 'monospace', fontSize: '.82rem', outline: 'none' }} />
                <input value={v} placeholder="value" type={k === 'token' || k.toLowerCase().includes('key') ? 'password' : 'text'}
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
              + Add
            </button>
            <button onClick={handleSave}
              style={{ background: '#6366f1', color: '#fff', border: 'none', borderRadius: 5, padding: '.35rem .85rem', fontSize: '.8rem', fontWeight: 600, cursor: 'pointer' }}>
              Save to browser
            </button>
            <button onClick={() => { onSave({}); setRows([]); }}
              style={{ background: 'none', border: '1px solid #e5e5e5', borderRadius: 5, padding: '.35rem .75rem', fontSize: '.8rem', color: '#888', cursor: 'pointer' }}>
              Clear all
            </button>
            {saved && <span style={{ fontSize: '.78rem', color: '#15803d' }}>✓ Saved</span>}
          </div>
        </div>
      )}
    </div>
  );
}
