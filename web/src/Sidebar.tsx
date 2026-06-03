import { useEffect, useMemo, useRef, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { api } from './api';
import { getStoredRun } from './hooks/useRunResults';
import type { FlowEntry, GitBranchesData, ResourceGroup } from './types';
import { Icon } from './ui';

// ── Brand mark: [· ● ·] bracket-dot logo ─────────────────────────────────────
function BrandMark({ size = 22, color = 'var(--accent)' }: { size?: number; color?: string }) {
  const h = size * (28 / 36);
  return (
    <svg viewBox="0 0 36 28" width={size} height={h} fill="none" stroke={color}
      strokeWidth="2.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d="M11 4 L5 4 L5 24 L11 24" />
      <path d="M25 4 L31 4 L31 24 L25 24" />
      <circle cx="14" cy="14" r="1.1" fill={color} stroke="none" />
      <circle cx="18" cy="14" r="2.1" fill={color} stroke="none" />
      <circle cx="22" cy="14" r="1.1" fill={color} stroke="none" />
    </svg>
  );
}

export { BrandMark };

// ── Status dot for a spec row ─────────────────────────────────────────────────
function NavDot({ relPath }: { relPath: string }) {
  const r = getStoredRun(relPath);
  const cls = !r ? 'never' : r.passed ? 'ok' : 'fail';
  return <span className={`nav-dot ${cls}`} />;
}

// ── Collection ⋯ context menu ─────────────────────────────────────────────────
function CollectionMenu({ onNewRequest }: { onNewRequest: () => void }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const h = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', h);
    return () => document.removeEventListener('mousedown', h);
  }, []);
  return (
    <div className="sb-more-wrap" ref={ref}>
      <button
        className={`sb-more${open ? ' is-open' : ''}`}
        title="More options"
        onClick={(e) => { e.stopPropagation(); setOpen(o => !o); }}
      >
        <Icon.dots />
      </button>
      {open && (
        <div className="sb-more-menu" onClick={e => e.stopPropagation()}>
          <button className="sb-more-item" onClick={() => { setOpen(false); onNewRequest(); }}>
            <Icon.plus /> New request
          </button>
          <button className="sb-more-item" onClick={() => setOpen(false)}>
            <Icon.folder /> New folder
          </button>
          <div className="sb-more-divider" />
          <button className="sb-more-item" onClick={() => setOpen(false)}>
            <Icon.edit /> Rename
          </button>
          <button className="sb-more-item danger" onClick={() => setOpen(false)}>
            <Icon.trash /> Delete
          </button>
        </div>
      )}
    </div>
  );
}

// ── Workspace Switcher (compact = topbar inline, full = sidebar top) ──────────
export interface WorkspaceInfo {
  name: string;
  path: string;
}

export function WorkspaceSwitcher({
  compact = false,
  workspaceTick = 0,
  onNavigateHome,
}: {
  compact?: boolean;
  workspaceTick?: number;
  onNavigateHome?: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [current, setCurrent] = useState<WorkspaceInfo | null>(null);
  const [workspaces, setWorkspaces] = useState<WorkspaceInfo[]>([]);
  const ref = useRef<HTMLDivElement>(null);
  const navigate = useNavigate();

  useEffect(() => {
    const h = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', h);
    return () => document.removeEventListener('mousedown', h);
  }, []);

  useEffect(() => {
    api.getWorkspaceCurrent()
      .then(ws => { if (ws?.name) setCurrent({ name: ws.name, path: ws.path ?? '' }); })
      .catch(() => {});
    Promise.all([api.getWorkspaceRecent(), api.getWorkspaceAll()])
      .then(([recent, all]) => {
        const seen = new Set<string>();
        const merged: WorkspaceInfo[] = [];
        for (const w of [...recent, ...all]) {
          if (!seen.has(w.path)) { seen.add(w.path); merged.push({ name: w.name, path: w.path }); }
        }
        setWorkspaces(merged);
      })
      .catch(() => {});
  }, [workspaceTick]);

  async function switchTo(path: string) {
    try {
      await api.openWorkspace(path);
      const ws = workspaces.find(w => w.path === path);
      if (ws) setCurrent(ws);
      window.dispatchEvent(new CustomEvent('rqb:workspace-switched'));
      navigate('/');
    } catch {}
    setOpen(false);
  }

  const displayPath = current?.path.replace(/^\/Users\/[^/]+/, '~') ?? '—';

  return (
    <div className={`ws-switcher${compact ? ' compact' : ''}`} ref={ref}>
      <div className={`ws-trigger${open ? ' is-open' : ''}`}>
        <button
          className="ws-main"
          onClick={() => { onNavigateHome?.(); navigate('/'); setOpen(false); }}
          title="Overview"
        >
          <span className="ws-folder"><Icon.folder /></span>
          <span className="ws-meta">
            <span className="ws-name">{current?.name ?? 'No workspace'}</span>
            {!compact && <span className="ws-path">{displayPath}</span>}
          </span>
        </button>
        <button className="ws-caret-btn" onClick={() => setOpen(o => !o)} title="Switch workspace">
          <svg width="11" height="11" viewBox="0 0 12 12" fill="none" stroke="currentColor"
            strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="3.5,5 6,2.5 8.5,5" /><polyline points="3.5,7 6,9.5 8.5,7" />
          </svg>
        </button>
      </div>
      {open && (
        <div className="ws-menu" onClick={e => e.stopPropagation()}>
          <div className="ws-menu-label">Workspaces</div>
          {workspaces.map(w => (
            <button
              key={w.path}
              className={`ws-menu-item${w.path === current?.path ? ' is-active' : ''}`}
              onClick={() => switchTo(w.path)}
            >
              <span className="ws-folder sm"><Icon.folder /></span>
              <span className="ws-mi-meta">
                <span className="ws-mi-name">{w.name}</span>
                <span className="ws-mi-path">{w.path.replace(/^\/Users\/[^/]+/, '~')}</span>
              </span>
              {w.path === current?.path && <span className="ws-mi-check"><Icon.check /></span>}
            </button>
          ))}
          {workspaces.length === 0 && (
            <div style={{ padding: '8px 10px', fontSize: 12, color: 'var(--fg-4)' }}>No workspaces yet</div>
          )}
          <div className="ws-menu-divider" />
          <button className="ws-menu-action" onClick={() => { navigate('/workspaces'); setOpen(false); }}>
            <Icon.folder /> Manage workspaces…
          </button>
        </div>
      )}
    </div>
  );
}

// ── Main Sidebar ──────────────────────────────────────────────────────────────
export function Sidebar({ runTick, workspaceTick, onNewRequest }: {
  runTick: number;
  workspaceTick: number;
  onNewRequest: () => void;
}) {
  const location = useLocation();
  const navigate = useNavigate();

  const [groups, setGroups] = useState<ResourceGroup[]>([]);
  const [flows, setFlows] = useState<FlowEntry[]>([]);
  const [query, setQuery] = useState('');
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  const [secOpen, setSecOpen] = useState({ endpoints: true, flows: true });

  function refreshEndpoints() {
    api.getIndex().then(data => setGroups(data.groups)).catch(() => {});
    api.getFlows().then(data => setFlows(data.flows)).catch(() => {});
  }

  useEffect(() => {
    refreshEndpoints();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [workspaceTick]);

  useEffect(() => {
    window.addEventListener('rqb:endpoint-created', refreshEndpoints);
    return () => window.removeEventListener('rqb:endpoint-created', refreshEndpoints);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  void runTick;

  const q = query.trim().toLowerCase();
  const filteredGroups = useMemo(() =>
    groups.map(g => ({
      ...g,
      specs: g.specs.filter(s =>
        !q || s.path.toLowerCase().includes(q) || s.title.toLowerCase().includes(q) || s.method.toLowerCase().includes(q)
      ),
    })).filter(g => g.specs.length > 0),
    [groups, q]
  );

  const totalSpecs = groups.reduce((n, g) => n + g.specs.length, 0);

  const activeRel = location.pathname.startsWith('/spec/')
    ? decodeURIComponent(location.pathname.slice('/spec/'.length))
    : null;
  const activeFlowRel = location.pathname.startsWith('/flows/')
    ? decodeURIComponent(location.pathname.slice('/flows/'.length))
    : null;

  return (
    <aside className="sidebar">
      <div className="sb-search">
        <span className="sb-ic"><Icon.search /></span>
        <input
          value={query}
          onChange={e => setQuery(e.target.value)}
          placeholder="Search endpoints…"
        />
        {query && (
          <button className="sb-clear" onClick={() => setQuery('')}><Icon.x /></button>
        )}
      </div>

      <nav className="sb-scroll">
        {/* Collections */}
        <div className={`sb-section${activeRel !== null ? ' has-active' : ''}`}>
          <div className="sb-section-h">
            <button
              className="sb-section-toggle"
              onClick={() => setSecOpen(s => ({ ...s, endpoints: !s.endpoints }))}
              title={secOpen.endpoints ? 'Collapse' : 'Expand'}
            >
              <Icon.chev open={secOpen.endpoints} />
            </button>
            <span className="sb-section-ic"><Icon.folder /></span>
            <span className="sb-section-name">Endpoints</span>
            <span className="sb-section-r">
              <span className="sb-section-badge">{totalSpecs}</span>
              <button
                className="sb-add"
                title="New request"
                onClick={(e) => { e.stopPropagation(); onNewRequest(); }}
              >
                <Icon.plus />
              </button>
            </span>
          </div>
          {secOpen.endpoints && (
            <div className="sb-tree">
              {filteredGroups.map(g => {
                const open = !collapsed[g.resource];
                return (
                  <div key={g.resource} className="sb-group">
                    <div
                      className="sb-group-h"
                      onClick={() => setCollapsed(c => ({ ...c, [g.resource]: open }))}
                    >
                      <span className="chev"><Icon.chev open={open} /></span>
                      <span className="sb-group-name">{g.resource}</span>
                      <span className="sb-group-r">
                        <span className="sb-group-count">{g.specs.length}</span>
                        <span className="sb-group-actions">
                          <button
                            className="sb-add sm"
                            title={`New in ${g.resource}`}
                            onClick={(e) => { e.stopPropagation(); onNewRequest(); }}
                          >
                            <Icon.plus />
                          </button>
                          <CollectionMenu onNewRequest={onNewRequest} />
                        </span>
                      </span>
                    </div>
                    {open && g.specs.map(s => (
                      <button
                        key={s.rel_path}
                        className={`sb-item${activeRel === s.rel_path ? ' is-active' : ''}`}
                        onClick={() => navigate(`/spec/${s.rel_path}`)}
                        title={`${s.method} ${s.path}`}
                      >
                        <span className={`sb-method ${s.method.toLowerCase()}`}>
                          {s.method === 'DELETE' ? 'DEL' : s.method}
                        </span>
                        <span className="sb-item-path">
                          {s.path.replace(`/${g.resource}`, '') || '/'}
                        </span>
                        <NavDot relPath={s.rel_path} />
                      </button>
                    ))}
                  </div>
                );
              })}
              {filteredGroups.length === 0 && (
                <div className="sb-empty">{q ? 'No matches' : 'No endpoints yet'}</div>
              )}
            </div>
          )}
        </div>

        {/* Flows */}
        <div className={`sb-section${activeFlowRel !== null ? ' has-active' : ''}`}>
          <div className="sb-section-h">
            <button
              className="sb-section-toggle"
              onClick={() => setSecOpen(s => ({ ...s, flows: !s.flows }))}
              title={secOpen.flows ? 'Collapse' : 'Expand'}
            >
              <Icon.chev open={secOpen.flows} />
            </button>
            <span className="sb-section-ic"><Icon.flow /></span>
            <button className="sb-section-name as-link" onClick={() => navigate('/flows')}>
              Flows
            </button>
            <span className="sb-section-r">
              <span className="sb-section-badge">{flows.length}</span>
              <button
                className="sb-add"
                title="New flow"
                onClick={(e) => { e.stopPropagation(); navigate('/flows'); }}
              >
                <Icon.plus />
              </button>
            </span>
          </div>
          {secOpen.flows && (
            <div className="sb-tree">
              {flows.map(f => (
                <button
                  key={f.rel_path}
                  className={`sb-item flow${activeFlowRel === f.rel_path ? ' is-active' : ''}`}
                  onClick={() => navigate(`/flows/${f.rel_path}`)}
                  title={f.title}
                >
                  <span className="sb-flow-ic"><Icon.flow /></span>
                  <span className="sb-item-path">{f.title}</span>
                </button>
              ))}
              <button className="sb-item add flow" onClick={() => navigate('/flows')}>
                <span className="sb-flow-ic"><Icon.plus /></span>
                <span className="sb-item-path">New flow…</span>
              </button>
            </div>
          )}
        </div>
      </nav>
    </aside>
  );
}

// ── Status bar ────────────────────────────────────────────────────────────────
export function StatusBar({ env, onScan, scanning, scanMsg, runTick = 0 }: {
  env: string;
  onScan: () => void;
  scanning: boolean;
  scanMsg: string;
  runTick?: number;
}) {
  const [git, setGit] = useState<GitBranchesData | null>(null);
  const [currentBranch, setCurrentBranch] = useState<string | null>(null);
  const [branchOpen, setBranchOpen] = useState(false);
  const [branchBusy, setBranchBusy] = useState(false);
  const branchRef = useRef<HTMLDivElement>(null);
  const [totals, setTotals] = useState({ passed: 0, failed: 0, never: 0 });

  useEffect(() => {
    api.getGitBranches()
      .then(d => { if (d.is_repo && d.current) { setGit(d); setCurrentBranch(d.current); } })
      .catch(() => {});
  }, []);

  useEffect(() => {
    api.getIndex().then(data => {
      let passed = 0, failed = 0, never = 0;
      for (const g of data.groups) {
        for (const s of g.specs) {
          const r = getStoredRun(s.rel_path);
          if (!r) never++;
          else if (r.passed) passed++;
          else failed++;
        }
      }
      setTotals({ passed, failed, never });
    }).catch(() => {});
  }, [runTick]);

  useEffect(() => {
    const h = (e: MouseEvent) => {
      if (branchRef.current && !branchRef.current.contains(e.target as Node)) setBranchOpen(false);
    };
    document.addEventListener('mousedown', h);
    return () => document.removeEventListener('mousedown', h);
  }, []);

  async function doCheckout(branch: string) {
    if (branch === currentBranch || branchBusy) { setBranchOpen(false); return; }
    setBranchBusy(true);
    try {
      const next = await api.checkoutGitBranch(branch);
      setGit(next);
      setCurrentBranch(next.current ?? branch);
    } catch {}
    finally { setBranchBusy(false); setBranchOpen(false); }
  }

  return (
    <footer className="statusbar">
      {git && currentBranch && (
        <div className="git-select" ref={branchRef}>
          <button
            className={`sb-stat git${branchOpen ? ' is-open' : ''}`}
            onClick={() => setBranchOpen(o => !o)}
            disabled={branchBusy}
          >
            <Icon.branch />
            <span className="git-name">{currentBranch}</span>
            {git.dirty && <span className="git-dirty" title="Uncommitted changes">●</span>}
            <svg width="9" height="9" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round">
              <path d="M2 4 L5 7 L8 4" />
            </svg>
          </button>
          {branchOpen && (
            <div className="git-menu" onClick={e => e.stopPropagation()}>
              <div className="git-menu-label">Switch branch</div>
              {git.branches.map(b => (
                <button
                  key={b.name}
                  className={`git-menu-item${b.name === currentBranch ? ' is-active' : ''}`}
                  onClick={() => doCheckout(b.name)}
                  disabled={branchBusy}
                >
                  <Icon.branch />
                  <span className="git-mi-name">{b.name}</span>
                  {b.remote && <span style={{ fontSize: 10, color: 'var(--fg-4)', marginLeft: 'auto' }}>remote</span>}
                  {b.name === currentBranch && <span className="git-mi-check"><Icon.check /></span>}
                </button>
              ))}
            </div>
          )}
        </div>
      )}

      <button
        className={`sb-stat sync${scanning ? ' is-syncing' : ''}`}
        onClick={onScan}
        disabled={scanning}
        title={scanning ? 'Scanning…' : 'Scan project for new API specs'}
      >
        <Icon.scan />
        <span>{scanning ? 'Scanning…' : scanMsg || 'Scan'}</span>
      </button>

      <div className="status-spacer" />

      {totals.passed > 0 && (
        <div className="sb-stat" title="Passing specs"><span className="st-dot ok" />{totals.passed} passing</div>
      )}
      {totals.failed > 0 && (
        <div className="sb-stat" title="Failing specs"><span className="st-dot fail" />{totals.failed} failing</div>
      )}
      {totals.never > 0 && (
        <div className="sb-stat muted" title="Never run"><span className="st-dot never" />{totals.never} idle</div>
      )}
      {(totals.passed > 0 || totals.failed > 0 || totals.never > 0) && (
        <div className="sb-stat divider" />
      )}
      <div className="sb-stat muted">
        env <b style={{ color: 'var(--fg-2)', fontWeight: 600 }}>{env}</b>
      </div>
    </footer>
  );
}
