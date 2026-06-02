import { useEffect, useRef, useState } from 'react';
import { BrowserRouter, Route, Routes, useLocation, useNavigate } from 'react-router-dom';
import { api } from './api';
import { IndexPage } from './pages/IndexPage';
import { SpecPage } from './pages/SpecPage';
import { FlowsPage } from './pages/FlowsPage';
import { FlowCanvasPage } from './pages/FlowCanvasPage';
import { RequestPage } from './pages/RequestPage';
import { WorkspacePage } from './pages/WorkspacePage';
import { Icon } from './ui';
import type { VarsData } from './types';
import { useBrowserVars } from './hooks/useBrowserVars';
import { BrandMark, Sidebar, StatusBar, WorkspaceSwitcher } from './Sidebar';

export function App() {
  return (
    <BrowserRouter>
      <MadShell />
    </BrowserRouter>
  );
}

function MadShell() {
  const navigate = useNavigate();
  const location = useLocation();
  const { vars: browserVars, save: saveBrowserVars } = useBrowserVars();
  const [varsData, setVarsData] = useState<VarsData | null>(null);
  const [env, setEnv] = useState('dev');
  const [theme, setTheme] = useState(() => localStorage.getItem('mad-theme') ?? 'light');
  const [mockMode, setMockMode] = useState(false);
  const [varsOpen, setVarsOpen] = useState(false);
  const [envModalOpen, setEnvModalOpen] = useState(false);
  const [refreshIndexKey] = useState(0);
  const [syncing, setSyncing] = useState(false);
  const [syncMsg, setSyncMsg] = useState('');
  const [runTick, setRunTick] = useState(0);
  const [workspaceTick, setWorkspaceTick] = useState(0);

  const refreshWorkspaceData = () => setWorkspaceTick(tick => tick + 1);

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('mad-theme', theme);
  }, [theme]);

  useEffect(() => {
    api.getVariables().then(data => {
      setVarsData(data);
      setEnv(data.env || data.envs[0] || 'dev');
    }).catch(() => {});
    api.getIndex().then(data => {
      setMockMode(data.mock_mode ?? false);
    }).catch(() => {});
    const handler = () => setRunTick(t => t + 1);
    window.addEventListener('mad:run-saved', handler);
    const onEndpointCreated = () => refreshWorkspaceData();
    const onWorkspaceSwitched = () => refreshWorkspaceData();
    window.addEventListener('mad:endpoint-created', onEndpointCreated);
    window.addEventListener('mad:workspace-switched', onWorkspaceSwitched);
    return () => {
      window.removeEventListener('mad:run-saved', handler);
      window.removeEventListener('mad:endpoint-created', onEndpointCreated);
      window.removeEventListener('mad:workspace-switched', onWorkspaceSwitched);
    };
  }, []);

  const relPath = location.pathname.startsWith('/spec/') ? decodeURIComponent(location.pathname.slice('/spec/'.length)) : '';

  async function syncProject() {
    if (syncing) return;
    setSyncing(true);
    setSyncMsg('');
    try {
      const scan = await api.scanProject();
      if (scan.missing_count === 0) {
        setSyncMsg(`${scan.routes_found} route(s), up to date`);
        return;
      }
      const imported = await api.importProjectRoutes();
      refreshWorkspaceData();
      setSyncMsg(`Synced ${imported.written.length} spec(s)`);
      if (location.pathname !== '/') navigate('/');
    } catch (e) {
      setSyncMsg(String(e));
    } finally {
      setSyncing(false);
      window.setTimeout(() => setSyncMsg(''), 3500);
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
        relPath={relPath}
        onHome={() => navigate('/')}
        onNewRequest={() => navigate('/request')}
        theme={theme}
        setTheme={setTheme}
        env={env}
        setEnv={setEnv}
        envs={varsData?.envs.length ? varsData.envs : [env]}
        onOpenVars={() => setVarsOpen(true)}
        onAddEnvironment={() => setEnvModalOpen(true)}
        mockMode={mockMode}
        workspaceTick={workspaceTick}
      />
      <div className="body-row">
        <Sidebar
          runTick={runTick}
          workspaceTick={workspaceTick}
          onNewRequest={() => navigate('/request')}
        />
        <main className="main">
          <Routes>
            <Route path="/" element={<IndexPage env={env} refreshKey={refreshIndexKey + workspaceTick} />} />
            <Route path="/flows" element={<FlowsPage key={workspaceTick} />} />
            <Route path="/flows/*" element={<FlowCanvasPage key={workspaceTick} />} />
            <Route path="/spec/*" element={<SpecPage key={workspaceTick} env={env} varsData={varsData} browserVars={browserVars} mockMode={mockMode} />} />
            <Route path="/request" element={<RequestPage key={`${location.key}-${workspaceTick}`} env={env} varsData={varsData} />} />
            <Route path="/workspaces" element={<WorkspacePage key={workspaceTick} onWorkspaceChanged={refreshWorkspaceData} />} />
          </Routes>
        </main>
      </div>
      <StatusBar
        env={env}
        onScan={syncProject}
        scanning={syncing}
        scanMsg={syncMsg}
        runTick={runTick}
      />
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

function EnvSwitcher({ envs, value, onChange, onAdd }: {
  envs: string[];
  value: string;
  onChange: (env: string) => void;
  onAdd: () => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const h = (e: MouseEvent) => { if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false); };
    document.addEventListener('mousedown', h);
    return () => document.removeEventListener('mousedown', h);
  }, []);
  const dotBg = (env: string) => env.includes('prod') ? 'var(--fail)' : env.includes('stag') ? 'var(--warn)' : 'var(--ok)';
  return (
    <div ref={ref} className="env-pill" onClick={() => setOpen(o => !o)} style={{ userSelect: 'none' }}>
      <span className="env-dot" style={{ background: dotBg(value) }} />
      <span className="lab">env</span>
      <span>{value}</span>
      <svg width="9" height="9" viewBox="0 0 10 10" fill="none" style={{ color: 'var(--fg-4)' }}>
        <path d="M2 4 L5 7 L8 4" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
      </svg>
      {open && (
        <div className="menu" onClick={e => e.stopPropagation()}>
          {envs.map(e => (
            <div key={e} className="menu-item" onClick={() => { onChange(e); setOpen(false); }}>
              <span className="dot" style={{ background: dotBg(e) }} />
              <span>{e}</span>
              <span className="end">{e === value && <Icon.check />}</span>
            </div>
          ))}
          <div className="menu-divider" />
          <div className="menu-item" style={{ color: 'var(--fg-3)' }} onClick={() => { onAdd(); setOpen(false); }}>
            <Icon.plus /> new environment
          </div>
        </div>
      )}
    </div>
  );
}

function TopBar({ relPath, onHome, onNewRequest, theme, setTheme, env, setEnv, envs, onOpenVars, onAddEnvironment, mockMode, workspaceTick }: {
  relPath: string;
  onHome: () => void;
  onNewRequest: () => void;
  theme: string;
  setTheme: (theme: string) => void;
  env: string;
  setEnv: (env: string) => void;
  envs: string[];
  onOpenVars: () => void;
  onAddEnvironment: () => void;
  mockMode?: boolean;
  workspaceTick: number;
}) {
  return (
    <header className="topbar">
      <button className="brand" onClick={onHome}>
        <span className="brand-mark"><BrandMark size={20} color="var(--accent)" /></span>
        <span className="brand-name">MarkApiDown</span>
      </button>
      <span className="topbar-div" />
      <WorkspaceSwitcher compact workspaceTick={workspaceTick} onNavigateHome={onHome} />
      {relPath && (
        <div className="crumbs">
          <span className="sep">/</span>
          <span className="cur">{relPath}</span>
        </div>
      )}
      <div className="tnav-r">
        <button className="tnav-item primary icon" onClick={onNewRequest} title="New request"><Icon.plus /></button>
        <button className="tnav-item" onClick={onOpenVars}><span className="ic"><Icon.vars /></span>Variables</button>
        {mockMode && <span className="mock-pill">mock</span>}
        <EnvSwitcher envs={envs} value={env} onChange={setEnv} onAdd={onAddEnvironment} />
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
