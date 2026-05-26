const COLORS: Record<string, string> = {
  GET:     '#dbeafe:#1d4ed8',
  POST:    '#dcfce7:#15803d',
  PUT:     '#fef9c3:#92400e',
  PATCH:   '#f3e8ff:#7c3aed',
  DELETE:  '#fee2e2:#dc2626',
  HEAD:    '#f3f4f6:#374151',
  OPTIONS: '#f3f4f6:#374151',
};

export function MethodBadge({ method }: { method: string }) {
  const [bg, fg] = (COLORS[method] ?? '#f3f4f6:#374151').split(':');
  return (
    <span style={{
      background: bg, color: fg,
      fontSize: '.7rem', fontWeight: 700, fontFamily: 'monospace',
      padding: '.2rem .45rem', borderRadius: 4, flexShrink: 0,
      minWidth: 54, textAlign: 'center', display: 'inline-block',
    }}>
      {method}
    </span>
  );
}
