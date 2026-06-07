import { MouseEvent, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { api } from '../api';
import type { ExecResult, FlowCapture, FlowRunResult, IndexData, SpecEntry } from '../types';
import { Icon, MethodBadge, PathStr } from '../ui';

const NODE_W = 240;
const NODE_H = 100; // approximate node height for drop hit-testing
const DEFAULT_OUTPUT = {
  status: 200,
  body: {
    id: 'id_123',
    token: 'tok_sample',
    user: { id: 'usr_123', email: 'ada@example.com' },
  },
};

type InputValue =
  | { kind: 'literal'; value: string }
  | { kind: 'ref'; from: string; path: string };

type FlowNode = {
  id: string;
  relPath: string;
  label: string;
  position: { x: number; y: number };
  inputs: Record<string, InputValue>;
  captures: FlowCapture[];
  status: 'idle' | 'running' | 'ok' | 'fail';
  result: unknown;
  ms: number | null;
  runMode: 'awaited' | 'detached';
};

type FlowEdge = { id: string; from: string; to: string };

export function FlowCanvasPage() {
  const { '*': relPath = 'new' } = useParams();
  const navigate = useNavigate();
  const [index, setIndex] = useState<IndexData | null>(null);
  const [flowName, setFlowName] = useState(relPath === 'new' ? 'new-flow' : relPath.replace(/^(flows|pipelines)\//, '').replace(/\.md$/, ''));
  const [title, setTitle] = useState(relPath === 'new' ? 'New flow' : 'Flow');
  const [nodes, setNodes] = useState<FlowNode[]>([newNode(0)]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [isRunning, setIsRunning] = useState(false);
  const [activeEdges, setActiveEdges] = useState<Set<string>>(new Set());
  const [result, setResult] = useState<FlowRunResult | null>(null);
  const [msg, setMsg] = useState('');
  const [pan, setPan] = useState({ x: 24, y: 24 });
  const [zoom, setZoom] = useState(1);
  const canvasRef = useRef<HTMLDivElement>(null);
  const [autoSaveState, setAutoSaveState] = useState<'idle' | 'pending' | 'saved'>('idle');
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isLoadingRef = useRef(false);
  const [dragEdge, setDragEdge] = useState<{
    fromNodeId: string;
    mouseX: number;
    mouseY: number;
    targetNodeId: string | null;
    reconnectTargetId?: string; // for input-port drag: the original "to" node being detached
  } | null>(null);

  useEffect(() => {
    api.getIndex().then(setIndex).catch(e => setMsg(String(e)));
  }, []);

  useEffect(() => {
    if (relPath === 'new') return;
    isLoadingRef.current = true;
    api.getFlow(relPath).then(flow => {
      setFlowName(flow.name);
      setTitle(flow.title);
      setNodes(flow.steps.length > 0 ? flowStepsToNodes(flow.steps) : [newNode(0)]);
      setSelectedId(null);
    }).catch(e => setMsg(String(e)))
    .finally(() => { setTimeout(() => { isLoadingRef.current = false; }, 80); });
  }, [relPath]);

  const endpoints = useMemo(() => {
    const out: SpecEntry[] = [];
    for (const group of index?.groups ?? []) out.push(...group.specs);
    return out;
  }, [index]);
  const endpointByPath = useMemo(() => new Map(endpoints.map(endpoint => [endpoint.rel_path, endpoint])), [endpoints]);
  const edges = useMemo(() => deriveEdges(nodes), [nodes]);
  const effectiveEdges = useMemo(() => edges.filter(edge => nodes.find(node => node.id === edge.from)?.runMode !== 'detached'), [edges, nodes]);
  const waves = useMemo(() => topoWaves(nodes, effectiveEdges), [nodes, effectiveEdges]);
  const nodeWave = useMemo(() => {
    const out = new Map<string, number>();
    waves.forEach((wave, index) => wave.forEach(id => out.set(id, index)));
    return out;
  }, [waves]);
  const selected = nodes.find(node => node.id === selectedId) ?? null;
  const savePath = `flows/${slug(flowName)}.md`;
  const markdown = renderFlowMarkdown(flowName, title, nodes);

  // Autosave on every edit (debounced 1.5 s) — skipped for unsaved new flows
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (isLoadingRef.current || relPath === 'new') return;
    setAutoSaveState('pending');
    if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
    saveTimerRef.current = setTimeout(async () => {
      try {
        await api.saveFlow(savePath, markdown);
        setAutoSaveState('saved');
        setTimeout(() => setAutoSaveState('idle'), 2000);
      } catch {
        setAutoSaveState('idle');
      }
    }, 1500);
    return () => { if (saveTimerRef.current) clearTimeout(saveTimerRef.current); };
  }, [markdown]);

  const totalMs = nodes.reduce((sum, node) => sum + (node.ms ?? 0), 0);
  const allDone = !isRunning && nodes.some(node => node.status !== 'idle') && nodes.every(node => node.status === 'ok' || node.status === 'fail');
  const anyFail = nodes.some(node => node.status === 'fail');
  const maxParallel = waves.reduce((max, wave) => Math.max(max, wave.length), 0);

  async function save() {
    setMsg('');
    try {
      await api.saveFlow(savePath, markdown);
      setMsg(`Saved ${savePath}`);
      if (relPath === 'new') navigate(`/flows/${savePath}`);
      return true;
    } catch (e) {
      setMsg(String(e));
      return false;
    }
  }

  async function runFlow() {
    if (isRunning || !await save()) return;
    setIsRunning(true);
    setResult(null);
    setNodes(items => items.map(node => ({ ...node, status: 'idle', result: null, ms: null })));
    setActiveEdges(new Set());
    try {
      const data = await api.runFlow(savePath);
      setResult(data);
      setNodes(items => items.map((node, index) => {
        const step = data.steps[index];
        const execution = step?.execution as ExecResult | undefined;
        const bodyText = execution?.response?.body;
        const parsedBody = parseJson(bodyText);
        const status = step?.error || execution?.diff?.passed === false ? 'fail' : 'ok';
        return {
          ...node,
          status,
          result: execution?.response ? { status: execution.response.status, body: parsedBody ?? bodyText } : { error: step?.error },
          ms: execution?.duration_ms ?? null,
        };
      }));
      setActiveEdges(new Set(edges.map(edge => edge.id)));
    } catch (e) {
      setMsg(String(e));
    } finally {
      setIsRunning(false);
    }
  }

  function stopFlow() {
    setIsRunning(false);
    setActiveEdges(new Set());
  }

  function resetFlow() {
    setResult(null);
    setActiveEdges(new Set());
    setNodes(items => items.map(node => ({ ...node, status: 'idle', result: null, ms: null })));
  }

  // ── Edge connection helpers ──────────────────────────────────────────────

  /** Add a ref input on toId pointing to fromId (no-op if already connected). */
  function connectNodes(fromId: string, toId: string) {
    setNodes(items => items.map(node => {
      if (node.id !== toId) return node;
      if (Object.values(node.inputs).some(v => v.kind === 'ref' && v.from === fromId)) return node;
      const base = `from_${fromId}`;
      const key = node.inputs[base] ? `${base}_2` : base;
      return { ...node, inputs: { ...node.inputs, [key]: { kind: 'ref', from: fromId, path: 'body.id' } as InputValue } };
    }));
  }

  /** Move a ref from (fromId → oldToId) to (fromId → newToId). */
  function reconnectEdge(fromId: string, oldToId: string, newToId: string) {
    setNodes(items => {
      const removed = items.map(node => {
        if (node.id !== oldToId) return node;
        const inputs = Object.fromEntries(
          Object.entries(node.inputs).filter(([, v]) => !(v.kind === 'ref' && v.from === fromId))
        );
        return { ...node, inputs };
      });
      return removed.map(node => {
        if (node.id !== newToId) return node;
        if (Object.values(node.inputs).some(v => v.kind === 'ref' && v.from === fromId)) return node;
        const base = `from_${fromId}`;
        const key = node.inputs[base] ? `${base}_2` : base;
        return { ...node, inputs: { ...node.inputs, [key]: { kind: 'ref', from: fromId, path: 'body.id' } as InputValue } };
      });
    });
  }

  /** Drag from a node's output port to create a new connection. */
  function startOutputPortDrag(e: MouseEvent<HTMLElement>, nodeId: string) {
    e.stopPropagation();
    const rect = canvasRef.current?.getBoundingClientRect() ?? { left: 0, top: 0 };
    const cp = pan, cz = zoom, cn = nodes;
    const cvt = (sx: number, sy: number) => ({ x: (sx - rect.left - cp.x) / cz, y: (sy - rect.top - cp.y) / cz });
    const start = cvt(e.clientX, e.clientY);
    setDragEdge({ fromNodeId: nodeId, mouseX: start.x, mouseY: start.y, targetNodeId: null });
    const onMove = (ev: globalThis.MouseEvent) => {
      const { x, y } = cvt(ev.clientX, ev.clientY);
      const t = cn.find(n =>
        n.id !== nodeId &&
        x >= n.position.x - 8 && x <= n.position.x + NODE_W + 8 &&
        y >= n.position.y - 8 && y <= n.position.y + NODE_H + 8
      ) ?? null;
      setDragEdge(prev => prev ? { ...prev, mouseX: x, mouseY: y, targetNodeId: t?.id ?? null } : null);
    };
    const onUp = () => {
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
      setDragEdge(prev => {
        if (prev?.targetNodeId) connectNodes(prev.fromNodeId, prev.targetNodeId);
        return null;
      });
    };
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
  }

  /**
   * Shared drag logic for reconnecting an existing edge.
   * The ghost edge goes FROM fromNodeId TO cursor.
   * On drop at a new node: moves the edge's target from reconnectTargetId → new node.
   */
  function startEdgeReconnectDrag(e: MouseEvent<HTMLElement | HTMLDivElement>, fromNodeId: string, reconnectTargetId: string) {
    e.stopPropagation();
    const rect = canvasRef.current?.getBoundingClientRect() ?? { left: 0, top: 0 };
    const cp = pan, cz = zoom, cn = nodes;
    const cvt = (sx: number, sy: number) => ({ x: (sx - rect.left - cp.x) / cz, y: (sy - rect.top - cp.y) / cz });
    const start = cvt(e.clientX, e.clientY);
    setDragEdge({ fromNodeId, mouseX: start.x, mouseY: start.y, targetNodeId: null, reconnectTargetId });
    const onMove = (ev: globalThis.MouseEvent) => {
      const { x, y } = cvt(ev.clientX, ev.clientY);
      const t = cn.find(n =>
        n.id !== fromNodeId &&
        x >= n.position.x - 8 && x <= n.position.x + NODE_W + 8 &&
        y >= n.position.y - 8 && y <= n.position.y + NODE_H + 8
      ) ?? null;
      setDragEdge(prev => prev ? { ...prev, mouseX: x, mouseY: y, targetNodeId: t?.id ?? null } : null);
    };
    const onUp = () => {
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
      setDragEdge(prev => {
        if (prev?.targetNodeId && prev.reconnectTargetId && prev.targetNodeId !== prev.reconnectTargetId) {
          reconnectEdge(prev.fromNodeId, prev.reconnectTargetId, prev.targetNodeId);
        }
        return null;
      });
    };
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
  }

  /**
   * Drag from a node's input port: picks the first incoming edge.
   * Useful when there's only 1 incoming connection.
   * For nodes with multiple edges, use the per-edge handles instead.
   */
  function startInputPortDrag(e: MouseEvent<HTMLElement>, nodeId: string) {
    const incoming = edges.filter(edge => edge.to === nodeId);
    if (incoming.length === 0) return; // no edge to move — let event bubble
    startEdgeReconnectDrag(e, incoming[0].from, nodeId);
  }

  /** Drag from the mid-point handle of a specific edge to reconnect it individually. */
  function startEdgeDrag(e: MouseEvent<HTMLDivElement>, edge: FlowEdge) {
    startEdgeReconnectDrag(e as unknown as MouseEvent<HTMLElement>, edge.from, edge.to);
  }

  // ────────────────────────────────────────────────────────────────────────

  function addBlock() {
    setNodes(items => {
      const node = newNode(items.length);
      setSelectedId(node.id);
      return [...items, node];
    });
  }

  function tidyLayout() {
    const colW = 320;
    const rowH = 180;
    setNodes(items => {
      const next = items.map(item => ({ ...item }));
      waves.forEach((wave, col) => {
        wave.forEach((id, row) => {
          const node = next.find(item => item.id === id);
          if (node) node.position = { x: 60 + col * colW, y: 160 + row * rowH - ((wave.length - 1) * rowH) / 2 };
        });
      });
      return next;
    });
  }

  function updateNode(id: string, patch: Partial<FlowNode>) {
    setNodes(items => items.map(item => item.id === id ? { ...item, ...patch } : item));
  }

  function removeNode(id: string) {
    setNodes(items => items.length <= 1 ? items : items.filter(item => item.id !== id));
    setSelectedId(null);
  }

  function startPan(e: MouseEvent<HTMLDivElement>) {
    if (dragEdge) return; // don't pan while drawing a connection
    if ((e.target as HTMLElement).closest('.flow-node, .canvas-overlay, .inspector')) return;
    const start = { x: e.clientX, y: e.clientY };
    const initial = pan;
    const onMove = (ev: globalThis.MouseEvent) => setPan({ x: initial.x + ev.clientX - start.x, y: initial.y + ev.clientY - start.y });
    const onUp = () => {
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
    };
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
    setSelectedId(null);
  }

  function startDragNode(e: MouseEvent<HTMLElement>, nodeId: string) {
    if (dragEdge) return; // don't drag node while drawing a connection
    e.stopPropagation();
    setSelectedId(nodeId);
    const node = nodes.find(item => item.id === nodeId);
    if (!node) return;
    const start = { x: e.clientX, y: e.clientY };
    const initial = node.position;
    const onMove = (ev: globalThis.MouseEvent) => {
      const dx = (ev.clientX - start.x) / zoom;
      const dy = (ev.clientY - start.y) / zoom;
      updateNode(nodeId, { position: { x: initial.x + dx, y: initial.y + dy } });
    };
    const onUp = () => {
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
    };
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
  }

  function onWheel(e: React.WheelEvent<HTMLDivElement>) {
    if (!e.ctrlKey && !e.metaKey && e.deltaY === 0) return;
    e.preventDefault();
    setZoom(value => Math.max(0.4, Math.min(1.6, value - e.deltaY * 0.0015)));
  }

  return (
    <div className="flow-shell" data-testid="flow-canvas-page">
      <div className="flow-toolbar">
        <button className="btn ghost sm" onClick={() => navigate('/flows')}>
          <BackIcon /> All flows
        </button>
        <span className="flow-slash">/</span>
        <span className="flow-tb-crumb">{title || flowName}</span>
        <span className="chip flow-shape-chip" title="Execution shape derived from dependencies">
          <span className="chain-glyph">chain</span>
          <span><b>{waves.length}</b> wave{waves.length !== 1 ? 's' : ''} · max <b>{maxParallel}</b> in parallel</span>
        </span>
        <div className="flow-toolbar-actions">
          {msg && <span className={`flow-inline-msg ${msg.startsWith('Saved') ? 'ok-text' : 'fail-text'}`} data-testid="flow-message">{msg}</span>}
          {allDone && <span className={`chip ${anyFail ? 'fail' : 'ok'}`} data-testid="flow-run-summary"><span className="dot" />{anyFail ? 'Failed' : 'Passed'} · {totalMs}ms</span>}
          {isRunning && <span className="chip running-chip" data-testid="flow-running"><span className="pulse-dot tiny" />Running</span>}
          <button className="btn sm" onClick={resetFlow} disabled={isRunning} data-testid="flow-reset">Reset</button>
          <button className="btn sm" onClick={tidyLayout} disabled={isRunning} data-testid="flow-auto-layout">Auto-layout</button>
          <button className="btn sm" onClick={save} data-testid="flow-save">Save</button>
          {!isRunning ? (
            <button className="btn-primary btn-sm-primary" onClick={runFlow} data-testid="flow-run"><Icon.play /> Run flow</button>
          ) : (
            <button className="btn-primary btn-sm-primary stop-btn" onClick={stopFlow} data-testid="flow-stop"><span className="stop-square" /> Stop</button>
          )}
        </div>
      </div>

      <div className="flow-doc-header">
        <input
          className="flow-doc-title"
          value={title}
          onChange={e => setTitle(e.target.value)}
          placeholder="Flow title"
          spellCheck={false}
          data-testid="flow-title-input"
        />
        <div className="flow-doc-meta">
          <span className="flow-doc-prefix">flows/</span>
          <input
            className="flow-doc-slug"
            value={flowName}
            onChange={e => setFlowName(e.target.value)}
            placeholder="flow-name"
            spellCheck={false}
            data-testid="flow-slug-input"
          />
          <span className="flow-doc-prefix">.md</span>
          {autoSaveState === 'pending' && <span className="autosave-pill">Saving…</span>}
          {autoSaveState === 'saved' && <span className="autosave-pill is-saved"><Icon.check /> Saved</span>}
          {relPath === 'new' && <span className="autosave-pill is-new">Not saved — click Save to create</span>}
        </div>
      </div>

      <div className={`flow-main ${selected ? 'has-inspector' : ''}`} data-testid="flow-main">
        <div className="flow-canvas" ref={canvasRef} onMouseDown={startPan} onWheel={onWheel} data-testid="flow-canvas">
          <div className="flow-canvas-pan" style={{ transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})` }}>
            <FlowEdges nodes={nodes} edges={edges} active={activeEdges} dragEdge={dragEdge} />
            {nodes.map(node => (
              <FlowNodeCard
                key={node.id}
                node={node}
                wave={nodeWave.get(node.id)}
                spec={endpointByPath.get(node.relPath)}
                isSelected={selectedId === node.id}
                isDragTarget={dragEdge?.targetNodeId === node.id}
                onSelect={() => { if (!dragEdge) setSelectedId(node.id); }}
                onDrag={event => startDragNode(event, node.id)}
                onDragOutputPort={e => startOutputPortDrag(e, node.id)}
                onDragInputPort={e => startInputPortDrag(e, node.id)}
              />
            ))}
            {/* Per-edge reconnect handles — one dot per edge at ~75% along the path */}
            {edges.map(edge => {
              const from = nodes.find(n => n.id === edge.from);
              const to = nodes.find(n => n.id === edge.to);
              if (!from || !to) return null;
              const x1 = from.position.x + NODE_W;
              const y1 = from.position.y + 22;
              const x2 = to.position.x;
              const y2 = to.position.y + 22;
              // ~75% along a straight-line approximation keeps the dot in open space
              const hx = x1 + (x2 - x1) * 0.72;
              const hy = y1 + (y2 - y1) * 0.72;
              const isBeingDragged = dragEdge?.fromNodeId === edge.from && dragEdge.reconnectTargetId === edge.to;
              return (
                <div
                  key={`eh-${edge.id}`}
                  className={`edge-handle ${isBeingDragged ? 'is-dragging' : ''}`}
                  style={{ left: hx - 6, top: hy - 6 }}
                  onMouseDown={e => startEdgeDrag(e, edge)}
                  title="Drag to reconnect this edge"
                />
              );
            })}
          </div>
          <div className="canvas-overlay tl"><Legend /></div>
          <div className="canvas-overlay bl"><button className="btn sm" onClick={addBlock} data-testid="flow-add-block"><Icon.plus /> Add block</button></div>
          <div className="canvas-overlay br"><ZoomBar zoom={zoom} setZoom={setZoom} /></div>
        </div>

        {selected && (
          <NodeInspector
            node={selected}
            nodes={nodes}
            edges={edges}
            waves={waves}
            nodeWave={nodeWave}
            endpoints={endpoints}
            result={result}
            updateNode={patch => updateNode(selected.id, patch)}
            updateAllNodes={setNodes}
            removeNode={() => removeNode(selected.id)}
            onClose={() => setSelectedId(null)}
          />
        )}
      </div>
    </div>
  );
}

function FlowNodeCard({ node, wave, spec, isSelected, isDragTarget, onSelect, onDrag, onDragOutputPort, onDragInputPort }: {
  node: FlowNode;
  wave?: number;
  spec?: SpecEntry;
  isSelected: boolean;
  isDragTarget: boolean;
  onSelect: () => void;
  onDrag: (e: MouseEvent<HTMLElement>) => void;
  onDragOutputPort: (e: MouseEvent<HTMLElement>) => void;
  onDragInputPort: (e: MouseEvent<HTMLElement>) => void;
}) {
  const method = spec?.method || 'GET';
  const path = spec?.path || node.relPath || 'Select endpoint';
  const detached = node.runMode === 'detached';
  return (
    <div
      className={`flow-node status-${node.status} ${isSelected ? 'is-selected' : ''} ${detached ? 'is-detached' : ''} ${isDragTarget ? 'is-drop-target' : ''}`}
      style={{ left: node.position.x, top: node.position.y, width: NODE_W }}
      onClick={event => { event.stopPropagation(); onSelect(); }}
      data-testid="flow-node"
      data-node-status={node.status}
      data-node-path={node.relPath}
      data-node-label={node.label}
    >
      <div className="fn-grip" onMouseDown={onDrag}>
        <span className="fn-handle in" onMouseDown={e => { e.stopPropagation(); onDragInputPort(e); }} title="Drag to reconnect incoming edge" />
        <MethodBadge method={method} />
        {wave != null && <span className="fn-wave" title={`Wave ${wave + 1}`}>w{wave + 1}</span>}
        <span className="fn-status"><NodeStatusIndicator status={node.status} /></span>
        <span className="fn-handle out" onMouseDown={e => { e.stopPropagation(); onDragOutputPort(e); }} title="Drag to connect to another block" />
      </div>
      <div className="fn-body">
        <div className="fn-label">{node.label || 'Untitled block'}</div>
        <div className="fn-path"><PathStr path={path} /></div>
      </div>
      <div className="fn-foot">
        <span className="ic"><BoltIcon /></span>
        <span>{Object.keys(node.inputs).length} inputs</span>
        {node.captures.length > 0 && <span>{node.captures.length} captures</span>}
        {detached && <span className="fn-pill">detached</span>}
        <span style={{ marginLeft: 'auto', color: 'var(--fg-4)' }}>{node.ms ? `${node.ms}ms` : node.status === 'running' ? '...' : '-'}</span>
      </div>
    </div>
  );
}

function NodeStatusIndicator({ status }: { status: FlowNode['status'] }) {
  if (status === 'running') return <span className="pulse-dot" />;
  if (status === 'ok') return <span className="ok-text"><Icon.check /></span>;
  if (status === 'fail') return <span className="fail-text"><Icon.cross /></span>;
  return <span className="idle-dot" />;
}

function FlowEdges({ nodes, edges, active, dragEdge }: {
  nodes: FlowNode[];
  edges: FlowEdge[];
  active: Set<string>;
  dragEdge: { fromNodeId: string; mouseX: number; mouseY: number; targetNodeId: string | null } | null;
}) {
  const nodeMap = Object.fromEntries(nodes.map(node => [node.id, node]));
  return (
    <svg className="flow-edges-svg" width="4000" height="3000" aria-hidden="true">
      {edges.map(edge => {
        const from = nodeMap[edge.from];
        const to = nodeMap[edge.to];
        if (!from || !to) return null;
        const x1 = from.position.x + NODE_W;
        const y1 = from.position.y + 22;
        const x2 = to.position.x;
        const y2 = to.position.y + 22;
        const dx = Math.max(60, Math.abs(x2 - x1) * 0.45);
        const d = `M ${x1} ${y1} C ${x1 + dx} ${y1}, ${x2 - dx} ${y2}, ${x2} ${y2}`;
        const isActive = active.has(edge.id);
        const isDone = to.status === 'ok' || to.status === 'fail';
        const stroke = isActive ? 'var(--accent)' : isDone ? (to.status === 'fail' ? 'var(--fail)' : 'var(--ok)') : 'var(--border-2)';
        return (
          <g key={edge.id} className={`flow-edge-path ${isActive ? 'is-active' : ''}`}>
            <path d={d} fill="none" stroke={stroke} strokeWidth="1.5" />
            {isActive && <path d={d} fill="none" stroke="var(--accent)" strokeWidth="2" strokeDasharray="6 8" className="edge-anim" />}
          </g>
        );
      })}

      {/* Ghost edge drawn while user drags a connection */}
      {dragEdge && (() => {
        const fromNode = nodeMap[dragEdge.fromNodeId];
        if (!fromNode) return null;
        const x1 = fromNode.position.x + NODE_W;
        const y1 = fromNode.position.y + 22;
        const x2 = dragEdge.mouseX;
        const y2 = dragEdge.mouseY;
        const dx = Math.max(60, Math.abs(x2 - x1) * 0.45);
        const d = `M ${x1} ${y1} C ${x1 + dx} ${y1}, ${x2 - dx} ${y2}, ${x2} ${y2}`;
        const snapped = dragEdge.targetNodeId != null;
        return (
          <g className="flow-edge-ghost">
            <path d={d} fill="none" stroke="var(--accent)" strokeWidth="1.5" strokeDasharray="5 4" opacity={0.8} />
            <circle
              cx={x2} cy={y2} r={snapped ? 6 : 4}
              fill={snapped ? 'var(--accent)' : 'var(--bg-2)'}
              stroke="var(--accent)" strokeWidth="1.5"
            />
          </g>
        );
      })()}
    </svg>
  );
}

function NodeInspector({ node, nodes, edges, waves, nodeWave, endpoints, result, updateNode, updateAllNodes, removeNode, onClose }: {
  node: FlowNode;
  nodes: FlowNode[];
  edges: FlowEdge[];
  waves: string[][];
  nodeWave: Map<string, number>;
  endpoints: SpecEntry[];
  result: FlowRunResult | null;
  updateNode: (patch: Partial<FlowNode>) => void;
  updateAllNodes: React.Dispatch<React.SetStateAction<FlowNode[]>>;
  removeNode: () => void;
  onClose: () => void;
}) {
  const spec = endpoints.find(endpoint => endpoint.rel_path === node.relPath);
  const directUp = edges.filter(edge => edge.to === node.id).map(edge => edge.from);
  const directDown = edges.filter(edge => edge.from === node.id).map(edge => edge.to);
  const wave = nodeWave.get(node.id);
  const parallelWith = wave != null ? (waves[wave] ?? []).filter(id => id !== node.id) : [];
  const upstreamIds = useMemo(() => {
    const out = new Set<string>();
    const visit = (id: string) => edges.filter(edge => edge.to === id).forEach(edge => {
      if (!out.has(edge.from)) {
        out.add(edge.from);
        visit(edge.from);
      }
    });
    visit(node.id);
    return [...out];
  }, [edges, node.id]);
  const labelFor = (id: string) => nodes.find(item => item.id === id)?.label || id;
  const runStep = result?.steps.find(step => step.endpoint === node.relPath);

  function selectEndpoint(endpoint: SpecEntry) {
    const inputs = mergeEndpointInputs(node.inputs, endpoint, upstreamIds);
    updateNode({
      relPath: endpoint.rel_path,
      label: endpoint.title || node.label,
      inputs,
    });
    for (const [name, value] of Object.entries(inputs)) {
      if (value.kind === 'ref') updateAllNodes(items => ensureCapture(items, value.from, toResponsePath(value.path), name));
    }
  }

  function updateInput(name: string, patch: Partial<InputValue>) {
    const next = { ...node.inputs, [name]: { ...node.inputs[name], ...patch } as InputValue };
    updateNode({ inputs: next });
    const value = next[name];
    if (value.kind === 'ref') {
      updateAllNodes(items => ensureCapture(items, value.from, toResponsePath(value.path), name));
    }
  }

  function addInput() {
    const key = `input_${Object.keys(node.inputs).length + 1}`;
    const value = defaultInputValue(key, upstreamIds);
    updateNode({ inputs: { ...node.inputs, [key]: value } });
    if (value.kind === 'ref') updateAllNodes(items => ensureCapture(items, value.from, toResponsePath(value.path), key));
  }

  function deleteInput(name: string) {
    const next = { ...node.inputs };
    delete next[name];
    updateNode({ inputs: next });
  }

  function renameInput(oldName: string, newName: string) {
    const clean = slugVar(newName);
    if (!clean || clean === oldName || node.inputs[clean]) return;
    const next: Record<string, InputValue> = {};
    for (const [key, value] of Object.entries(node.inputs)) next[key === oldName ? clean : key] = value;
    updateNode({ inputs: next });
  }

  return (
    <aside className="inspector" data-testid="flow-inspector">
      <div className="inspector-h">
        <MethodBadge method={spec?.method || 'GET'} />
        <div style={{ flex: 1, minWidth: 0 }}>
          <input className="input flush inspector-title-input" value={node.label} onChange={e => updateNode({ label: e.target.value })} />
          <div className="ip-path"><PathStr path={spec?.path || node.relPath || 'Select endpoint'} /></div>
        </div>
        <button className="btn icon" onClick={onClose} title="Close"><Icon.x /></button>
      </div>

      <div className="inspector-tabs">
        <button className="tab is-on">Inputs <span className="badge">{Object.keys(node.inputs).length}</span></button>
        <button className="tab">Output {node.result ? <span className="badge">{node.status}</span> : null}</button>
        <button className="tab">Tests</button>
      </div>

      <div className="inspector-body">
        <div className="ip-section">
          <div className="sec-h"><span>Endpoint</span><span className="tag">{node.relPath || 'empty'}</span></div>
          <EndpointPicker
            endpoints={endpoints}
            selected={node.relPath}
            onPick={selectEndpoint}
          />
        </div>

        <div className="ip-section">
          <div className="sec-h"><span>Execution</span><span className="tag">wave {wave != null ? wave + 1 : '-'}</span></div>
          <div className="exec-mode">
            <button className={node.runMode !== 'detached' ? 'is-on' : ''} onClick={() => updateNode({ runMode: 'awaited' })}>
              <span className="em-glyph awaited">&gt;</span>
              <div className="em-text"><b>Awaited</b><span>Downstream blocks wait for this</span></div>
            </button>
            <button className={node.runMode === 'detached' ? 'is-on' : ''} onClick={() => updateNode({ runMode: 'detached' })}>
              <span className="em-glyph detached">~</span>
              <div className="em-text"><b>Detached</b><span>Fire and forget</span></div>
            </button>
          </div>
          <div className="dep-grid">
            <DepRow label="Depends on" ids={directUp} labelFor={labelFor} empty="nothing, runs first" />
            <DepRow label="Parallel with" ids={parallelWith} labelFor={labelFor} empty="nothing, runs alone" />
            <DepRow label="Blocks" ids={directDown} labelFor={labelFor} empty="nothing, leaf block" />
          </div>
        </div>

        <div className="ip-section">
          <div className="sec-h"><span>Inputs</span><span className="tag">{Object.keys(node.inputs).length} bound</span></div>
          {Object.entries(node.inputs).map(([name, value]) => (
            <InputRow
              key={name}
              name={name}
              value={value}
              upstreamIds={upstreamIds}
              nodes={nodes}
              onRename={next => renameInput(name, next)}
              onChange={patch => updateInput(name, patch)}
              onDelete={() => deleteInput(name)}
            />
          ))}
          <button className="add-row" style={{ marginTop: 10 }} onClick={addInput}><Icon.plus /> Add input</button>
        </div>

        <div className="ip-section">
          <div className="sec-h"><span>Captures</span><span className="tag">{node.captures.length} values</span></div>
          {node.captures.map((capture, index) => (
            <div className="input-row capture-row" key={`${capture.name}-${index}`}>
              <input className="input flush mono" value={capture.name} onChange={e => updateNode({ captures: node.captures.map((item, i) => i === index ? { ...item, name: e.target.value } : item) })} />
              <input className="input flush mono" value={capture.source} onChange={e => updateNode({ captures: node.captures.map((item, i) => i === index ? { ...item, source: e.target.value } : item) })} />
              <button className="row-del always" onClick={() => updateNode({ captures: node.captures.filter((_, i) => i !== index) })}><Icon.x /></button>
            </div>
          ))}
          <button className="add-row" style={{ marginTop: 10 }} onClick={() => updateNode({ captures: [...node.captures, { name: `value${node.captures.length + 1}`, source: 'response.body.id' }] })}><Icon.plus /> Capture output</button>
        </div>

        {node.result != null && (
          <div className="ip-section" data-testid="flow-inspector-response">
            <div className="sec-h"><span>Last response</span><span className="tag">{node.status} · {node.ms ?? '-'}ms</span></div>
            <pre className="code inspector-code">{JSON.stringify(node.result, null, 2)}</pre>
          </div>
        )}

        {runStep?.error && <div className="ip-section fail-text">{runStep.error}</div>}
      </div>

      <div className="inspector-foot">
        <button className="btn ghost sm danger" onClick={removeNode} disabled={nodes.length <= 1}>Delete block</button>
        <span className="inspector-counts">{directUp.length} up · {directDown.length} down</span>
      </div>
    </aside>
  );
}

function InputRow({ name, value, upstreamIds, nodes, onRename, onChange, onDelete }: {
  name: string;
  value: InputValue;
  upstreamIds: string[];
  nodes: FlowNode[];
  onRename: (name: string) => void;
  onChange: (patch: Partial<InputValue>) => void;
  onDelete: () => void;
}) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const ref = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    const handler = (event: globalThis.MouseEvent) => {
      if (ref.current && !ref.current.contains(event.target as Node)) setPickerOpen(false);
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, []);
  const isRef = value.kind === 'ref';
  const fromNode = isRef ? nodes.find(node => node.id === value.from) : null;
  const refValue = fromNode && isRef ? getByPath(outputForNode(fromNode), value.path) : undefined;

  return (
    <div className="input-row">
      <input className="input flush mono ir-name-input" defaultValue={name} onBlur={event => onRename(event.target.value)} />
      <div className="ir-val">
        <div className="ir-mode">
          <button className={!isRef ? 'is-on' : ''} onClick={() => onChange({ kind: 'literal', value: '' })} title="Literal value">abc</button>
          <button className={isRef ? 'is-on' : ''} disabled={upstreamIds.length === 0} onClick={() => onChange({ kind: 'ref', from: upstreamIds[0], path: 'body.id' })} title="Reference upstream output">ref</button>
        </div>
        {!isRef && <input className="input flush mono" style={{ flex: 1 }} value={value.value} onChange={e => onChange({ value: e.target.value })} placeholder="value" />}
        {isRef && (
          <div ref={ref} style={{ flex: 1, position: 'relative' }}>
            <button className="ref-pill" onClick={() => setPickerOpen(open => !open)}>
              <span className="ref-from">{value.from}</span>
              <span className="ref-dot">.</span>
              <span className="ref-path">{value.path}</span>
              <span className="ref-sample">{refValue != null ? formatSample(refValue) : '-'}</span>
            </button>
            {pickerOpen && (
              <RefPicker
                nodes={nodes}
                upstreamIds={upstreamIds}
                current={value}
                onPick={(from, path) => {
                  onChange({ kind: 'ref', from, path });
                  setPickerOpen(false);
                }}
              />
            )}
          </div>
        )}
      </div>
      <button className="row-del always" onClick={onDelete} title="Remove"><Icon.x /></button>
    </div>
  );
}

function RefPicker({ nodes, upstreamIds, current, onPick }: {
  nodes: FlowNode[];
  upstreamIds: string[];
  current: Extract<InputValue, { kind: 'ref' }>;
  onPick: (from: string, path: string) => void;
}) {
  const [activeFrom, setActiveFrom] = useState(current.from);
  const fromNode = nodes.find(node => node.id === activeFrom);
  const paths = useMemo(() => fromNode ? listPaths(outputForNode(fromNode)) : [], [fromNode]);

  return (
    <div className="menu ref-picker" onClick={event => event.stopPropagation()}>
      <div className="rp-h">Pick from upstream</div>
      <div className="rp-cols">
        <div className="rp-col">
          {upstreamIds.length === 0 && <div className="rp-empty">No upstream blocks yet.</div>}
          {upstreamIds.map(id => {
            const node = nodes.find(item => item.id === id);
            return node ? (
              <button key={id} className={`rp-from ${id === activeFrom ? 'is-active' : ''}`} onClick={() => setActiveFrom(id)}>
                <span className="rp-node-id">{id}</span>
                <span>{node.label}</span>
              </button>
            ) : null;
          })}
        </div>
        <div className="rp-col paths">
          {paths.map(path => (
            <button key={path.path} className={`rp-path ${path.path === current.path && activeFrom === current.from ? 'is-active' : ''}`} onClick={() => onPick(activeFrom, path.path)}>
              <span className="path">{path.path}</span>
              <span className="type">{path.type}</span>
              {path.sample && <span className="sample">{path.sample}</span>}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

function DepRow({ label, ids, labelFor, empty }: { label: string; ids: string[]; labelFor: (id: string) => string; empty: string }) {
  return (
    <div className="dep-row">
      <div className="dr-l">{label}</div>
      <div className="dr-r">
        {ids.length === 0 ? <span style={{ color: 'var(--fg-4)' }}>{empty}</span> : ids.map(id => (
          <span key={id} className="dep-pill"><span className="dp-id">{id}</span><span>{labelFor(id)}</span></span>
        ))}
      </div>
    </div>
  );
}

function EndpointPicker({ endpoints, selected, onPick }: {
  endpoints: SpecEntry[];
  selected: string;
  onPick: (endpoint: SpecEntry) => void;
}) {
  const [query, setQuery] = useState('');
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement | null>(null);
  const current = endpoints.find(endpoint => endpoint.rel_path === selected);
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return endpoints.slice(0, 24);
    return endpoints.filter(endpoint => {
      const haystack = `${endpoint.method} ${endpoint.path} ${endpoint.title} ${endpoint.rel_path}`.toLowerCase();
      return haystack.includes(q);
    }).slice(0, 40);
  }, [endpoints, query]);

  useEffect(() => {
    const handler = (event: globalThis.MouseEvent) => {
      if (ref.current && !ref.current.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, []);

  function pick(endpoint: SpecEntry) {
    onPick(endpoint);
    setOpen(false);
    setQuery('');
  }

  return (
    <div className="endpoint-picker" ref={ref} data-testid="endpoint-picker">
      <button className={`endpoint-current ${open ? 'is-open' : ''}`} onClick={() => setOpen(value => !value)} data-testid="endpoint-picker-current">
        {current ? (
          <>
            <MethodBadge method={current.method} />
            <span className="ep-path"><PathStr path={current.path} /></span>
          </>
        ) : <span className="ep-empty">No endpoint selected</span>}
        <span className="ep-caret">⌄</span>
      </button>
      {open && (
        <div className="endpoint-menu">
          <div className="endpoint-search">
            <Icon.search />
            <input autoFocus value={query} onChange={event => setQuery(event.target.value)} placeholder="Search endpoint by method, path, title..." data-testid="endpoint-picker-search" />
          </div>
          <div className="endpoint-results">
            {filtered.map(endpoint => (
              <button
                key={endpoint.rel_path}
                className={`endpoint-option ${endpoint.rel_path === selected ? 'is-active' : ''}`}
                onClick={() => pick(endpoint)}
                data-testid="endpoint-picker-option"
                data-endpoint-path={endpoint.rel_path}
              >
                <MethodBadge method={endpoint.method} />
                <span className="ep-option-main">
                  <span className="ep-option-path"><PathStr path={endpoint.path} /></span>
                  <span className="ep-option-title">{endpoint.title} · {endpoint.rel_path}</span>
                </span>
              </button>
            ))}
            {filtered.length === 0 && <div className="ep-empty result">No endpoint matched.</div>}
          </div>
        </div>
      )}
    </div>
  );
}

function Legend() {
  return (
    <div className="legend">
      <span><span className="lg-dot" style={{ background: 'var(--fg-5)' }} />idle</span>
      <span><span className="lg-dot pulse" style={{ background: 'var(--accent)' }} />running</span>
      <span><span className="lg-dot" style={{ background: 'var(--ok)' }} />passed</span>
      <span><span className="lg-dot" style={{ background: 'var(--fail)' }} />failed</span>
    </div>
  );
}

function ZoomBar({ zoom, setZoom }: { zoom: number; setZoom: React.Dispatch<React.SetStateAction<number>> }) {
  return (
    <div className="zoom-bar">
      <button onClick={() => setZoom(value => Math.max(0.4, value - 0.1))}>-</button>
      <span>{Math.round(zoom * 100)}%</span>
      <button onClick={() => setZoom(value => Math.min(1.6, value + 0.1))}>+</button>
      <span className="zoom-sep" />
      <button onClick={() => setZoom(1)} title="Reset zoom">reset</button>
    </div>
  );
}

function flowStepsToNodes(steps: Array<{ name: string; endpoint: string; inject: string[]; capture: FlowCapture[] }>): FlowNode[] {
  const captureOwner = new Map<string, { id: string; source: string }>();
  const nodes = steps.map((step, index) => {
    const id = `n${index + 1}`;
    for (const capture of step.capture) captureOwner.set(capture.name, { id, source: capture.source });
    return {
      id,
      relPath: step.endpoint,
      label: step.name || `Step ${index + 1}`,
      position: { x: 60 + index * 320, y: index % 2 === 0 ? 90 : 260 },
      inputs: {},
      captures: step.capture,
      status: 'idle' as const,
      result: null,
      ms: null,
      runMode: 'awaited' as const,
    };
  });
  return nodes.map((node, index) => {
    const step = steps[index];
    const inputs: Record<string, InputValue> = {};
    for (const name of step.inject) {
      const owner = captureOwner.get(name);
      inputs[name] = owner ? { kind: 'ref', from: owner.id, path: fromResponsePath(owner.source) } : { kind: 'literal', value: `{{${name}}}` };
    }
    return { ...node, inputs };
  });
}

function newNode(index: number): FlowNode {
  return {
    id: `n${Date.now().toString(36)}${index}`,
    relPath: '',
    label: 'New block',
    position: { x: 60 + index * 320, y: index % 2 === 0 ? 90 : 260 },
    inputs: {},
    captures: [],
    status: 'idle',
    result: null,
    ms: null,
    runMode: 'awaited',
  };
}

function deriveEdges(nodes: FlowNode[]) {
  const edges = new Map<string, FlowEdge>();
  for (const node of nodes) {
    const refs = Object.values(node.inputs).filter((input): input is Extract<InputValue, { kind: 'ref' }> => input.kind === 'ref');
    for (const ref of refs) edges.set(`${ref.from}-${node.id}`, { id: `${ref.from}-${node.id}`, from: ref.from, to: node.id });
  }
  return [...edges.values()];
}

function topoWaves(nodes: FlowNode[], edges: FlowEdge[]) {
  const incoming = new Map(nodes.map(node => [node.id, 0]));
  const adj = new Map(nodes.map(node => [node.id, [] as string[]]));
  edges.forEach(edge => {
    incoming.set(edge.to, (incoming.get(edge.to) ?? 0) + 1);
    adj.get(edge.from)?.push(edge.to);
  });
  const waves: string[][] = [];
  let frontier = nodes.filter(node => (incoming.get(node.id) ?? 0) === 0).map(node => node.id);
  const visited = new Set(frontier);
  while (frontier.length > 0) {
    waves.push(frontier);
    const next: string[] = [];
    frontier.forEach(id => {
      (adj.get(id) ?? []).forEach(to => {
        incoming.set(to, (incoming.get(to) ?? 0) - 1);
        if (incoming.get(to) === 0 && !visited.has(to)) {
          visited.add(to);
          next.push(to);
        }
      });
    });
    frontier = next;
  }
  return waves;
}

function renderFlowMarkdown(name: string, title: string, nodes: FlowNode[]) {
  const sorted = [...nodes].filter(node => node.relPath).sort((a, b) => a.position.x - b.position.x || a.position.y - b.position.y);
  const withDerivedCaptures = deriveCapturesForRefs(sorted);
  const body = withDerivedCaptures.map((node, index) => {
    const inject = Object.keys(node.inputs).filter(name => node.inputs[name].kind === 'ref');
    const lines = [`${index + 1}. **${node.label || `Step ${index + 1}`}** -> \`${node.relPath}\``];
    if (inject.length > 0) lines.push(`   - Inject: ${inject.map(item => `\`${item}\``).join(', ')}`);
    for (const capture of node.captures.filter(item => item.source && item.name)) lines.push(`   - Capture: \`${capture.source}\` as \`${capture.name}\``);
    return lines.join('\n');
  }).join('\n');
  return `---\ntype: pipeline\nname: ${slug(name)}\ndescription: Created in Reqbook web canvas\ncontinue-on-error: false\nparallel: false\n---\n\n# ${title || name}\n\n## Steps\n\n${body || '1. **Select endpoint** -> `users/get-users.md`'}\n`;
}

function deriveCapturesForRefs(nodes: FlowNode[]) {
  return nodes.map(node => {
    const captures = [...node.captures];
    for (const downstream of nodes) {
      for (const [name, input] of Object.entries(downstream.inputs)) {
        if (input.kind === 'ref' && input.from === node.id && !captures.some(capture => capture.name === name)) {
          captures.push({ name, source: toResponsePath(input.path) });
        }
      }
    }
    return { ...node, captures };
  });
}

function ensureCapture(nodes: FlowNode[], nodeId: string, source: string, name: string) {
  return nodes.map(node => {
    if (node.id !== nodeId || node.captures.some(capture => capture.name === name)) return node;
    return { ...node, captures: [...node.captures, { source, name }] };
  });
}

function mergeEndpointInputs(existing: Record<string, InputValue>, endpoint: SpecEntry, upstreamIds: string[]) {
  const next = { ...existing };
  for (const name of endpointInputNames(endpoint)) {
    if (!next[name]) next[name] = defaultInputValue(name, upstreamIds);
  }
  return next;
}

function endpointInputNames(endpoint: SpecEntry) {
  const names = new Set<string>();
  for (const match of endpoint.path.matchAll(/:([A-Za-z0-9_]+)/g)) names.add(match[1]);
  for (const match of endpoint.path.matchAll(/\{\{([A-Za-z0-9_]+)\}\}/g)) {
    if (match[1] !== 'baseUrl') names.add(match[1]);
  }
  return [...names];
}

function defaultInputValue(name: string, upstreamIds: string[]): InputValue {
  if (upstreamIds.length === 0) return { kind: 'literal', value: '' };
  return { kind: 'ref', from: upstreamIds[0], path: defaultOutputPath(name) };
}

function defaultOutputPath(name: string) {
  const lower = name.toLowerCase();
  if (lower.includes('token')) return 'body.token';
  if (lower === 'userid' || lower === 'user_id') return 'body.user.id';
  if (lower.endsWith('id') || lower === 'id') return 'body.id';
  return `body.${name}`;
}

function listPaths(obj: unknown, prefix = ''): Array<{ path: string; type: string; sample: string }> {
  if (!obj || typeof obj !== 'object') return [];
  return Object.entries(obj as Record<string, unknown>).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    const isObject = value != null && typeof value === 'object' && !Array.isArray(value);
    const row = { path, type: isObject ? 'object' : Array.isArray(value) ? 'array' : typeof value, sample: isObject ? '' : Array.isArray(value) ? `[${value.length}]` : String(value).slice(0, 36) };
    return isObject ? [row, ...listPaths(value, path)] : [row];
  });
}

function getByPath(obj: unknown, path: string) {
  return path.split('.').reduce<unknown>((current, part) => {
    if (!current || typeof current !== 'object') return undefined;
    return (current as Record<string, unknown>)[part];
  }, obj);
}

function outputForNode(node: FlowNode) {
  return node.result ?? DEFAULT_OUTPUT;
}

function parseJson(value?: string) {
  if (!value) return null;
  try { return JSON.parse(value); } catch { return null; }
}

function formatSample(value: unknown) {
  return (typeof value === 'string' ? value : JSON.stringify(value)).slice(0, 24);
}

function fromResponsePath(source: string) {
  return source.replace(/^response\./, '');
}

function toResponsePath(path: string) {
  return path.startsWith('response.') ? path : `response.${path}`;
}

function slug(value: string) {
  return value.trim().toLowerCase().replace(/[^a-z0-9_-]+/g, '-').replace(/^-+|-+$/g, '') || 'new-flow';
}

function slugVar(value: string) {
  return value.trim().replace(/[^A-Za-z0-9_]+/g, '_').replace(/^_+|_+$/g, '');
}

function BackIcon() {
  return <svg width="11" height="11" viewBox="0 0 11 11" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><line x1="9" y1="5.5" x2="2" y2="5.5" /><polyline points="5,3 2,5.5 5,8" /></svg>;
}

function BoltIcon() {
  return <svg width="11" height="11" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round"><path d="M6.7 1.5 3 6.5h3L5.3 10.5 9 5.2H6.1z" /></svg>;
}
