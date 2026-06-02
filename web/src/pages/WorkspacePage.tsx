import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { api } from '../api';
import type { WorkspaceEntry } from '../types';
import { Icon } from '../ui';

export function WorkspacePage({ onWorkspaceChanged }: { onWorkspaceChanged?: () => void }) {
  const navigate = useNavigate();
  const [current, setCurrent] = useState<WorkspaceEntry | null>(null);
  const [recent, setRecent] = useState<WorkspaceEntry[]>([]);
  const [all, setAll] = useState<WorkspaceEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState('');
  const [busy, setBusy] = useState(false);
  const [openByPath, setOpenByPath] = useState('');
  const [showOpenByPath, setShowOpenByPath] = useState(false);
  const [createPath, setCreatePath] = useState('');
  const [dropOpen, setDropOpen] = useState(false);
  const dropRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const h = (e: MouseEvent) => {
      if (dropRef.current && !dropRef.current.contains(e.target as Node)) setDropOpen(false);
    };
    document.addEventListener('mousedown', h);
    return () => document.removeEventListener('mousedown', h);
  }, []);

  useEffect(() => {
    Promise.all([
      api.getWorkspaceCurrent(),
      api.getWorkspaceRecent(),
      api.getWorkspaceAll(),
    ])
      .then(([curr, rec, a]) => {
        setCurrent(curr);
        setRecent(rec);
        setAll(a);
      })
      .catch(e => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  async function openPath(path: string) {
    setBusy(true);
    setError('');
    try {
      await api.openWorkspace(path);
      onWorkspaceChanged?.();
      navigate('/');
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function pickFolder(): Promise<string | null> {
    const path = await api.pickFolder();
    if (path === null) {
      // Native picker not available — fall back to text input
      setShowOpenByPath(true);
      setCreating(false);
    }
    return path;
  }

  async function handleOpenFolder() {
    setError('');
    try {
      const path = await pickFolder();
      if (path) await openPath(path);
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleCreate() {
    setError('');
    // If native picker is available, use it; otherwise fall back to typed path
    const pickedPath = await api.pickFolder().catch(() => null);
    const path = pickedPath ?? createPath.trim();

    if (!path) {
      // Native picker not available and no typed path yet — show path input
      setShowOpenByPath(false);
      return;
    }
    if (!pickedPath && !createPath.trim()) {
      setError('Enter a folder path.');
      return;
    }

    setBusy(true);
    try {
      await api.createWorkspace(path, newName.trim() || undefined);
      await api.openWorkspace(path);
      onWorkspaceChanged?.();
      navigate('/');
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      setCreating(false);
    }
  }

  const shortenPath = (p: string) => p.replace(/^\/Users\/[^/]+/, '~');
  const allUnique = all.filter(a => !recent.some(r => r.path === a.path));
  const allWorkspaces = [...recent, ...allUnique];

  return (
    <div className="workspace-page">
      <div className="workspace-header">
        <h1>Workspaces</h1>
      </div>

      {error && <div className="workspace-error">{error}</div>}
      {busy && <div className="workspace-loading">Switching workspace…</div>}

      {/* Workspace dropdown switcher */}
      <div className="ws-page-selector" ref={dropRef}>
        <button
          className={`ws-page-trigger${dropOpen ? ' is-open' : ''}`}
          onClick={() => !loading && !busy && setDropOpen(o => !o)}
          disabled={loading || busy}
        >
          <span className="ws-folder"><Icon.folder /></span>
          <span className="ws-meta">
            {loading ? (
              <span className="ws-name">Loading…</span>
            ) : current ? (
              <>
                <span className="ws-name">{current.name}</span>
                <span className="ws-path">{shortenPath(current.path)}</span>
              </>
            ) : (
              <span className="ws-name ws-page-none">No workspace open</span>
            )}
          </span>
          <svg className="ws-pg-caret" width="11" height="11" viewBox="0 0 12 12" fill="none"
            stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="3.5,5 6,2.5 8.5,5" /><polyline points="3.5,7 6,9.5 8.5,7" />
          </svg>
        </button>

        {dropOpen && (
          <div className="ws-page-menu" onClick={e => e.stopPropagation()}>
            <div className="ws-menu-label">Switch workspace</div>
            {allWorkspaces.length > 0 ? allWorkspaces.map(w => (
              <button
                key={w.path}
                className={`ws-menu-item${w.path === current?.path ? ' is-active' : ''}`}
                onClick={() => { openPath(w.path); setDropOpen(false); }}
                disabled={busy}
              >
                <span className="ws-folder sm"><Icon.folder /></span>
                <span className="ws-mi-meta">
                  <span className="ws-mi-name">{w.name}</span>
                  <span className="ws-mi-path">{shortenPath(w.path)}</span>
                </span>
                {w.path === current?.path && <span className="ws-mi-check"><Icon.check /></span>}
              </button>
            )) : (
              <div className="ws-menu-empty">No workspaces yet</div>
            )}
            <div className="ws-menu-divider" />
            <button className="ws-menu-action" onClick={() => { handleOpenFolder(); setDropOpen(false); }}>
              <Icon.folder /> Open folder…
            </button>
            <button className="ws-menu-action" onClick={() => { setCreating(true); setShowOpenByPath(false); setDropOpen(false); }}>
              <Icon.plus /> Create new workspace…
            </button>
          </div>
        )}
      </div>

      {/* Quick actions */}
      <div className="workspace-actions">
        <button className="btn" onClick={() => { handleOpenFolder(); setCreating(false); }} disabled={busy}>
          <Icon.folder /> Open Folder
        </button>
        <button className="btn" onClick={() => { setCreating(v => !v); setShowOpenByPath(false); }} disabled={busy}>
          <Icon.plus /> {creating ? 'Cancel' : 'Create Workspace'}
        </button>
      </div>

      {/* Open by path form (fallback when native picker unavailable) */}
      {showOpenByPath && !creating && (
        <div className="workspace-path-form">
          <label className="workspace-path-label">Open by path</label>
          <div className="workspace-path-row">
            <input
              className="input mono"
              type="text"
              placeholder="/Users/me/my-project"
              value={openByPath}
              onChange={e => setOpenByPath(e.target.value)}
              onKeyDown={e => e.key === 'Enter' && openPath(openByPath)}
              autoFocus
            />
            <button
              className="btn-primary btn-sm-primary"
              onClick={() => openPath(openByPath)}
              disabled={!openByPath.trim() || busy}
            >
              Open
            </button>
          </div>
          <p className="workspace-path-hint">
            Point to a folder containing an <code>api-docs/</code> directory.
          </p>
        </div>
      )}

      {/* Create form */}
      {creating && (
        <div className="workspace-create-form">
          <input
            className="input mono"
            type="text"
            placeholder="/Users/me/new-project (or click 'Pick folder')"
            value={createPath}
            onChange={e => setCreatePath(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && handleCreate()}
            autoFocus
          />
          <input
            className="input"
            type="text"
            placeholder="Workspace name (optional)"
            value={newName}
            onChange={e => setNewName(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && handleCreate()}
          />
          <button className="btn-primary btn-sm-primary" onClick={handleCreate} disabled={busy}>
            Pick folder and create
          </button>
        </div>
      )}

      {!loading && recent.length > 0 && (
        <section className="workspace-section">
          <h2><Icon.clock /> Recent</h2>
          <ul className="workspace-list">
            {recent.map(w => (
              <li key={w.path}>
                <button
                  className={`workspace-item${w.path === current?.path ? ' is-current' : ''}`}
                  onClick={() => openPath(w.path)}
                  disabled={busy}
                >
                  <span className="wl-icon"><Icon.folder /></span>
                  <span className="wl-info">
                    <span className="wl-name">{w.name}</span>
                    <span className="wl-path">{shortenPath(w.path)}</span>
                  </span>
                  {w.last_opened && (
                    <span className="wl-time">{new Date(w.last_opened).toLocaleDateString()}</span>
                  )}
                  {w.path === current?.path
                    ? <span className="wl-badge">Current</span>
                    : <span className="wl-open-action">Open →</span>
                  }
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}

      {!loading && allUnique.length > 0 && (
        <section className="workspace-section">
          <h2><Icon.folder /> All Workspaces</h2>
          <ul className="workspace-list">
            {allUnique.map(w => (
              <li key={w.path}>
                <button
                  className={`workspace-item${w.path === current?.path ? ' is-current' : ''}`}
                  onClick={() => openPath(w.path)}
                  disabled={busy}
                >
                  <span className="wl-icon"><Icon.folder /></span>
                  <span className="wl-info">
                    <span className="wl-name">{w.name}</span>
                    <span className="wl-path">{shortenPath(w.path)}</span>
                  </span>
                  {w.path === current?.path
                    ? <span className="wl-badge">Current</span>
                    : <span className="wl-open-action">Open →</span>
                  }
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}

      {!loading && recent.length === 0 && allUnique.length === 0 && (
        <div className="workspace-empty">
          No workspaces yet. Open a folder containing an <code>api-docs/</code> directory, or create a new one.
        </div>
      )}
    </div>
  );
}
