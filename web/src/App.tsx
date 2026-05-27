import { useEffect, useState } from 'react';
import { BrowserRouter, Route, Routes, useLocation, useNavigate } from 'react-router-dom';
import { api } from './api';
import { IndexPage } from './pages/IndexPage';
import { SpecPage } from './pages/SpecPage';
import { FlowsPage } from './pages/FlowsPage';
import { FlowCanvasPage } from './pages/FlowCanvasPage';
import { Icon } from './ui';
import type { VarsData } from './types';
import { useBrowserVars } from './hooks/useBrowserVars';
import { TrellisMark } from './brand';

export function App() {
  return (
    <BrowserRouter>
      <TrellisShell />
    </BrowserRouter>
  );
}

function TrellisShell() {
  const navigate = useNavigate();
  const location = useLocation();
  const { vars: browserVars, save: saveBrowserVars } = useBrowserVars();
  const [varsData, setVarsData] = useState<VarsData | null>(null);
  const [env, setEnv] = useState('dev');
  const [theme, setTheme] = useState(() => localStorage.getItem('trellis-theme') ?? 'light');
  const [varsOpen, setVarsOpen] = useState(false);
  const [curlOpen, setCurlOpen] = useState(false);
  const [envModalOpen, setEnvModalOpen] = useState(false);
  const [refreshIndexKey, setRefreshIndexKey] = useState(0);
  const [scanning, setScanning] = useState(false);
  const [scanMsg, setScanMsg] = useState('');

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('trellis-theme', theme);
  }, [theme]);

  useEffect(() => {
    api.getVariables().then(data => {
      setVarsData(data);
      setEnv(data.env || data.envs[0] || 'dev');
    }).catch(() => {});
  }, []);

  const projectName = 'Trellis';
  const relPath = location.pathname.startsWith('/spec/') ? decodeURIComponent(location.pathname.slice('/spec/'.length)) : '';

  async function scanProject() {
    setScanning(true);
    setScanMsg('');
    try {
      const scan = await api.scanProject();
      if (scan.missing_count === 0) {
        setScanMsg(`${scan.routes_found} route(s), nothing missing`);
        return;
      }
      const imported = await api.importProjectRoutes();
      setRefreshIndexKey(key => key + 1);
      setScanMsg(`Imported ${imported.written.length} spec(s)`);
      if (location.pathname !== '/') navigate('/');
    } catch (e) {
      setScanMsg(String(e));
    } finally {
      setScanning(false);
      window.setTimeout(() => setScanMsg(''), 3500);
    }
  }

  async function addEnvironment(name: string, baseUrl: string) {
    const vars = {
      baseUrl: baseUrl.trim() || 'https://example.com',
      apiVersion: 'v1',
    };
    await api.saveVariables(name, vars);
    const envs = varsData?.envs.includes(name) ? varsData.envs : [...(varsData?.envs ?? []), name];
    setVarsData({ env: name, envs, vars });
    setEnv(name);
  }

  return (
    <div className="shell">
      <TopBar
        projectName={projectName}
        relPath={relPath}
        onHome={() => navigate('/')}
        onFlows={() => navigate('/flows')}
        theme={theme}
        setTheme={setTheme}
        env={env}
        setEnv={setEnv}
        envs={varsData?.envs.length ? varsData.envs : [env]}
        onOpenVars={() => setVarsOpen(true)}
        onOpenCurl={() => setCurlOpen(true)}
        onAddEnvironment={() => setEnvModalOpen(true)}
        onScanProject={scanProject}
        scanning={scanning}
        scanMsg={scanMsg}
      />
      <main className="main">
        <Routes>
          <Route path="/" element={<IndexPage env={env} refreshKey={refreshIndexKey} />} />
          <Route path="/flows" element={<FlowsPage />} />
          <Route path="/flows/*" element={<FlowCanvasPage />} />
          <Route path="/spec/*" element={<SpecPage env={env} varsData={varsData} browserVars={browserVars} />} />
        </Routes>
      </main>
      <VariablesDrawer
        open={varsOpen}
        onClose={() => setVarsOpen(false)}
        env={env}
        setEnv={setEnv}
        varsData={varsData}
        setVarsData={setVarsData}
        browserVars={browserVars}
        saveBrowserVars={saveBrowserVars}
      />
      {curlOpen && (
        <CurlImportModal
          onClose={() => setCurlOpen(false)}
          onImported={rel => {
            setCurlOpen(false);
            setRefreshIndexKey(k => k + 1);
            if (rel) navigate(`/spec/${rel}`);
          }}
        />
      )}
      {envModalOpen && (
        <AddEnvironmentModal
          existing={varsData?.envs ?? []}
          onClose={() => setEnvModalOpen(false)}
          onCreate={async (name, baseUrl) => {
            await addEnvironment(name, baseUrl);
            setEnvModalOpen(false);
            setVarsOpen(true);
          }}
        />
      )}
    </div>
  );
}

function TopBar({ projectName, relPath, onHome, onFlows, theme, setTheme, env, setEnv, envs, onOpenVars, onOpenCurl, onAddEnvironment, onScanProject, scanning, scanMsg }: {
  projectName: string;
  relPath: string;
  onHome: () => void;
  onFlows: () => void;
  theme: string;
  setTheme: (theme: string) => void;
  env: string;
  setEnv: (env: string) => void;
  envs: string[];
  onOpenVars: () => void;
  onOpenCurl: () => void;
  onAddEnvironment: () => void;
  onScanProject: () => void;
  scanning: boolean;
  scanMsg: string;
}) {
  return (
    <header className="topbar">
      <button className="brand" onClick={onHome}>
        <span className="brand-mark"><TrellisMark /></span>
        <span className="brand-name">Trellis</span>
      </button>
      <div className="crumbs">
        <span className="sep">/</span>
        <button onClick={onHome}>{projectName}</button>
        {relPath && <><span className="sep">/</span><span className="cur">{relPath}</span></>}
      </div>
      <div className="tnav-r">
        <button className="tnav-item" onClick={onFlows}><span className="ic"><Icon.arr /></span>Flows</button>
        <button className="tnav-item" onClick={onOpenVars}><span className="ic"><Icon.vars /></span>Variables</button>
        <button className="tnav-item" onClick={onOpenCurl}><span className="ic"><Icon.plus /></span>Import curl</button>
        <button className="tnav-item" onClick={onScanProject} disabled={scanning}>
          <span className="ic">{scanning ? <span className="pulse-dot tiny" /> : <Icon.search />}</span>
          {scanning ? 'Scanning' : 'Scan'}
        </button>
        {scanMsg && <span className={`top-msg ${scanMsg.startsWith('Imported') || scanMsg.includes('nothing') ? 'ok' : 'fail'}`}>{scanMsg}</span>}
        <label className="env-pill">
            <span className={`env-dot ${env}`} />
            <span className="lab">env</span>
            <select
              className="env-select"
              value={env}
              onChange={event => {
                if (event.target.value === '__create__') {
                  onAddEnvironment();
                  return;
                }
                setEnv(event.target.value);
              }}
            >
              {envs.map(item => <option key={item} value={item}>{item}</option>)}
              <option value="__create__">Create environment...</option>
            </select>
        </label>
        <button className="btn icon" onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')} title="Toggle theme">
          {theme === 'dark' ? <Icon.sun /> : <Icon.moon />}
        </button>
      </div>
    </header>
  );
}

function VariablesDrawer({ open, onClose, env, setEnv, varsData, setVarsData, browserVars, saveBrowserVars }: {
  open: boolean;
  onClose: () => void;
  env: string;
  setEnv: (env: string) => void;
  varsData: VarsData | null;
  setVarsData: (data: VarsData) => void;
  browserVars: Record<string, string>;
  saveBrowserVars: (vars: Record<string, string>) => void;
}) {
  const [tab, setTab] = useState<'browser' | 'env' | 'docs'>('browser');
  const [browserRows, setBrowserRows] = useState<[string, string][]>([]);
  const [envRows, setEnvRows] = useState<[string, string][]>([]);
  const [msg, setMsg] = useState('');

  useEffect(() => {
    if (open) {
      setBrowserRows(Object.entries(browserVars));
      setEnvRows(Object.entries(varsData?.vars ?? {}));
      setMsg('');
    }
  }, [browserVars, open, varsData]);

  if (!open) return null;

  async function saveEnv() {
    const vars = Object.fromEntries(envRows.filter(([key]) => key.trim()));
    await api.saveVariables(env, vars);
    setVarsData({ env, envs: varsData?.envs ?? [env], vars });
    setMsg('Saved');
  }

  return (
    <>
      <div className="drawer-backdrop" onClick={onClose} />
      <aside className="drawer" onClick={e => e.stopPropagation()}>
        <div className="drawer-h">
          <Icon.vars />
          <div><h2>Variables</h2><div className="sub">Browser-local values and env values</div></div>
          <button className="btn icon" onClick={onClose}><Icon.x /></button>
        </div>
        <div className="tabs">
          <button className={`tab ${tab === 'browser' ? 'is-on' : ''}`} onClick={() => setTab('browser')}>Browser <span className="badge">{browserRows.length}</span></button>
          <button className={`tab ${tab === 'env' ? 'is-on' : ''}`} onClick={() => setTab('env')}>Env: {env} <span className="badge">{envRows.length}</span></button>
          <button className={`tab ${tab === 'docs' ? 'is-on' : ''}`} onClick={() => setTab('docs')}>How it works</button>
        </div>
        <div className="drawer-body">
          {tab === 'browser' && (
            <div className="drawer-section">
              <p className="help">Stored in this browser only. Use for short-lived secrets like <code>{'{{token}}'}</code>.</p>
              <Rows rows={browserRows} setRows={setBrowserRows} secret />
              <div className="drawer-actions">
                <button className="add-row" onClick={() => setBrowserRows(rows => [...rows, ['', '']])}><Icon.plus /> Add variable</button>
                <button className="btn-primary" onClick={() => { saveBrowserVars(Object.fromEntries(browserRows.filter(([key]) => key.trim()))); setMsg('Saved'); }}>Save browser vars</button>
              </div>
            </div>
          )}
          {tab === 'env' && (
            <div className="drawer-section">
              <div className="seg">{(varsData?.envs ?? [env]).map(item => <button key={item} className={item === env ? 'is-on' : ''} onClick={() => setEnv(item)}>{item}</button>)}</div>
              <p className="help">Saved to <code>api-docs/_shared/env.md</code>. Do not store secrets here.</p>
              <Rows rows={envRows} setRows={setEnvRows} />
              <div className="drawer-actions">
                <button className="add-row" onClick={() => setEnvRows(rows => [...rows, ['', '']])}><Icon.plus /> Add variable</button>
                <button className="btn-primary" onClick={saveEnv}>Save env vars</button>
              </div>
            </div>
          )}
          {tab === 'docs' && (
            <div className="drawer-section docs-copy">
              <h3>Resolution order</h3>
              <ol><li>Per-run overrides typed in the runner</li><li>Browser-local values</li><li>Env file values</li><li>Spec defaults</li></ol>
              <p>Runner fields never change markdown. Markdown changes only through Edit source.</p>
            </div>
          )}
          {msg && <div className="drawer-section ok-text">{msg}</div>}
        </div>
      </aside>
    </>
  );
}

function Rows({ rows, setRows, secret = false }: { rows: [string, string][]; setRows: (fn: (rows: [string, string][]) => [string, string][]) => void; secret?: boolean }) {
  return (
    <div className="kv-list">
      {rows.map(([key, value], index) => (
        <div className="kv" key={index}>
          <input className="input flush mono" value={key} placeholder="name" onChange={e => setRows(rows => rows.map((row, i) => i === index ? [e.target.value, row[1]] : row))} />
          <input className="input flush mono" type={secret && /token|key|secret/i.test(key) ? 'password' : 'text'} value={value} placeholder="value" onChange={e => setRows(rows => rows.map((row, i) => i === index ? [row[0], e.target.value] : row))} />
          <span />
          <button className="row-del always" onClick={() => setRows(rows => rows.filter((_, i) => i !== index))}><Icon.x /></button>
        </div>
      ))}
    </div>
  );
}

function CurlImportModal({ onClose, onImported }: { onClose: () => void; onImported: (relPath?: string) => void }) {
  const [text, setText] = useState('');
  const [loading, setLoading] = useState(false);
  const [msg, setMsg] = useState('');
  async function submit() {
    setLoading(true);
    setMsg('');
    try {
      const result = await api.importCurl(text);
      if (result.error) setMsg(result.error);
      else onImported(result.rel_path);
    } catch (e) {
      setMsg(String(e));
    } finally {
      setLoading(false);
    }
  }
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal modal-wide" onClick={e => e.stopPropagation()}>
        <div className="modal-h">
          <span className="modal-icon"><Icon.plus /></span>
          <div><h2>Import from curl</h2><div className="sub">Paste a curl command. Trellis creates a markdown spec.</div></div>
          <button className="btn icon" onClick={onClose}><Icon.x /></button>
        </div>
        <div className="modal-b modal-pad">
          <textarea className="input mono curl-textarea" value={text} onChange={e => setText(e.target.value)} spellCheck={false} placeholder={'curl https://api.example.com/users -H "Accept: application/json"'} />
          {msg && <p className="fail-text">{msg}</p>}
        </div>
        <div className="modal-f">
          <div className="note">Existing specs are not overwritten.</div>
          <button className="btn sm" onClick={onClose}>Cancel</button>
          <button className="btn-primary btn-sm-primary" onClick={submit} disabled={loading || !text.trim()}>{loading ? 'Importing…' : 'Import endpoint'}</button>
        </div>
      </div>
    </div>
  );
}

function AddEnvironmentModal({ existing, onClose, onCreate }: {
  existing: string[];
  onClose: () => void;
  onCreate: (name: string, baseUrl: string) => Promise<void>;
}) {
  const [name, setName] = useState('');
  const [baseUrl, setBaseUrl] = useState('https://example.com');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');

  async function submit() {
    const envName = name.trim();
    setError('');
    if (!envName) {
      setError('Environment name is required.');
      return;
    }
    if (!/^[a-zA-Z0-9_-]+$/.test(envName)) {
      setError('Use letters, numbers, underscore, or dash.');
      return;
    }
    if (existing.includes(envName)) {
      setError(`Environment "${envName}" already exists.`);
      return;
    }
    setSaving(true);
    try {
      await onCreate(envName, baseUrl);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal env-modal" onClick={e => e.stopPropagation()}>
        <div className="modal-h">
          <span className="modal-icon"><Icon.plus /></span>
          <div>
            <h2>New environment</h2>
            <div className="sub">Create a new env in <code>api-docs/_shared/env.md</code>.</div>
          </div>
          <button className="btn icon" onClick={onClose}><Icon.x /></button>
        </div>
        <div className="modal-b modal-pad env-form">
          <label>
            <span>Environment name</span>
            <input className="input mono" value={name} onChange={e => setName(e.target.value)} placeholder="qa" autoFocus />
          </label>
          <label>
            <span>Base URL</span>
            <input className="input mono" value={baseUrl} onChange={e => setBaseUrl(e.target.value)} placeholder="https://qa.example.com" />
          </label>
          {error && <p className="fail-text env-error">{error}</p>}
        </div>
        <div className="modal-f">
          <div className="note">You can add more variables after the env is created.</div>
          <button className="btn sm" onClick={onClose}>Cancel</button>
          <button className="btn-primary btn-sm-primary" onClick={submit} disabled={saving}>{saving ? 'Creating…' : 'Create env'}</button>
        </div>
      </div>
    </div>
  );
}
