import type { ExecResult } from '../types';

interface Props {
  result: ExecResult;
  onCaptureToken: (token: string) => void;
}

function findToken(body: string): string | null {
  try {
    const obj = JSON.parse(body);
    return obj?.token ?? obj?.access_token ?? obj?.accessToken ?? obj?.jwt ?? null;
  } catch { return null; }
}

export function ResultBox({ result, onCaptureToken }: Props) {
  const passed = result.diff?.passed ?? false;
  const status = result.response?.status;
  const statusText = status ? `HTTP ${status}` : 'No response';
  const token = result.response?.body ? findToken(result.response.body) : null;

  return (
    <div style={{ background: '#fff', border: '1px solid #e8e8e8', borderRadius: 8, overflow: 'hidden', marginTop: '1rem' }}>
      <div style={{
        padding: '.65rem 1rem', fontWeight: 600, fontSize: '.875rem',
        borderBottom: '1px solid #e8e8e8',
        background: passed ? '#f0fdf4' : '#fef2f2',
        color: passed ? '#15803d' : '#dc2626',
      }}>
        {passed ? '✓ Passed' : '✗ Failed'} · {statusText} · {result.duration_ms}ms
      </div>
      {token && (
        <div style={{ padding: '.6rem 1rem', borderBottom: '1px solid #e8e8e8', background: '#faf5ff' }}>
          <button
            onClick={() => onCaptureToken(String(token))}
            style={{ background: '#f3e8ff', color: '#7c3aed', border: '1px solid #ddd6fe', borderRadius: 6, padding: '.3rem .75rem', fontSize: '.8rem', fontWeight: 600, cursor: 'pointer' }}
          >
            → Set as {'{{'+'token'+'}}'}
          </button>
          <span style={{ fontSize: '.75rem', color: '#888', marginLeft: '.6rem' }}>Token found in response</span>
        </div>
      )}
      <pre style={{
        padding: '1rem 1.25rem', fontFamily: 'monospace', fontSize: '.82rem',
        overflowX: 'auto', whiteSpace: 'pre', color: '#333', margin: 0,
      }}>
        {JSON.stringify(result, null, 2)}
      </pre>
    </div>
  );
}
