import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { api } from '../api';
import type { WorkspaceEntry } from '../types';
import { Icon } from '../ui';

const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function pickDirectory(): Promise<string | null> {
  if (!isTauri) return null;
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<string | null>('pick_directory');
}

export function WorkspacePage() {
  const navigate = useNavigate();
  const [current, setCurrent] = useState<WorkspaceEntry | null>(null);
  const [recent, setRecent] = useState<WorkspaceEntry[]>([]);
  const [all, setAll] = useState<WorkspaceEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState('');
  const [busy, setBusy] = useState(false);

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
      if (isTauri) {
        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('open_workspace', { path });
      }
      await api.openWorkspace(path);
      navigate('/');
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleOpenFolder() {
    setError('');
    if (!isTauri) {
      setError('Native folder picker requires the desktop app. Use "Open by path" below.');
      return;
    }
    try {
      const path = await pickDirectory();
      if (path) await openPath(path);
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleCreate() {
    setError('');
    if (!isTauri) {
      setError('Native folder picker requires the desktop app.');
      return;
    }
    try {
      const path = await pickDirectory();
      if (!path) return;
      setBusy(true);
      const { invoke } = await import('@tauri-apps/api/core');
      await api.createWorkspace(path, newName.trim() || undefined);
      await invoke('open_workspace', { path });
      navigate('/');
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      setCreating(false);
    }
  }

  const allUnique = all.filter(a => !recent.some(r => r.path === a.path));

  return (
    <div className="workspace-page">
      <div className="workspace-header">
        <h1>Workspaces</h1>
        {current && (
          <div className="workspace-current">
            <span className="wl-icon"><Icon.folder /></span>
            <span className="wl-name">{current.name}</span>
            <span className="wl-path">{current.path}</span>
          </div>
        )}
      </div>

      {error && <div className="workspace-error">{error}</div>}
      {(loading || busy) && <div className="workspace-loading">{loading ? 'Loading…' : 'Switching workspace…'}</div>}

      <div className="workspace-actions">
        <button className="btn-primary" onClick={handleOpenFolder} disabled={busy}>
          <Icon.folder /> Open Folder
        </button>
        <button className="btn-secondary" onClick={() => setCreating(v => !v)} disabled={busy}>
          <Icon.plus /> Create Workspace
        </button>
      </div>

      {creating && (
        <div className="workspace-create-form">
          <input
            type="text"
            placeholder="Workspace name (optional)"
            value={newName}
            onChange={e => setNewName(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && handleCreate()}
            className="workspace-name-input"
          />
          <button className="btn-primary" onClick={handleCreate} disabled={busy}>
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
                <button className="workspace-item" onClick={() => openPath(w.path)} disabled={busy}>
                  <span className="wl-icon"><Icon.folder /></span>
                  <span className="wl-info">
                    <span className="wl-name">{w.name}</span>
                    <span className="wl-path">{w.path}</span>
                  </span>
                  {w.last_opened && (
                    <span className="wl-time">{new Date(w.last_opened).toLocaleDateString()}</span>
                  )}
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
                <button className="workspace-item" onClick={() => openPath(w.path)} disabled={busy}>
                  <span className="wl-icon"><Icon.folder /></span>
                  <span className="wl-info">
                    <span className="wl-name">{w.name}</span>
                    <span className="wl-path">{w.path}</span>
                  </span>
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
