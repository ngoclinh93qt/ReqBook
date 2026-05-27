export function TrellisMark({ className = 'trellis-mark' }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 36 28" fill="none" aria-hidden="true">
      <path d="M11 4 L5 4 L5 24 L11 24" />
      <path d="M25 4 L31 4 L31 24 L25 24" />
      <circle cx="14" cy="14" r="1.08" />
      <circle cx="18" cy="14" r="2.04" />
      <circle cx="22" cy="14" r="1.08" />
    </svg>
  );
}
