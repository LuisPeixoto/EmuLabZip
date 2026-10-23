// Small typed wrapper around localStorage. Every access is guarded: storage can be unavailable
// (private mode, cleared data) and the app must still work.

export function load<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(`emulabzip.${key}`);
    return raw == null ? fallback : { ...fallback, ...JSON.parse(raw) };
  } catch {
    return fallback;
  }
}

export function loadValue<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(`emulabzip.${key}`);
    return raw == null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}

export function save(key: string, value: unknown) {
  try {
    localStorage.setItem(`emulabzip.${key}`, JSON.stringify(value));
  } catch {
    /* storage unavailable */
  }
}
