import type { Item, Node, Source, SourceError } from './api/types';
import { HEAD, ROW, LIST_PAD, ROLLUP, MAX_ROWS } from './constants';

export type Key = string;
export type Filter = 'all' | 'open' | 'wip' | 'overdue';
export type DueBucket = 'overdue' | 'today' | 'week' | 'later' | 'none' | 'done';
export type SortBy = 'source' | 'node' | 'pri' | 'status' | 'title' | 'due';
export type GroupBy = 'none' | 'node' | 'status' | 'source' | 'due';
export type SortDir = 'asc' | 'desc';
export type ViewMode = 'nodes' | 'folders' | 'all';

export interface ItemFilter {
  today: string;
  now?: string;
  filter: Filter;
  hideDone: boolean;
}

export const ROOT_KEY: Key = ':';

export function isVirtualRoot(n: { source: string; path: string }): boolean {
  return n.source === '' && n.path === '';
}

export function nodeKey(n: { source: string; path: string }): Key;
export function nodeKey(source: string, path: string): Key;
export function nodeKey(a: string | { source: string; path: string }, b?: string): Key {
  return typeof a === 'string' ? `${a}:${b ?? ''}` : `${a.source}:${a.path}`;
}

export const dueDate = (due: string): string => due.slice(0, 10);
export const dueTime = (due: string): string | null => (due.length > 10 ? due.slice(11, 16) : null);

export function joinDue(date: string, time: string | null | undefined): string {
  return time ? `${date}T${time}` : date;
}

export function dayDiff(a: string, b: string): number {
  const [ya, ma, da] = dueDate(a).split('-').map(Number);
  const [yb, mb, db] = dueDate(b).split('-').map(Number);
  return Math.round((Date.UTC(ya, ma - 1, da) - Date.UTC(yb, mb - 1, db)) / 86400000);
}

export function clockMinute(d = new Date()): string {
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}`;
}

export function nowFor(today: string, now?: string): string {
  if (now) return now;
  const c = clockMinute();
  return dueDate(c) === today ? c : `${today}T00:00`;
}

export function isOverdue(item: Item, today: string, now?: string): boolean {
  if (item.status === 'done' || !item.due) return false;
  return item.due.length > 10 ? item.due < nowFor(today, now) : item.due < today;
}

export function compareDue(a: string | null, b: string | null): number {
  if (a === b) return 0;
  if (!a) return 1;
  if (!b) return -1;
  const da = dueDate(a);
  const db = dueDate(b);
  if (da !== db) return da < db ? -1 : 1;
  const ta = dueTime(a);
  const tb = dueTime(b);
  if (ta === tb) return 0;
  if (ta === null) return 1;
  if (tb === null) return -1;
  return ta < tb ? -1 : 1;
}

export function dueClass(item: Item, today: string, now?: string): '' | 'overdue' | 'soon' {
  if (!item.due || item.status === 'done') return '';
  if (isOverdue(item, today, now)) return 'overdue';
  return dayDiff(item.due, today) <= 3 ? 'soon' : '';
}

export function dueLabel(item: Item, today: string, full = false): string {
  if (!item.due) return '';
  const time = dueTime(item.due);
  if (full) return time ? `${dueDate(item.due)} ${time}` : item.due;
  const date = dueDate(item.due);
  if (item.status !== 'done') {
    const d = dayDiff(date, today);
    if (d === 0) return time ? `today ${time}` : 'today';
    if (d === 1) return time ? `tomorrow ${time}` : 'tomorrow';
  }
  const day = date.slice(0, 4) === today.slice(0, 4) ? date.slice(5) : date;
  return time ? `${day} ${time}` : day;
}

export function passes(item: Item, f: ItemFilter): boolean {
  if (f.hideDone && item.status === 'done') return false;
  switch (f.filter) {
    case 'open': return item.status !== 'done';
    case 'wip': return item.status === 'wip';
    case 'overdue': return isOverdue(item, f.today, f.now);
    default: return true;
  }
}

export interface Rollup {
  todo: number;
  wip: number;
  done: number;
  overdue: number;
  next: string | null;
  nodes: number;
}

export function rollup(n: Node, today: string, now?: string): Rollup {
  const r: Rollup = { todo: 0, wip: 0, done: 0, overdue: 0, next: null, nodes: 0 };
  (function walk(k: Node) {
    for (const i of k.items) {
      r[i.status]++;
      if (isOverdue(i, today, now)) r.overdue++;
      if (i.status !== 'done' && i.due && (!r.next || compareDue(i.due, r.next) < 0)) r.next = i.due;
    }
    r.nodes += k.children.filter((c) => c.managed).length;
    k.children.forEach(walk);
  })(n);
  return r;
}

export function matches(n: Node, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return false;
  return (
    n.name.toLowerCase().includes(q) ||
    n.path.toLowerCase().includes(q) ||
    n.items.some((i) => i.title.toLowerCase().includes(q))
  );
}

export interface TreeIndex {
  byKey: Map<Key, Node>;
  parentOf: Map<Key, Node>;
}

export function indexTree(root: Node): TreeIndex {
  const byKey = new Map<Key, Node>();
  const parentOf = new Map<Key, Node>();
  (function walk(n: Node, parent: Node | null) {
    const k = nodeKey(n);
    byKey.set(k, n);
    if (parent) parentOf.set(k, parent);
    n.children.forEach((c) => walk(c, n));
  })(root, null);
  return { byKey, parentOf };
}

export function computeForceOpen(index: TreeIndex, query: string): Set<Key> {
  const out = new Set<Key>();
  if (!query.trim()) return out;
  for (const n of index.byKey.values()) {
    if (!matches(n, query)) continue;
    for (let p = index.parentOf.get(nodeKey(n)); p; p = index.parentOf.get(nodeKey(p))) out.add(nodeKey(p));
  }
  return out;
}

export interface BoxCtx extends ItemFilter {
  collapsed: ReadonlySet<Key>;
  moreOpen: ReadonlySet<Key>;
  forceOpen: ReadonlySet<Key>;
}

export function isCollapsed(n: Node, ctx: Pick<BoxCtx, 'collapsed' | 'forceOpen'>): boolean {
  const k = nodeKey(n);
  if ((!n.children.length && !n.items.length) || ctx.forceOpen.has(k)) return false;
  return ctx.collapsed.has(k);
}

export interface BoxModel {
  collapsed: boolean;
  visible: Item[];
  shown: Item[];
  more: number;
  h: number;
}

export function boxModel(n: Node, ctx: BoxCtx): BoxModel {
  const collapsed = isCollapsed(n, ctx);
  const visible = n.items.filter((i) => passes(i, ctx));
  const open = ctx.moreOpen.has(nodeKey(n));
  const shown = open ? visible : visible.slice(0, MAX_ROWS);
  const more = visible.length - shown.length;
  let h = HEAD;
  if (isVirtualRoot(n) && !collapsed) {
    h += LIST_PAD + (1 + n.children.length) * ROW;
  } else if (n.managed && !collapsed) {
    h += LIST_PAD + ROW + Math.max(shown.length, 1) * ROW;
    if (more > 0 || (open && visible.length > MAX_ROWS)) h += ROW;
  }
  if (collapsed) h += ROLLUP;
  return { collapsed, visible, shown, more, h: h + 2 };
}

export function cardTitle(n: Pick<Node, 'source' | 'path' | 'name' | 'meta'>): string {
  if (isVirtualRoot(n)) return '/';
  return String(n.meta.name || n.name);
}

export function cardAside(n: Pick<Node, 'source' | 'path' | 'name' | 'meta'>, source: Pick<Source, 'id' | 'name' | 'host'> | null): string {
  if (isVirtualRoot(n)) return 'all sources';
  if (source) {
    const where = source.host || 'local';
    const sname = source.name || source.id;
    return sname === cardTitle(n) ? where : `${sname} · ${where}`;
  }
  return String(n.meta.kind ?? '');
}

export interface SourceLine {
  key: Key;
  source: string;
  name: string;
  where: string;
  open: number;
  error: string | null;
  pending: boolean;
}

export function rootCounts(root: Node, today: string, now?: string): { open: number; wip: number; overdue: number } {
  const r = rollup(root, today, now);
  return { open: r.todo + r.wip, wip: r.wip, overdue: r.overdue };
}

export interface ErrorPart {
  text: string;
  url?: string;
}

export function errorParts(message: string): ErrorPart[] {
  const parts: ErrorPart[] = [];
  let last = 0;
  for (const m of message.matchAll(/https:\/\/[^\s<>"']+/g)) {
    const url = m[0].replace(/[.,;:!?)\]]+$/, '');
    if (url.length <= 'https://'.length) continue;
    if (m.index > last) parts.push({ text: message.slice(last, m.index) });
    parts.push({ text: url, url });
    last = m.index + url.length;
  }
  if (last < message.length) parts.push({ text: message.slice(last) });
  return parts.length ? parts : [{ text: message }];
}

export function sourceLines(root: Node, sources: Source[], errors: SourceError[], today: string, now?: string, pending: readonly string[] = []): SourceLine[] {
  return root.children.map((c) => {
    const src = sources.find((s) => s.id === c.source);
    const r = rollup(c, today, now);
    return {
      key: nodeKey(c),
      source: c.source,
      name: src?.name || c.name || c.source,
      where: src?.host || 'local',
      open: r.todo + r.wip,
      error: errors.find((e) => e.source === c.source)?.message ?? null,
      pending: pending.includes(c.source),
    };
  });
}

export function compareItems(a: Item, b: Item): number {
  const da = a.status === 'done' ? 1 : 0;
  const db = b.status === 'done' ? 1 : 0;
  if (da !== db) return da - db;
  const pa = a.pri ?? 99;
  const pb = b.pri ?? 99;
  if (pa !== pb) return pa - pb;
  const dc = compareDue(a.due, b.due);
  if (dc) return dc;
  return a.title.localeCompare(b.title);
}

export function dueBucket(item: Item, today: string, now?: string): DueBucket {
  if (item.status === 'done') return 'done';
  if (!item.due) return 'none';
  if (isOverdue(item, today, now)) return 'overdue';
  const d = dayDiff(item.due, today);
  if (d === 0) return 'today';
  if (d <= 7) return 'week';
  return 'later';
}

export const DUE_BUCKET_ORDER: DueBucket[] = ['overdue', 'today', 'week', 'later', 'none', 'done'];
export const DUE_BUCKET_LABEL: Record<DueBucket, string> = {
  overdue: 'Overdue',
  today: 'Today',
  week: 'Next 7 days',
  later: 'Later',
  none: 'No date',
  done: 'Done',
};

export interface ListRow {
  id: string;
  nodeKey: Key;
  source: string;
  path: string;
  nodeName: string;
  index: number;
  item: Item;
}

export function flattenItems(root: Node, f: ItemFilter, query = ''): ListRow[] {
  const q = query.trim().toLowerCase();
  const rows: ListRow[] = [];
  (function walk(n: Node) {
    const nk = nodeKey(n);
    n.items.forEach((item, index) => {
      if (!passes(item, f)) return;
      if (q && !(item.title.toLowerCase().includes(q) || n.name.toLowerCase().includes(q) || n.path.toLowerCase().includes(q))) return;
      rows.push({ id: `${nk}#${index}`, nodeKey: nk, source: n.source, path: n.path, nodeName: n.name, index, item });
    });
    n.children.forEach(walk);
  })(root);
  return rows;
}

const STATUS_ORDER: Record<string, number> = { wip: 0, todo: 1, done: 2 };

export function sortRows(rows: ListRow[], by: SortBy, dir: SortDir, today: string): ListRow[] {
  const sign = dir === 'asc' ? 1 : -1;
  const cmp = (a: ListRow, b: ListRow): number => {
    switch (by) {
      case 'source': return a.source.localeCompare(b.source);
      case 'node': return a.path.localeCompare(b.path);
      case 'pri': return (a.item.pri ?? 99) - (b.item.pri ?? 99);
      case 'status': return STATUS_ORDER[a.item.status] - STATUS_ORDER[b.item.status];
      case 'title': return a.item.title.localeCompare(b.item.title);
      case 'due':
        if (!a.item.due && !b.item.due) return 0;
        if (!a.item.due) return sign;
        if (!b.item.due) return -sign;
        return compareDue(a.item.due, b.item.due);
    }
  };
  return rows
    .map((r, i) => ({ r, i }))
    .sort((x, y) => sign * cmp(x.r, y.r) || compareItems(x.r.item, y.r.item) || x.i - y.i)
    .map((x) => x.r);
}

export interface RowGroup {
  id: string;
  label: string;
  rows: ListRow[];
}

export function groupRows(rows: ListRow[], by: GroupBy, today: string, now?: string): RowGroup[] {
  if (by === 'none') return [{ id: '', label: '', rows }];
  const idOf = (r: ListRow): [string, string] => {
    switch (by) {
      case 'node': return [r.nodeKey, r.path || r.nodeName];
      case 'status': return [r.item.status, r.item.status];
      case 'source': return [r.source, r.source];
      case 'due': {
        const b = dueBucket(r.item, today, now);
        return [b, DUE_BUCKET_LABEL[b]];
      }
    }
  };
  const map = new Map<string, RowGroup>();
  for (const r of rows) {
    const [id, label] = idOf(r);
    let g = map.get(id);
    if (!g) map.set(id, (g = { id, label, rows: [] }));
    g.rows.push(r);
  }
  const groups = [...map.values()];
  if (by === 'due') groups.sort((a, b) => DUE_BUCKET_ORDER.indexOf(a.id as DueBucket) - DUE_BUCKET_ORDER.indexOf(b.id as DueBucket));
  if (by === 'status') groups.sort((a, b) => STATUS_ORDER[a.id] - STATUS_ORDER[b.id]);
  return groups;
}

export const VIEW_MODES: ViewMode[] = ['nodes', 'folders', 'all'];

export interface ViewOpts {
  mode: ViewMode;
  showHidden: boolean;
  hideEmpty?: boolean;
  items?: ItemFilter;
  keep?: Key | null;
}

export function isHidden(n: Pick<Node, 'meta'>): boolean {
  const h: unknown = n.meta.hidden;
  return h === true || (typeof h === 'string' && h.trim().toLowerCase() === 'true');
}

const baseName = (n: Node): string => n.path.split('/').pop() || n.name;

export function visibleTree(root: Node, o: ViewOpts): Node {
  const shaped = shapeTree(root, o);
  return o.hideEmpty && o.items ? pruneEmpty(shaped, o.items, o.keep) : shaped;
}

export function pruneEmpty(root: Node, f: ItemFilter, keep?: Key | null): Node {
  const prune = (n: Node): Node | null => {
    const children = n.children.map(prune).filter((c): c is Node => c !== null);
    const kept = n.path === '' || children.length > 0 || nodeKey(n) === keep || n.items.some((i) => passes(i, f));
    return kept ? { ...n, children } : null;
  };
  return prune(root) ?? root;
}

function shapeTree(root: Node, o: ViewOpts): Node {
  const gone = (n: Node) => isHidden(n) && !o.showHidden;
  const dropped = (n: Node) => o.mode === 'nodes' && !n.managed && n.path !== '';
  const skippedOf = (n: Node): string[] => {
    const parts = n.via ? n.via.split('/') : [];
    if (parts.length && parts[parts.length - 1] === baseName(n)) parts.pop();
    return parts;
  };
  const collect = (n: Node, skipped: string[]): Node[] => {
    if (gone(n)) return [];
    if (dropped(n)) {
      const next = [...skipped, ...skippedOf(n), baseName(n)];
      return n.children.flatMap((c) => collect(c, next));
    }
    const copy = keep(n);
    if (skipped.length || n.via) copy.via = [...skipped, ...skippedOf(n)].join('/') || null;
    return [copy];
  };
  const keep = (n: Node): Node => ({ ...n, children: n.children.flatMap((c) => collect(c, [])) });
  return keep(root);
}

export const COLOR_NAMES = ['blue', 'green', 'indigo', 'orange', 'pink', 'purple', 'red', 'teal', 'yellow'] as const;
export const COLOR_RE = new RegExp(`^(${COLOR_NAMES.join('|')}|#[0-9A-Fa-f]{6})$`);

export function colorSpec(n: Pick<Node, 'meta'>): string | null {
  const c: unknown = n.meta.color;
  return typeof c === 'string' && COLOR_RE.test(c) ? c : null;
}

export function colorCss(spec: string): string {
  return spec.startsWith('#') ? spec : `var(--${spec})`;
}

export function resolveColor(n: Pick<Node, 'meta'>, parentResolved: string | null, appTint: string): string {
  const own = colorSpec(n);
  if (own) return colorCss(own);
  if (parentResolved) return parentResolved;
  return COLOR_RE.test(appTint) ? colorCss(appTint) : appTint;
}

export function effectiveColors(root: Node, appTint = 'blue'): Map<Key, string> {
  const out = new Map<Key, string>();
  (function walk(n: Node, parent: string | null) {
    const c = isVirtualRoot(n) ? resolveColor({ meta: {} }, null, appTint) : resolveColor(n, parent, appTint);
    if (c) out.set(nodeKey(n), c);
    for (const k of n.children) walk(k, c);
  })(root, null);
  return out;
}
