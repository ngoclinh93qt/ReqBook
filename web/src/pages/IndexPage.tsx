import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { api } from '../api';
import { getStoredRun, timeAgo } from '../hooks/useRunResults';
import type { IndexData } from '../types';
import { Icon, MethodBadge, PathStr } from '../ui';

const METHODS = ['ALL', 'GET', 'POST', 'PATCH', 'PUT', 'DELETE'];

export function IndexPage({ env, refreshKey }: { env: string; refreshKey: number }) {
  const [data, setData] = useState<IndexData | null>(null);
  const [error, setError] = useState('');
  const [query, setQuery] = useState('');
  const [methodFilter, setMethodFilter] = useState('ALL');
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  const [runTick, setRunTick] = useState(0);
  const navigate = useNavigate();
  const searchRef = useRef<HTMLInputElement | null>(null);

  const fetchIndex = useCallback(async () => {
    try {
      setData(await api.getIndex());
      setError('');
    } catch (e: unknown) {
      setError(String(e));
    }
  }, []);

  useEffect(() => { fetchIndex(); }, [fetchIndex, refreshKey]);

  // Re-read localStorage whenever an endpoint is run (from any page)
  useEffect(() => {
    const handler = () => setRunTick(t => t + 1);
    window.addEventListener('rqb:run-saved', handler);
    return () => window.removeEventListener('rqb:run-saved', handler);
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        searchRef.current?.focus();
      }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, []);

  const allSpecs = useMemo(() => (data?.groups ?? []).flatMap(g => g.specs), [data]);

  // Recompute when runTick changes (after any endpoint run)
  const stats = useMemo(() => {
    let passing = 0, failing = 0, never = 0;
    for (const spec of allSpecs) {
      const r = getStoredRun(spec.rel_path);
      if (!r) { never++; continue; }
      if (r.passed) passing++; else failing++;
    }
    return { total: allSpecs.length, passing, failing, never };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [allSpecs, runTick]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (data?.groups ?? []).map(group => ({
      ...group,
      specs: group.specs.filter(spec => {
        if (methodFilter !== 'ALL' && spec.method !== methodFilter) return false;
        if (!q) return true;
        return spec.path.toLowerCase().includes(q) || spec.title.toLowerCase().includes(q) || spec.method.toLowerCase().includes(q);
      }),
    })).filter(group => group.specs.length > 0);
  }, [data, methodFilter, query]);

  if (error) return <div className="index-wrap"><div className="empty fail-text">{error}</div></div>;
  if (!data) return <div className="index-wrap"><div className="empty">Loading endpoints…</div></div>;

  return (
    <div className="index-wrap">
      <header className="page-head">
        <h1 className="page-title">Endpoints</h1>
        <p className="page-sub">
          Every contract in <b>{data.project_name}</b> lives as a single markdown file. Click an endpoint to inspect, fill runtime inputs, and run it against <b>{env}</b>.
        </p>
      </header>

      <div className="stats">
        <div className="stat"><div className="lab">Total</div><div className="val">{stats.total}</div></div>
        <div className="stat"><div className="lab">Passing</div><div className="val ok-num">{stats.passing}{stats.passing > 0 && stats.total > 0 && <span className="delta ok">{Math.round(stats.passing / stats.total * 100)}%</span>}</div></div>
        <div className="stat"><div className="lab">Failing</div><div className="val fail-num">{stats.failing}</div></div>
        <div className="stat"><div className="lab">Not run</div><div className="val muted-num">{stats.never}</div></div>
      </div>

      <div className="searchbar">
        <span className="ic"><Icon.search /></span>
        <input ref={searchRef} value={query} onChange={e => setQuery(e.target.value)} placeholder="Search by path, title, or method…" />
        <span className="kbd">⌘K</span>
      </div>

      <div className="filter-row">
        <span className="filter-ic"><Icon.filter /></span>
        {METHODS.map(method => (
          <button key={method} className={`chip-filter ${methodFilter === method ? 'is-on' : ''}`} onClick={() => setMethodFilter(method)}>
            {method === 'ALL' ? 'All methods' : method}
          </button>
        ))}
      </div>

      {filtered.map(group => {
        const open = !collapsed[group.resource];
        const groupResults = group.specs.map(s => getStoredRun(s.rel_path));
        const ran = groupResults.filter(Boolean).length;
        const passing = groupResults.filter(r => r?.passed).length;
        const healthPct = group.specs.length > 0 ? (passing / group.specs.length * 100) : 0;

        return (
          <section key={group.resource} className="group">
            <header className="group-h" onClick={() => setCollapsed(prev => ({ ...prev, [group.resource]: open }))}>
              <span className="chev"><Icon.chev open={open} /></span>
              <span className="name">{group.resource}</span>
              <span className="count">{group.specs.length}</span>
              <span className="health">
                <span className="health-bar">
                  {ran > 0 && <span className="health-bar-fill" style={{ width: `${healthPct}%`, background: healthPct === 100 ? 'var(--ok)' : healthPct === 0 ? 'var(--fail)' : 'var(--warn, #f59e0b)' }} />}
                </span>
                {ran > 0 && <span className="health-label">{passing}/{group.specs.length}</span>}
              </span>
            </header>
            {open && (
              <div className="rows fade-in">
                {group.specs.map(spec => {
                  const result = getStoredRun(spec.rel_path);
                  const runStatus = result ? (result.passed ? 'ok' : 'fail') : 'never';
                  return (
                    <div key={spec.rel_path} className="row" onClick={() => navigate(`/spec/${spec.rel_path}`)}>
                      <MethodBadge method={spec.method} />
                      <div className="r-main">
                        <div className="r-path"><PathStr path={spec.path} /></div>
                        <div className="r-title">{spec.title}</div>
                      </div>
                      <div className="r-last">{result ? timeAgo(result.at) : 'Not run'}</div>
                      <div className={`r-status ${runStatus}`}>
                        <span className="dot" />
                        <span>{result?.status ?? '—'}</span>
                      </div>
                      <div className="row-go"><Icon.arr /></div>
                    </div>
                  );
                })}
              </div>
            )}
          </section>
        );
      })}

      {filtered.length === 0 && (
        <div className="empty">{query ? `No endpoints match "${query}".` : 'No endpoints yet. Sync the workspace or create a new request.'}</div>
      )}
    </div>
  );
}
