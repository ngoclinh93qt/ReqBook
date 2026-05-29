import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { api } from '../api';
import type { FlowEntry } from '../types';
import { Icon } from '../ui';

export function FlowsPage() {
  const [flows, setFlows] = useState<FlowEntry[]>([]);
  const [error, setError] = useState('');
  const navigate = useNavigate();

  useEffect(() => {
    api.getFlows()
      .then(data => setFlows(data.flows))
      .catch(e => setError(String(e)));
  }, []);

  return (
    <div className="index-wrap">
      <header className="page-head flow-head">
        <div>
          <h1 className="page-title">Flows</h1>
          <p className="page-sub">
            Chain endpoints together, pass tokens and IDs from one response into the next, then run the whole path from a canvas.
          </p>
        </div>
      </header>

      {error && <div className="empty fail-text">{error}</div>}
      {!error && (
        <div className="flows-grid">
          {flows.map(flow => (
            <button className="flow-card" key={flow.rel_path} onClick={() => navigate(`/flows/${flow.rel_path}`)}>
              <div className="fc-h">
                <h3>{flow.title}</h3>
                <span className="chip"><span className="dot" />Saved</span>
              </div>
              <p className="fc-desc">{flow.rel_path}</p>
              <FlowMini count={flow.steps} />
              <div className="fc-foot">
                <span><b>{flow.steps}</b> blocks</span>
                <span className="sep">.</span>
                <span>Markdown pipeline</span>
                <span style={{ marginLeft: 'auto', color: 'var(--fg-4)' }}>{flow.name}</span>
              </div>
            </button>
          ))}

          <button className="flow-card empty-card" onClick={() => navigate('/flows/new')}>
            <div className="empty-mark"><Icon.plus /></div>
            <div className="empty-title">New flow</div>
            <div className="empty-copy">Start blank, add endpoint blocks, then bind outputs to downstream inputs.</div>
          </button>
        </div>
      )}
    </div>
  );
}

function FlowMini({ count }: { count: number }) {
  const blocks = Math.max(1, Math.min(count, 5));
  return (
    <svg viewBox="0 0 240 56" className="fc-mini" preserveAspectRatio="xMidYMid meet" aria-hidden="true">
      <line x1="20" y1="28" x2="220" y2="28" stroke="var(--border-2)" strokeWidth="1" strokeDasharray="3 4" />
      {Array.from({ length: blocks }).map((_, index) => {
        const x = 20 + index * (200 / Math.max(1, blocks - 1));
        return (
          <g key={index}>
            <rect x={x - 12} y="18" width="24" height="20" rx="3" fill="var(--bg)" stroke="var(--border-2)" />
            <circle cx={x} cy="28" r="2" fill="var(--accent)" />
          </g>
        );
      })}
    </svg>
  );
}
