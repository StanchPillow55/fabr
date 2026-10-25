import type { Node } from './api/types';
import { isOverdue, passes, type Filter, type ItemFilter, type Rollup, type ViewMode } from './model';

export interface ShowPrefs {
  view?: 'tree' | 'list';
  filter?: Filter;
  viewMode: ViewMode;
  showHidden: boolean;
  hideDone: boolean;
  hideEmpty?: boolean;
}

const MODE_LABEL: Record<ViewMode, string> = { nodes: 'Nodes', folders: 'Folders', all: 'All' };
export const FILTERS: { value: Filter; label: string; title: string }[] = [
  { value: 'all', label: 'All', title: 'All items (1)' },
  { value: 'open', label: 'Open', title: 'Open: todo + wip (2)' },
  { value: 'wip', label: 'WIP', title: 'Only wip (3)' },
  { value: 'overdue', label: 'Overdue', title: 'Only overdue (4)' },
];

export function showLabel(p: ShowPrefs): string {
  const parts: string[] = [];
  if (p.view === 'list') parts.push('List');
  if (p.filter && p.filter !== 'all') parts.push(FILTERS.find((f) => f.value === p.filter)!.label);
  if (p.viewMode !== 'folders') parts.push(MODE_LABEL[p.viewMode]);
  if (p.showHidden) parts.push('hidden');
  if (p.hideDone) parts.push('no done');
  if (p.hideEmpty) parts.push('no empty');
  return ['View', ...parts].join(' · ');
}

export type CountTone = 'zero' | 'plain' | 'wip' | 'overdue';
export interface CountPart {
  n: number;
  label: 'open' | 'wip' | 'overdue';
  tone: CountTone;
}

export function countParts(r: Pick<Rollup, 'todo' | 'wip' | 'overdue'>): CountPart[] {
  const open = r.todo + r.wip;
  return [
    { n: open, label: 'open', tone: open ? 'plain' : 'zero' },
    { n: r.wip, label: 'wip', tone: r.wip ? 'wip' : 'zero' },
    { n: r.overdue, label: 'overdue', tone: r.overdue ? 'overdue' : 'zero' },
  ];
}

export function shownCounts(root: Node, f: ItemFilter): Pick<Rollup, 'todo' | 'wip' | 'overdue'> {
  const r = { todo: 0, wip: 0, overdue: 0 };
  (function walk(n: Node) {
    for (const i of n.items) {
      if (!passes(i, f)) continue;
      if (i.status === 'todo') r.todo++;
      else if (i.status === 'wip') r.wip++;
      if (isOverdue(i, f.today, f.now)) r.overdue++;
    }
    n.children.forEach(walk);
  })(root);
  return r;
}

export function countsLabel(r: Pick<Rollup, 'todo' | 'wip' | 'overdue'>): string {
  return countParts(r).map((p) => `${p.n} ${p.label}`).join(' · ');
}

export interface PathSegment {
  label: string;
  key: string;
}

export function crumbSegments(at: { source: string; path: string } | null, sourceName: (id: string) => string): PathSegment[] {
  const out: PathSegment[] = [{ label: '/', key: ':' }];
  if (!at || at.source === '') return out;
  out.push({ label: sourceName(at.source) || at.source, key: `${at.source}:` });
  const parts = at.path.split('/').filter(Boolean);
  parts.forEach((p, i) => out.push({ label: p, key: `${at.source}:${parts.slice(0, i + 1).join('/')}` }));
  return out;
}

export type KeyAction =
  | { type: 'filter'; value: Filter }
  | { type: 'view'; value: 'tree' | 'list' }
  | { type: 'toggleHidden' }
  | { type: 'toggleDone' }
  | { type: 'toggleEmpty' }
  | { type: 'rescan' }
  | { type: 'fit' }
  | { type: 'search' }
  | { type: 'help' }
  | { type: 'escape' };

export function keyAction(e: { key: string; ctrlKey?: boolean; metaKey?: boolean; altKey?: boolean }, typing: boolean): KeyAction | null {
  if (typing) return null;
  if (e.ctrlKey || e.metaKey || e.altKey) return null;
  if (e.key === 'Escape') return { type: 'escape' };
  if (e.key >= '1' && e.key <= '4') return { type: 'filter', value: FILTERS[Number(e.key) - 1].value };
  switch (e.key) {
    case 't': return { type: 'view', value: 'tree' };
    case 'l': return { type: 'view', value: 'list' };
    case 'h': return { type: 'toggleHidden' };
    case 'd': return { type: 'toggleDone' };
    case 'e': return { type: 'toggleEmpty' };
    case 'r': return { type: 'rescan' };
    case 'f': return { type: 'fit' };
    case '/': return { type: 'search' };
    case '?': return { type: 'help' };
  }
  return null;
}

export const SHORTCUTS: { keys: string[]; what: string }[] = [
  { keys: ['1', '2', '3', '4'], what: 'See: All, Open, WIP, Overdue' },
  { keys: ['t', 'l'], what: 'Tree view, List view' },
  { keys: ['h'], what: 'Show or hide hidden nodes' },
  { keys: ['d'], what: 'Show or hide done items' },
  { keys: ['e'], what: 'Show or hide empty nodes (no todo matching See)' },
  { keys: ['r'], what: 'Rescan the directories' },
  { keys: ['f'], what: 'Fit the tree to the window' },
  { keys: ['/'], what: 'Search' },
  { keys: ['Enter'], what: 'In search: jump to the next match' },
  { keys: ['Esc'], what: 'Close this, the View menu or the panel; leave a field' },
  { keys: ['?'], what: 'This list' },
];
