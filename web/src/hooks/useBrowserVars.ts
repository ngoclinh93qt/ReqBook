import { useState, useCallback } from 'react';

const KEY = 'mad_vars';

function load(): Record<string, string> {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? '{}');
  } catch {
    return {};
  }
}

export function useBrowserVars() {
  const [vars, setVars] = useState<Record<string, string>>(load);

  const save = useCallback((next: Record<string, string>) => {
    localStorage.setItem(KEY, JSON.stringify(next));
    setVars(next);
  }, []);

  return { vars, save };
}
