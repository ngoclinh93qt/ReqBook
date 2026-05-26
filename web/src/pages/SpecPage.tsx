import { useEffect, useState, useCallback, useRef } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { api } from '../api';
import type { ExecResult, SpecData, VarsData } from '../types';
import { MethodBadge } from '../components/MethodBadge';
import { ParamsPanel } from '../components/ParamsPanel';
import { ResultBox } from '../components/ResultBox';
import { EditPanel } from '../components/EditPanel';
import { useBrowserVars } from '../hooks/useBrowserVars';

export function SpecPage() {
  const { '*': relPath = '' } = useParams();
  const navigate = useNavigate();
  const { vars: browserVars, save: saveBrowserVars } = useBrowserVars();
  const [spec, setSpec] = useState<SpecData | null>(null);
  const [envVars, setEnvVars] = useState<VarsData | null>(null);
  const [error, setError] = useState('');
  const [editing, setEditing] = useState(false);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<ExecResult | null>(null);
  const paramOverridesRef = useRef<Record<string, string>>({});

  const loadSpec = useCallback(async () => {
    try {
      const [s, v] = await Promise.all([
        api.getSpec(relPath),
        api.getVariables().catch(() => null),
      ]);
      setSpec(s);
      setEnvVars(v);
    } catch (e: unknown) {
      setError(String(e));
    }
  }, [relPath]);

  useEffect(() => { loadSpec(); }, [loadSpec]);

  async function handleRun() {
    if (!spec) return;
    setRunning(true);
    setResult(null);
    try {
      const vars = { ...(envVars?.vars ?? {}), ...browserVars, ...paramOverridesRef.current };
      const r = await api.execSpec(relPath, vars);
      setResult(r);
    } catch (e: unknown) {
      setResult({ duration_ms: 0, diff: { passed: false }, error: String(e) });
    } finally {
      setRunning(false);
    }
  }

  function handleCaptureToken(token: string) {
    const next = { ...browserVars, token };
    saveBrowserVars(next);
  }

  if (error) return (
    <div style={{ maxWidth: 860, margin: '0 auto', padding: '2rem 1.5rem', fontFamily: '-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif' }}>
      <nav style={{ marginBottom: '1.5rem' }}><button onClick={() => navigate('/')} style={{ background: 'none', border: 'none', color: '#666', cursor: 'pointer', fontSize: '.875rem' }}>← All endpoints</button></nav>
      <div style={{ color: '#dc2626' }}>{error}</div>
    </div>
  );

  if (!spec) return <div style={{ padding: '2rem', color: '#888', fontFamily: 'sans-serif' }}>Loading…</div>;

  return (
    <div style={{ maxWidth: 860, margin: '0 auto', padding: '2rem 1.5rem', fontFamily: '-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif', color: '#1a1a1a', lineHeight: 1.5 }}>
      <nav style={{ marginBottom: '1.5rem' }}>
        <button onClick={() => navigate('/')} style={{ background: 'none', border: 'none', color: '#666', cursor: 'pointer', fontSize: '.875rem' }}>← All endpoints</button>
      </nav>

      <header style={{ borderBottom: '1px solid #e5e5e5', marginBottom: '2rem', paddingBottom: '1.25rem', display: 'flex', alignItems: 'flex-start', gap: '1rem' }}>
        <MethodBadge method={spec.method} />
        <div style={{ flex: 1 }}>
          <h1 style={{ fontSize: '1.4rem', fontWeight: 700 }}>{spec.title}</h1>
          <p style={{ fontFamily: 'monospace', color: '#555', fontSize: '.9rem', marginTop: '.25rem' }}>{spec.path}</p>
          {spec.description && <p style={{ color: '#666', fontSize: '.9rem', marginTop: '.4rem' }}>{spec.description}</p>}
        </div>
        <button onClick={() => setEditing(e => !e)}
          style={{ background: 'none', border: '1px solid #e5e5e5', borderRadius: 6, padding: '.3rem .7rem', fontSize: '.8rem', color: '#555', cursor: 'pointer', flexShrink: 0, marginTop: '.1rem' }}>
          ✏️ Edit
        </button>
      </header>

      {editing && (
        <EditPanel relPath={relPath} rawSource={spec.raw_source}
          onSaved={() => { setEditing(false); loadSpec(); }}
          onCancel={() => setEditing(false)} />
      )}

      <Section title="Request"><pre style={preStyle}>{spec.request}</pre></Section>
      <Section title="Expected response"><pre style={preStyle}>{spec.expected_response}</pre></Section>
      {spec.tests && <Section title="Tests"><pre style={preStyle}>{spec.tests}</pre></Section>}

      <Section title="Run">
        <ParamsPanel
          requestText={spec.request}
          browserVars={browserVars}
          envVars={envVars}
          onChange={overrides => { paramOverridesRef.current = overrides; }}
        />
        <div style={{ display: 'flex', alignItems: 'center', gap: '1rem', flexWrap: 'wrap' }}>
          <button onClick={handleRun} disabled={running}
            style={{ background: running ? '#93c5fd' : '#1d4ed8', color: '#fff', border: 'none', padding: '.5rem 1.1rem', borderRadius: 6, fontSize: '.875rem', fontWeight: 600, cursor: running ? 'not-allowed' : 'pointer' }}>
            {running ? '…' : '▶ Run'}
          </button>
          <span style={{ color: '#888', fontSize: '.8rem' }}>env: {spec.env}</span>
          <span style={{
            fontSize: '.75rem', padding: '.25rem .55rem', borderRadius: 10, fontWeight: 600,
            background: browserVars.token ? '#dcfce7' : '#f3f4f6',
            color: browserVars.token ? '#15803d' : '#6b7280',
          }}>
            {browserVars.token ? '🔒 Token set' : '🔓 No token'}
          </span>
        </div>
        {result && <ResultBox result={result} onCaptureToken={handleCaptureToken} />}
      </Section>

      <footer style={{ marginTop: '3rem', paddingTop: '1rem', borderTop: '1px solid #eee', fontSize: '.75rem', color: '#bbb' }}>
        Trellis {spec.version}
      </footer>
    </div>
  );
}

const preStyle: React.CSSProperties = {
  background: '#fff', border: '1px solid #e8e8e8', borderRadius: 8,
  padding: '1rem 1.25rem', overflowX: 'auto',
  fontFamily: '"SF Mono","Fira Code",Consolas,monospace', fontSize: '.82rem', lineHeight: 1.6, color: '#333', margin: 0,
};

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section style={{ marginBottom: '1.75rem' }}>
      <h2 style={{ fontSize: '.8rem', fontWeight: 600, textTransform: 'uppercase', letterSpacing: '.07em', color: '#888', marginBottom: '.6rem' }}>{title}</h2>
      {children}
    </section>
  );
}
