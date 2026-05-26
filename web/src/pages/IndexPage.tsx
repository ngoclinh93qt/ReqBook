import { useEffect, useState, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { api } from '../api';
import type { IndexData, VarsData } from '../types';
import { MethodBadge } from '../components/MethodBadge';
import { VariablesSection } from '../components/VariablesSection';
import { BrowserVarsSection } from '../components/BrowserVarsSection';
import { CurlImportSection } from '../components/CurlImportSection';
import { useBrowserVars } from '../hooks/useBrowserVars';

export function IndexPage() {
  const [data, setData] = useState<IndexData | null>(null);
  const [error, setError] = useState('');
  const navigate = useNavigate();
  const { vars: browserVars, save: saveBrowserVars } = useBrowserVars();
  const [, setEnvVars] = useState<VarsData | null>(null);

  const fetchIndex = useCallback(async () => {
    try {
      setData(await api.getIndex());
    } catch (e: unknown) {
      setError(String(e));
    }
  }, []);

  useEffect(() => { fetchIndex(); }, [fetchIndex]);

  if (error) return <div style={{ color: '#dc2626', padding: '2rem' }}>{error}</div>;
  if (!data) return <div style={{ padding: '2rem', color: '#888' }}>Loading…</div>;

  return (
    <div style={{ maxWidth: 860, margin: '0 auto', padding: '2rem 1.5rem', fontFamily: '-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif', color: '#1a1a1a', lineHeight: 1.5 }}>
      <header style={{ borderBottom: '1px solid #e5e5e5', marginBottom: '2rem', paddingBottom: '1.25rem' }}>
        <h1 style={{ fontSize: '1.5rem', fontWeight: 700 }}>{data.project_name}</h1>
        <p style={{ color: '#666', fontSize: '.875rem', marginTop: '.25rem' }}>
          {data.spec_count} endpoint{data.spec_count !== 1 ? 's' : ''} · Trellis web preview
        </p>
      </header>

      {data.groups.length === 0 ? (
        <p style={{ color: '#aaa', textAlign: 'center', padding: '3rem 0', fontSize: '.9rem' }}>
          No endpoints found. Run <code>trellis init</code> to scaffold a project.
        </p>
      ) : (
        data.groups.map(group => (
          <div key={group.resource} style={{ marginBottom: '2rem' }}>
            <h2 style={{ fontSize: '.75rem', fontWeight: 600, textTransform: 'uppercase', letterSpacing: '.08em', color: '#888', marginBottom: '.65rem' }}>
              {group.resource}
            </h2>
            {group.specs.map(spec => (
              <div key={spec.rel_path} onClick={() => navigate(`/spec/${spec.rel_path}`)}
                style={{ display: 'flex', alignItems: 'center', gap: '.75rem', background: '#fff', border: '1px solid #e8e8e8', borderRadius: 8, padding: '.65rem 1rem', marginBottom: '.35rem', cursor: 'pointer', transition: 'border-color .15s, box-shadow .15s' }}
                onMouseEnter={e => { (e.currentTarget as HTMLDivElement).style.borderColor = '#bbb'; (e.currentTarget as HTMLDivElement).style.boxShadow = '0 1px 4px rgba(0,0,0,.06)'; }}
                onMouseLeave={e => { (e.currentTarget as HTMLDivElement).style.borderColor = '#e8e8e8'; (e.currentTarget as HTMLDivElement).style.boxShadow = 'none'; }}>
                <MethodBadge method={spec.method} />
                <span style={{ fontFamily: 'monospace', fontSize: '.85rem', color: '#333' }}>{spec.path}</span>
                <span style={{ marginLeft: 'auto', color: '#aaa', fontSize: '.8rem', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis', maxWidth: 200 }}>
                  {spec.title}
                </span>
              </div>
            ))}
          </div>
        ))
      )}

      <CurlImportSection onImported={() => { fetchIndex(); }} />
      <VariablesSection onVarsLoaded={setEnvVars} />
      <BrowserVarsSection vars={browserVars} onSave={saveBrowserVars} />

      <footer style={{ marginTop: '3rem', paddingTop: '1rem', borderTop: '1px solid #eee', fontSize: '.75rem', color: '#bbb' }}>
        Trellis {data.version} · <a href="https://trellis.dev" style={{ color: '#bbb' }}>trellis.dev</a>
      </footer>
    </div>
  );
}
