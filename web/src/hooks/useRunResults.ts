export interface StoredRun {
  status: number | null;
  passed: boolean;
  duration_ms: number;
  at: number;
}

const storageKey = (relPath: string) => `trellis:run:${relPath}`;

export function getStoredRun(relPath: string): StoredRun | null {
  try {
    const raw = localStorage.getItem(storageKey(relPath));
    return raw ? (JSON.parse(raw) as StoredRun) : null;
  } catch {
    return null;
  }
}

export function saveRun(
  relPath: string,
  result: { status?: number | null; passed: boolean; duration_ms: number },
): void {
  try {
    const stored: StoredRun = {
      status: result.status ?? null,
      passed: result.passed,
      duration_ms: result.duration_ms,
      at: Date.now(),
    };
    localStorage.setItem(storageKey(relPath), JSON.stringify(stored));
    window.dispatchEvent(new CustomEvent('trellis:run-saved', { detail: { relPath } }));
  } catch {
    // localStorage unavailable
  }
}

export function timeAgo(at: number): string {
  const secs = Math.floor((Date.now() - at) / 1000);
  if (secs < 5) return 'just now';
  if (secs < 60) return `${secs}s ago`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
  if (secs < 86400) return `${Math.floor(secs / 3600)}h ago`;
  return `${Math.floor(secs / 86400)}d ago`;
}
