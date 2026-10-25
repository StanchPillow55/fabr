import { VIEW_MODES, type GroupBy, type SortBy } from './model';
import { FILTERS } from './viewstate';

export function getPref<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem('fabr:' + key);
    return raw === null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}

export function setPref(key: string, value: unknown): void {
  try {
    localStorage.setItem('fabr:' + key, JSON.stringify(value));
  } catch {
    return;
  }
}

interface Pref {
  field: string;
  key?: string;
  def: unknown;
  ok: (v: unknown) => boolean;
}

const bool = (v: unknown) => typeof v === 'boolean';
const oneOf = (values: readonly unknown[]) => (v: unknown) => values.includes(v);
const SORTS: SortBy[] = ['source', 'node', 'pri', 'status', 'title', 'due'];
const GROUPS: GroupBy[] = ['none', 'node', 'status', 'source', 'due'];

export const PREFS: Record<'prefs' | 'list', Pref[]> = {
  prefs: [
    { field: 'filter', def: 'all', ok: oneOf(FILTERS.map((f) => f.value)) },
    { field: 'hideDone', def: false, ok: bool },
    { field: 'hideEmpty', def: false, ok: bool },
    { field: 'viewMode', key: 'view_mode', def: 'folders', ok: oneOf(VIEW_MODES) },
    { field: 'showHidden', def: false, ok: bool },
    { field: 'focus', def: null, ok: (v) => typeof v === 'string' },
    { field: 'view', def: 'tree', ok: oneOf(['tree', 'list']) },
  ],
  list: [
    { field: 'sortBy', def: 'pri', ok: oneOf(SORTS) },
    { field: 'sortDir', def: 'asc', ok: oneOf(['asc', 'desc']) },
    { field: 'groupBy', def: 'none', ok: oneOf(GROUPS) },
  ],
};

export function restore(target: object, group: keyof typeof PREFS): void {
  const raw = getPref<unknown>(group, {});
  const saved = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  const t = target as Record<string, unknown>;
  for (const p of PREFS[group]) {
    const v = saved[p.key ?? p.field];
    t[p.field] = p.ok(v) ? v : p.def;
  }
}

export function persist(src: object, group: keyof typeof PREFS): void {
  const s = src as Record<string, unknown>;
  setPref(group, Object.fromEntries(PREFS[group].map((p) => [p.key ?? p.field, s[p.field]])));
}
