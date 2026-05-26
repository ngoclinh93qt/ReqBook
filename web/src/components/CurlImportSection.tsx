import { useState } from 'react';
import { api } from '../api';

interface Props {
  onImported: (relPath: string) => void;
}

export function CurlImportSection({ onImported }: Props) {
  const [open, setOpen] = useState(false);
  const [text, setText] = useState('');
  const [loading, setLoading] = useState(false);
  const [msg, setMsg] = useState<{ text: string; ok: boolean; relPath?: string } | null>(null);

  async function handleImport() {
    if (!text.trim()) { setMsg({ text: 'Paste a curl command first.', ok: false }); return; }
    setLoading(true);
    setMsg(null);
    try {
      const result = await api.importCurl(text.trim());
      if (result.error) {
        setMsg({ text: result.error, ok: false });
      } else if (result.status === 'exists') {
        setMsg({ text: result.message ?? 'Spec already exists.', ok: false });
      } else {
        setMsg({ text: `Created: ${result.path}`, ok: true, relPath: result.rel_path });
        setText('');
        if (result.rel_path) onImported(result.rel_path);
      }
    } catch (e: unknown) {
      setMsg({ text: String(e), ok: false });
    } finally {
      setLoading(false);
    }
  }

  return (
    <div style={{ marginTop: '1.5rem', borderTop: '1px solid #eee', paddingTop: '1.75rem' }}>
      <button onClick={() => setOpen(o => !o)}
        style={{ background: 'none', border: '1px solid #e5e5e5', borderRadius: 6, padding: '.45rem .9rem', fontSize: '.8rem', color: '#555', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '.4rem' }}>
        <span style={{ fontSize: '.65rem', transition: 'transform .2s', transform: open ? 'rotate(90deg)' : 'none' }}>▶</span>
        Import from cURL
      </button>
      {open && (
        <div style={{ marginTop: '1rem' }}>
          <label style={{ fontSize: '.8rem', fontWeight: 600, color: '#555', display: 'block', marginBottom: '.4rem' }}>
            Paste a <code>curl</code> command (e.g. from Chrome DevTools → Copy as cURL)
          </label>
          <textarea value={text} onChange={e => setText(e.target.value)} spellCheck={false}
            placeholder={'curl \'https://api.example.com/users\' \\\n  -H \'accept: application/json\''}
            style={{ width: '100%', height: 160, fontFamily: 'monospace', fontSize: '.78rem', border: '1px solid #d4d4d4', borderRadius: 6, padding: '.6rem .75rem', resize: 'vertical', outline: 'none', boxSizing: 'border-box' }} />
          <div style={{ display: 'flex', gap: '.5rem', marginTop: '.6rem', alignItems: 'center' }}>
            <button onClick={handleImport} disabled={loading}
              style={{ background: loading ? '#a5a7f5' : '#6366f1', color: '#fff', border: 'none', borderRadius: 6, padding: '.45rem 1rem', fontSize: '.82rem', fontWeight: 600, cursor: loading ? 'not-allowed' : 'pointer' }}>
              {loading ? 'Importing…' : 'Import endpoint'}
            </button>
            {msg && (
              <span style={{ fontSize: '.82rem', color: msg.ok ? '#15803d' : '#dc2626' }}>
                {msg.ok && msg.relPath
                  ? <>{msg.text} — <a href={`/spec/${msg.relPath}`} style={{ color: 'inherit', fontWeight: 600 }}>view spec →</a></>
                  : msg.text}
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
