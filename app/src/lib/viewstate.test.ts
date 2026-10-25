import { describe, expect, it } from 'vitest';
import { countParts, countsLabel, crumbSegments, shownCounts, keyAction, SHORTCUTS, showLabel } from './viewstate';

describe('showLabel', () => {
  const base = { view: 'tree' as const, filter: 'all' as const, viewMode: 'folders' as const, showHidden: false, hideDone: false };
  it('is plain View at the defaults', () => {
    expect(showLabel(base)).toBe('View');
    expect(showLabel({ viewMode: 'folders', showHidden: false, hideDone: false })).toBe('View');
  });
  it('names each non-default setting', () => {
    expect(showLabel({ ...base, view: 'list' })).toBe('View · List');
    expect(showLabel({ ...base, filter: 'open' })).toBe('View · Open');
    expect(showLabel({ ...base, filter: 'wip' })).toBe('View · WIP');
    expect(showLabel({ ...base, filter: 'overdue' })).toBe('View · Overdue');
    expect(showLabel({ ...base, viewMode: 'nodes' })).toBe('View · Nodes');
    expect(showLabel({ ...base, viewMode: 'all' })).toBe('View · All');
    expect(showLabel({ ...base, showHidden: true })).toBe('View · hidden');
    expect(showLabel({ ...base, hideDone: true })).toBe('View · no done');
    expect(showLabel({ ...base, hideEmpty: true })).toBe('View · no empty');
    expect(showLabel({ ...base, hideEmpty: false })).toBe('View');
  });
  it('combines them with a dot', () => {
    expect(showLabel({ view: 'list', filter: 'open', viewMode: 'nodes', showHidden: true, hideDone: true })).toBe('View · List · Open · Nodes · hidden · no done');
    expect(showLabel({ ...base, filter: 'wip', hideDone: true, hideEmpty: true })).toBe('View · WIP · no done · no empty');
    expect(showLabel({ ...base, view: 'list', filter: 'open' })).toBe('View · List · Open');
  });
});

describe('countsLabel / countParts', () => {
  it('counts open as todo + wip', () => {
    expect(countsLabel({ todo: 28, wip: 6, overdue: 2 })).toBe('34 open · 6 wip · 2 overdue');
  });
  it('keeps zeros, marked quiet', () => {
    expect(countsLabel({ todo: 0, wip: 0, overdue: 0 })).toBe('0 open · 0 wip · 0 overdue');
    expect(countParts({ todo: 0, wip: 0, overdue: 0 }).map((p) => p.tone)).toEqual(['zero', 'zero', 'zero']);
  });
  it('tints wip and overdue only above zero', () => {
    expect(countParts({ todo: 3, wip: 1, overdue: 2 }).map((p) => p.tone)).toEqual(['plain', 'wip', 'overdue']);
    expect(countParts({ todo: 3, wip: 0, overdue: 0 }).map((p) => p.tone)).toEqual(['plain', 'zero', 'zero']);
  });
});

describe('shownCounts', () => {
  const it_ = (status: 'todo' | 'wip' | 'done', due: string | null = null) => ({ status, due, title: 'x', pri: null }) as never;
  const node = (items: never[], children: unknown[] = []) => ({ source: 's', path: '', name: 'n', items, children, managed: true, meta: {} }) as never;
  const tree = node([it_('todo'), it_('wip', '2026-01-01'), it_('done', '2026-01-01')], [node([it_('todo', '2026-01-01'), it_('wip')])]);
  const f = (filter: 'all' | 'open' | 'wip' | 'overdue', hideDone = false) => ({ today: '2026-06-01', filter, hideDone });
  it('counts the whole subtree', () => expect(shownCounts(tree, f('all'))).toEqual({ todo: 2, wip: 2, overdue: 2 }));
  it('follows the See filter', () => {
    expect(shownCounts(tree, f('wip'))).toEqual({ todo: 0, wip: 2, overdue: 1 });
    expect(shownCounts(tree, f('overdue'))).toEqual({ todo: 1, wip: 1, overdue: 2 });
    expect(countsLabel(shownCounts(tree, f('wip')))).toBe('2 open · 2 wip · 1 overdue');
  });
});

describe('crumbSegments', () => {
  const name = (id: string) => ({ s1: 'Laptop', s2: 'Server' })[id] ?? '';
  it('is just / at the root', () => {
    expect(crumbSegments(null, name)).toEqual([{ label: '/', key: ':' }]);
    expect(crumbSegments({ source: '', path: '' }, name)).toEqual([{ label: '/', key: ':' }]);
  });
  it('adds the source root of a single source', () => {
    expect(crumbSegments({ source: 's1', path: '' }, name)).toEqual([{ label: '/', key: ':' }, { label: 'Laptop', key: 's1:' }]);
  });
  it('adds one segment per path part for a nested focus', () => {
    expect(crumbSegments({ source: 's2', path: 'work/api-server' }, name)).toEqual([
      { label: '/', key: ':' },
      { label: 'Server', key: 's2:' },
      { label: 'work', key: 's2:work' },
      { label: 'api-server', key: 's2:work/api-server' },
    ]);
  });
  it('falls back to the source id without a name', () => {
    expect(crumbSegments({ source: 'x', path: 'a' }, name)[1]).toEqual({ label: 'x', key: 'x:' });
  });
});

describe('SHORTCUTS', () => {
  it('lists every mapped key', () => {
    const keys = SHORTCUTS.flatMap((s) => s.keys);
    for (const k of ['1', '2', '3', '4', 't', 'l', 'h', 'd', 'r', 'f', '/', '?', 'Esc', 'Enter']) expect(keys).toContain(k);
  });
});

describe('keyAction', () => {
  const k = (key: string, o = {}) => keyAction({ key, ...o }, false);
  it('maps 1-4 to the status chips', () => {
    expect(['1', '2', '3', '4'].map((x) => k(x))).toEqual([
      { type: 'filter', value: 'all' }, { type: 'filter', value: 'open' }, { type: 'filter', value: 'wip' }, { type: 'filter', value: 'overdue' },
    ]);
    expect(k('5')).toBeNull();
  });
  it('maps the letters', () => {
    expect(k('t')).toEqual({ type: 'view', value: 'tree' });
    expect(k('l')).toEqual({ type: 'view', value: 'list' });
    expect(k('h')).toEqual({ type: 'toggleHidden' });
    expect(k('d')).toEqual({ type: 'toggleDone' });
    expect(k('e')).toEqual({ type: 'toggleEmpty' });
    expect(k('e', { metaKey: true })).toBeNull();
    expect(SHORTCUTS.some((r) => r.keys.includes('e'))).toBe(true);
    expect(k('r')).toEqual({ type: 'rescan' });
    expect(k('f')).toEqual({ type: 'fit' });
    expect(k('/')).toEqual({ type: 'search' });
    expect(k('?')).toEqual({ type: 'help' });
    expect(k('Escape')).toEqual({ type: 'escape' });
    expect(k('x')).toBeNull();
  });
  it('ignores keys while typing and with modifiers', () => {
    expect(keyAction({ key: 'h' }, true)).toBeNull();
    expect(k('r', { metaKey: true })).toBeNull();
    expect(k('r', { ctrlKey: true })).toBeNull();
  });
});
