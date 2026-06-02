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

// ── Workspace Switcher ────────────────────────────────────────────────────────
export interface WorkspaceInfo {
  name: string;
  path: string;
}

export function WorkspaceSwitcher({ workspace }: { workspace: WorkspaceInfo | null }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const navigate = useNavigate();

  useEffect(() => {
    const h = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', h);
    return () => document.removeEventListener('mousedown', h);
  }, []);

  const displayPath = workspace?.path.replace(/^\/Users\/[^/]+/, '~') ?? '—';

  return (
    <div className="ws-switcher" ref={ref}>
      <button className={`ws-trigger ${open ? 'is-open' : ''}`} onClick={() => setOpen(o => !o)}>
        <span className="ws-folder"><Icon.folder /></span>
        <span className="ws-meta">
          <span className="ws-name">{workspace?.name ?? 'No workspace'}</span>
          <span className="ws-path">{displayPath}</span>
        </span>
        <span className="ws-caret">
          <svg width="11" height="11" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="3.5,5 6,2.5 8.5,5" /><polyline points="3.5,7 6,9.5 8.5,7" />
          </svg>
        </span>
      </button>
      {open && (
        <div className="ws-menu" onClick={e => e.stopPropagation()}>
          <div className="ws-menu-label">Workspace</div>
          {workspace && (
            <div className="ws-menu-item is-active">
              <span className="ws-folder sm"><Icon.folder /></span>
              <span className="ws-mi-meta">
                <span className="ws-mi-name">{workspace.name}</span>
                <span className="ws-mi-path">{displayPath}</span>
              </span>
              <span className="ws-mi-check"><Icon.check /></span>
            </div>
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

export function GitBranchSwitcher({ refreshKey, onBranchChange }: {
  refreshKey: number;
  onBranchChange: () => void;
}) {
  const [data, setData] = useState<GitBranchesData | null>(null);
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const h = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', h);
    return () => document.removeEventListener('mousedown', h);
  }, []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError('');
    setData(null);
    api.getGitBranches()
      .then(next => {
        if (!cancelled) setData(next);
      })
      .catch(e => {
        if (!cancelled) setError(errorMessage(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => { cancelled = true; };
  }, [refreshKey]);

  async function checkout(branch: string) {
    if (branch === data?.current || busy) {
      setOpen(false);
      return;
    }
    setBusy(true);
    setError('');
    try {
      const next = await api.checkoutGitBranch(branch);
      setData(next);
      setOpen(false);
      onBranchChange();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  const isRepo = data?.is_repo ?? false;
  const current = loading ? 'Loading...' : data?.current ?? 'No git repo';
  const branchCount = data?.branches.length ?? 0;
  const disabled = loading || busy || !isRepo || branchCount === 0;

  return (
    <div className="git-switcher" ref={ref}>
      <button
        className={`git-trigger ${open ? 'is-open' : ''}`}
        onClick={() => !disabled && setOpen(value => !value)}
        disabled={loading || busy || !isRepo}
        title={isRepo ? 'Switch git branch' : 'Workspace is not a git repository'}
      >
        <span className="git-ic"><Icon.branch /></span>
        <span className="git-meta">
          <span className="git-lab">branch</span>
          <span className="git-name">{current}</span>
        </span>
        {data?.dirty && <span className="git-dirty">dirty</span>}
        <span className="git-caret"><Icon.chev open={open} /></span>
      </button>

      {open && (
        <div className="git-menu" onClick={event => event.stopPropagation()}>
          <div className="ws-menu-label">Git branch</div>
          {data?.branches.map(branch => (
            <button
              key={`${branch.remote ? 'remote' : 'local'}:${branch.name}`}
              className={`git-menu-item${branch.current ? ' is-active' : ''}`}
              onClick={() => checkout(branch.name)}
              disabled={busy}
            >
              <span className="git-mi-main">
                <span className="git-mi-name">{branch.name}</span>
                {(branch.summary || branch.commit) && (
                  <span className="git-mi-sub">
                    {branch.commit}{branch.commit && branch.summary ? ' - ' : ''}{branch.summary}
                  </span>
                )}
              </span>
              {branch.remote && <span className="git-remote">remote</span>}
              {branch.current && <span className="ws-mi-check"><Icon.check /></span>}
            </button>
          ))}
          {error && <div className="git-error">{error}</div>}
        </div>
      )}
      {!open && error && <div className="git-error inline">{error}</div>}
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

  useEffect(() => {
    api.getIndex()
      .then(data => setGroups(data.groups))
      .catch(() => {});
    api.getFlows()
      .then(data => setFlows(data.flows))
      .catch(() => {});
  }, [workspaceTick]);

  // Re-render dots when a run is saved
  // runTick prop causes a re-render so NavDot reads fresh localStorage
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

  // Derive active rel_path from current route
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
        {/* Overview */}
        <button
          className={`sb-link ${location.pathname === '/' ? 'is-active' : ''}`}
          onClick={() => navigate('/')}
        >
          <span className="sb-link-ic"><Icon.grid /></span>
          Overview
          <span className="sb-link-end">{totalSpecs}</span>
        </button>

        {/* Collections */}
        <div className="sb-section">
          <button
            className="sb-section-h"
            onClick={() => setSecOpen(s => ({ ...s, endpoints: !s.endpoints }))}
          >
            <span className="chev"><Icon.chev open={secOpen.endpoints} /></span>
            <span className="sb-section-name">Collections</span>
            <span className="sb-section-count">{totalSpecs}</span>
            <span
              role="button"
              tabIndex={0}
              className="sb-section-action"
              title="New request"
              onClick={event => {
                event.stopPropagation();
                onNewRequest();
              }}
              onKeyDown={event => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  event.stopPropagation();
                  onNewRequest();
                }
              }}
            >
              <Icon.plus />
            </span>
          </button>
          {secOpen.endpoints && (
            <div className="sb-tree">
              {filteredGroups.map(g => {
                const open = !collapsed[g.resource];
                return (
                  <div key={g.resource} className="sb-group">
                    <button
                      className="sb-group-h"
                      onClick={() => setCollapsed(c => ({ ...c, [g.resource]: open }))}
                    >
                      <span className="chev"><Icon.chev open={open} /></span>
                      <span className="sb-folder-ic"><Icon.folder /></span>
                      <span className="sb-group-name">{g.resource}</span>
                      <span className="sb-group-count">{g.specs.length}</span>
                      <span
                        role="button"
                        tabIndex={0}
                        className="sb-group-action"
                        title={`New request in ${g.resource}`}
                        onClick={event => {
                          event.stopPropagation();
                          onNewRequest();
                        }}
                        onKeyDown={event => {
                          if (event.key === 'Enter' || event.key === ' ') {
                            event.preventDefault();
                            event.stopPropagation();
                            onNewRequest();
                          }
                        }}
                      >
                        <Icon.plus />
                      </span>
                    </button>
                    {open && g.specs.map(s => (
                      <button
                        key={s.rel_path}
                        className={`sb-item ${activeRel === s.rel_path ? 'is-active' : ''}`}
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
        <div className="sb-section">
          <button
            className="sb-section-h"
            onClick={() => setSecOpen(s => ({ ...s, flows: !s.flows }))}
          >
            <span className="chev"><Icon.chev open={secOpen.flows} /></span>
            <span className="sb-section-name">Flows</span>
            <span className="sb-section-count">{flows.length}</span>
          </button>
          {secOpen.flows && (
            <div className="sb-tree">
              {flows.map(f => (
                <button
                  key={f.rel_path}
                  className={`sb-item flow ${activeFlowRel === f.rel_path ? 'is-active' : ''}`}
                  onClick={() => navigate(`/flows/${f.rel_path}`)}
                  title={f.title}
                >
                  <span className="sb-flow-ic"><Icon.arr /></span>
                  <span className="sb-item-path">{f.title}</span>
                </button>
              ))}
              <button
                className="sb-item add flow"
                onClick={() => navigate('/flows')}
              >
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

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
