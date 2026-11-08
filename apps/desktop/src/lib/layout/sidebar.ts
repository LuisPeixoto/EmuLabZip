/** What the person chose about the sidebar: whether it is narrow, which entries are hidden, and each section's order. */
export interface SidebarPrefs {
  collapsed: boolean;
  hidden: string[];
  order: Record<string, string[]>;
}

export const DEFAULT_SIDEBAR: SidebarPrefs = { collapsed: false, hidden: [], order: {} };

const ID = /^[a-z0-9-]{1,32}$/;

export function sanitizeSidebar(raw: unknown): SidebarPrefs {
  const o = (typeof raw === "object" && raw !== null ? raw : {}) as Record<string, unknown>;
  const list = (v: unknown): string[] => (Array.isArray(v) ? v.filter((x): x is string => typeof x === "string" && ID.test(x)).slice(0, 64) : []);
  const order: Record<string, string[]> = {};
  if (typeof o.order === "object" && o.order !== null) {
    for (const [section, ids] of Object.entries(o.order as Record<string, unknown>)) if (ID.test(section)) order[section] = list(ids);
  }
  return { collapsed: o.collapsed === true, hidden: list(o.hidden), order };
}

/**
 * A section's entries in the order the person chose. Entries that are new since the order was saved come last, in their
 * own order, and entries that no longer exist are dropped, so an update never loses or hides an entry.
 */
export function mergeOrder(defaults: string[], saved: string[] | undefined): string[] {
  const known = (saved ?? []).filter((id, i, all) => defaults.includes(id) && all.indexOf(id) === i);
  return [...known, ...defaults.filter((id) => !known.includes(id))];
}

/** The entry at `from` moved to sit where the entry at `to` is. */
export function move<T>(list: T[], from: number, to: number): T[] {
  if (from === to || from < 0 || to < 0 || from >= list.length || to >= list.length) return list;
  const next = [...list];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}
