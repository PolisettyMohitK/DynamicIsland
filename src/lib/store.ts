// Small helpers shared by the pages: saved state and today's date.

export function load<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}

export function save(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // not remembered across restarts; fine
  }
}

/** Local date as YYYY-MM-DD, for "today" counters that reset at midnight. */
export const today = () => new Date().toLocaleDateString('en-CA');

export const pad = (n: number) => String(Math.floor(n)).padStart(2, '0');

/** 90_000 → "01:30" */
export const mmss = (ms: number) => `${pad(ms / 60000)}:${pad((ms / 1000) % 60)}`;

/** Timestamp → "08:07" */
export const hhmm = (t: number) =>
  new Date(t).toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' });
