import { useState } from 'react';
import { api } from '../api';

interface Props {
  relPath: string;
  rawSource: string;
  onSaved: () => void;
  onCancel: () => void;
}

export function EditPanel({ relPath, rawSource, onSaved, onCancel }: Props) {
  const [content, setContent] = useState(rawSource);
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState<{ text: string; ok: boolean } | null>(null);

  async function handleSave() {
    setSaving(true);
    setMsg(null);
    try {
      await api.saveSpec(relPath, content);
      setMsg({ text: 'Saved!', ok: true });
      setTimeout(onSaved, 600);
    } catch (e: unknown) {
      setMsg({ text: String(e), ok: false });
    } finally {
      setSaving(false);
    }
  }

  return (
    <div style={{ marginBottom: '1.75rem', background: '#fff', border: '1px solid #e8e8e8', borderRadius: 8, overflow: 'hidden' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: '.6rem', padding: '.65rem 1rem', borderBottom: '1px solid #e8e8e8', background: '#f9f9f9' }}>
        <span style={{ flex: 1, fontSize: '.8rem', fontWeight: 600, color: '#555' }}>Editing {relPath}</span>
        <button onClick={handleSave} disabled={saving}
          style={{ background: saving ? '#86efac' : '#15803d', color: '#fff', border: 'none', borderRadius: 5, padding: '.35rem .85rem', fontSize: '.8rem', fontWeight: 600, cursor: saving ? 'not-allowed' : 'pointer' }}>
          {saving ? 'Saving…' : 'Save'}
        </button>
        <button onClick={onCancel}
          style={{ background: 'none', border: '1px solid #d4d4d4', borderRadius: 5, padding: '.35rem .85rem', fontSize: '.8rem', color: '#555', cursor: 'pointer' }}>
          Cancel
        </button>
        {msg && <span style={{ fontSize: '.78rem', color: msg.ok ? '#15803d' : '#dc2626' }}>{msg.text}</span>}
      </div>
      <textarea
        value={content}
        onChange={e => setContent(e.target.value)}
        spellCheck={false}
        style={{ width: '100%', minHeight: 420, fontFamily: 'monospace', fontSize: '.82rem', lineHeight: 1.6, border: 'none', padding: '1rem 1.25rem', resize: 'vertical', outline: 'none', background: '#fff', color: '#1a1a1a', boxSizing: 'border-box' }}
      />
    </div>
  );
}
