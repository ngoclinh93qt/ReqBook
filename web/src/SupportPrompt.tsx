import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Icon } from './ui';

const SUPPORT_URLS = {
  feedback: 'https://markapidown.net/out/feedback',
  bug: 'https://markapidown.net/out/bug',
  star: 'https://markapidown.net/out/star',
} as const;

const DISMISSED_KEY = 'rqb-support-dismissed-at-v1';
const ACTION_KEY = 'rqb-support-action-at-v1';
const TELEMETRY_KEY = 'rqb-anonymous-usage-v1';
const CLIENT_ID_KEY = 'rqb-anonymous-client-v1';
const PROMPT_COOLDOWN_MS = 30 * 24 * 60 * 60 * 1000;
const HEARTBEAT_INTERVAL_MS = 60 * 1000;
const DEFAULT_HEARTBEAT_URL = 'https://markapidown.net/api/app/heartbeat';

type SupportAction = keyof typeof SUPPORT_URLS;

function isTauri() {
  return '__TAURI_INTERNALS__' in window;
}

function createAnonymousClientId() {
  if (typeof crypto.randomUUID === 'function') return crypto.randomUUID();
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('');
}

function getAnonymousClientId() {
  const existing = localStorage.getItem(CLIENT_ID_KEY);
  if (existing) return existing;
  const created = createAnonymousClientId();
  localStorage.setItem(CLIENT_ID_KEY, created);
  return created;
}

async function openExternal(url: string) {
  if (isTauri()) {
    await invoke('open_external', { url });
    return;
  }

  const link = document.createElement('a');
  link.href = url;
  link.target = '_blank';
  link.rel = 'noopener noreferrer';
  document.body.appendChild(link);
  link.click();
  link.remove();
}

function useAnonymousPresence(enabled: boolean, version: string) {
  useEffect(() => {
    if (!enabled) return;

    const endpoint = import.meta.env.VITE_RQB_TELEMETRY_ENDPOINT || DEFAULT_HEARTBEAT_URL;
    const clientId = getAnonymousClientId();
    const surface = isTauri() ? 'desktop' : 'web';

    const sendHeartbeat = () => {
      if (document.visibilityState !== 'visible') return;
      void fetch(endpoint, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ clientId, surface, version }),
        credentials: 'omit',
        keepalive: true,
      }).catch(() => {});
    };

    sendHeartbeat();
    const timer = window.setInterval(sendHeartbeat, HEARTBEAT_INTERVAL_MS);
    document.addEventListener('visibilitychange', sendHeartbeat);
    return () => {
      window.clearInterval(timer);
      document.removeEventListener('visibilitychange', sendHeartbeat);
    };
  }, [enabled, version]);
}

export function SupportPrompt({
  open,
  onOpenChange,
  version,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  version: string;
}) {
  const [anonymousUsage, setAnonymousUsage] = useState(
    () => localStorage.getItem(TELEMETRY_KEY) === 'enabled',
  );
  const [linkError, setLinkError] = useState('');

  useAnonymousPresence(anonymousUsage, version);

  useEffect(() => {
    if (open) return;
    const lastDismissed = Number(localStorage.getItem(DISMISSED_KEY) ?? 0);
    const lastAction = Number(localStorage.getItem(ACTION_KEY) ?? 0);
    if (Date.now() - Math.max(lastDismissed, lastAction) < PROMPT_COOLDOWN_MS) return;

    const timer = window.setTimeout(() => onOpenChange(true), 60_000);
    return () => window.clearTimeout(timer);
  }, [onOpenChange, open]);

  function close() {
    localStorage.setItem(DISMISSED_KEY, String(Date.now()));
    setLinkError('');
    onOpenChange(false);
  }

  function setTelemetry(enabled: boolean) {
    localStorage.setItem(TELEMETRY_KEY, enabled ? 'enabled' : 'disabled');
    if (!enabled) localStorage.removeItem(CLIENT_ID_KEY);
    setAnonymousUsage(enabled);
  }

  async function runAction(action: SupportAction) {
    setLinkError('');
    try {
      await openExternal(SUPPORT_URLS[action]);
      localStorage.setItem(ACTION_KEY, String(Date.now()));
      onOpenChange(false);
    } catch (error) {
      setLinkError(error instanceof Error ? error.message : String(error));
    }
  }

  if (!open) return null;

  return (
    <aside className="support-prompt" aria-labelledby="support-title" data-testid="support-prompt">
      <div className="support-head">
        <div>
          <span className="support-kicker">Reqbook community</span>
          <h2 id="support-title">Help improve Reqbook</h2>
        </div>
        <button className="btn icon sm support-close" onClick={close} title="Close">
          <Icon.x />
        </button>
      </div>

      <p className="support-copy">
        Share workflow feedback, report a reproducible problem, or support the project on GitHub.
      </p>

      <div className="support-actions">
        <button onClick={() => runAction('feedback')}>
          <span className="support-action-icon"><Icon.message /></span>
          <span><b>Share feedback</b><small>Suggest a workflow improvement</small></span>
          <Icon.arr />
        </button>
        <button onClick={() => runAction('bug')}>
          <span className="support-action-icon bug"><Icon.bug /></span>
          <span><b>Report a bug</b><small>Open a structured GitHub issue</small></span>
          <Icon.arr />
        </button>
        <button onClick={() => runAction('star')}>
          <span className="support-action-icon star"><Icon.star /></span>
          <span><b>Star on GitHub</b><small>Help other developers discover Reqbook</small></span>
          <Icon.arr />
        </button>
      </div>

      <div className="support-telemetry">
        <div>
          <b>Share anonymous active usage</b>
          <small>Random ID, app surface, version, and heartbeat only. No project or request data.</small>
        </div>
        <button
          className={`toggle${anonymousUsage ? ' is-on' : ''}`}
          role="switch"
          aria-checked={anonymousUsage}
          aria-label="Share anonymous active usage"
          onClick={() => setTelemetry(!anonymousUsage)}
        >
          <span />
        </button>
      </div>

      {linkError && <div className="support-error">{linkError}</div>}
    </aside>
  );
}
