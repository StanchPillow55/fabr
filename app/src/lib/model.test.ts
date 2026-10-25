import { describe, expect, it } from 'vitest';
import type { Item, Node } from './api/types';
import {
  boxModel, compareItems, computeForceOpen, dayDiff, dueBucket, dueClass, dueLabel, flattenItems,
  groupRows, indexTree, isHidden, isOverdue, matches, nodeKey, passes, rollup, sortRows, compareDue, joinDue, dueDate, dueTime, visibleTree,
  colorSpec, effectiveColors, resolveColor, isVirtualRoot, rootCounts, sourceLines, errorParts, ROOT_KEY, cardTitle, cardAside,
} from './model';
import { BARS, COUNTS, HEAD, LIST_PAD, MAX_ROWS, ROLLUP, ROW } from './constants';

const TODAY = '2026-10-19';
const it_ = (o: Partial<Item> = {}): Item => ({ id: 'r', phase: 'todo', due: null, title: 't', pri: 0, ...o });
const node = (path: string, o: Partial<Node> = {}): Node => ({
  source: 'local', path, name: path.split('/').pop() || 'root', managed: true, meta: {}, items: [],
  warnings: [], version: 'v', via: null, children: [], ...o,
});
const none = new Set<string>();
const ctx = (o = {}) => ({ today: TODAY, filter: 'all' as const, hideDone: false, collapsed: none, moreOpen: none, forceOpen: none, ...o });

describe('dates', () => {
  it('dayDiff', () => {
    expect(dayDiff('2026-10-20', TODAY)).toBe(1);
    expect(dayDiff('2026-10-19', '2026-10-20')).toBe(-1);
    expect(dayDiff('2027-01-01', '2026-12-31')).toBe(1);
  });
  it('dueClass', () => {
    expect(dueClass(it_({ due: '2026-10-18' }), TODAY)).toBe('overdue');
    expect(dueClass(it_({ due: '2026-10-22' }), TODAY)).toBe('soon');
    expect(dueClass(it_({ due: '2026-10-23' }), TODAY)).toBe('');
    expect(dueClass(it_({ due: '2026-10-01', phase: 'done' }), TODAY)).toBe('');
    expect(dueClass(it_(), TODAY)).toBe('');
  });
  it('dueLabel', () => {
    expect(dueLabel(it_({ due: TODAY }), TODAY)).toBe('today');
    expect(dueLabel(it_({ due: '2026-10-20' }), TODAY)).toBe('tomorrow');
    expect(dueLabel(it_({ due: '2026-12-01' }), TODAY)).toBe('12-01');
    expect(dueLabel(it_({ due: '2027-01-05' }), TODAY)).toBe('2027-01-05');
    expect(dueLabel(it_({ due: TODAY, phase: 'done' }), TODAY)).toBe('10-19');
    expect(dueLabel(it_(), TODAY)).toBe('');
  });
  it('isOverdue', () => {
    expect(isOverdue(it_({ due: '2026-10-18' }), TODAY)).toBe(true);
    expect(isOverdue(it_({ due: TODAY }), TODAY)).toBe(false);
    expect(isOverdue(it_({ due: '2026-10-18', phase: 'done' }), TODAY)).toBe(false);
  });
});

describe('filters', () => {
  const f = (filter: any, hideDone = false) => ({ today: TODAY, filter, hideDone });
  it('passes', () => {
    expect(passes(it_({ phase: 'done' }), f('all', true))).toBe(false);
    expect(passes(it_({ phase: 'done' }), f('all'))).toBe(true);
    expect(passes(it_({ phase: 'done' }), f('open'))).toBe(false);
    expect(passes(it_({ phase: 'working' }), f('wip'))).toBe(true);
    expect(passes(it_({ phase: 'todo' }), f('wip'))).toBe(false);
    expect(passes(it_({ due: '2026-10-01' }), f('overdue'))).toBe(true);
    expect(passes(it_(), f('overdue'))).toBe(false);
  });
  it('matches', () => {
    const n = node('a/b', { name: 'Budget', items: [it_({ title: 'Pay Rent' })] });
    expect(matches(n, 'budg')).toBe(true);
    expect(matches(n, 'A/B')).toBe(true);
    expect(matches(n, 'rent')).toBe(true);
    expect(matches(n, 'zzz')).toBe(false);
    expect(matches(n, '')).toBe(false);
  });
});

describe('tree', () => {
  const leaf = node('a/b', { items: [it_({ title: 'needle' })] });
  const mid = node('a', { children: [leaf], items: [it_({ phase: 'working', due: '2026-10-25' }), it_({ phase: 'done' }), it_({ due: '2026-10-01' })] });
  const root = node('', { children: [mid] });
  it('indexTree and nodeKey', () => {
    const ix = indexTree(root);
    expect(nodeKey(leaf)).toBe('local:a/b');
    expect(nodeKey('x', '')).toBe('x:');
    expect(ix.byKey.get('local:a/b')).toBe(leaf);
    expect(ix.parentOf.get('local:a/b')).toBe(mid);
    expect(ix.parentOf.has('local:')).toBe(false);
  });
  it('rollup', () => {
    const r = rollup(root, TODAY);
    expect(r).toEqual({ todo: 2, active: 1, needs_you: 0, done: 1, suspended: 0, overdue: 1, next: '2026-10-01', nodes: 2 });
  });
  it('computeForceOpen opens ancestors of matches', () => {
    const fo = computeForceOpen(indexTree(root), 'needle');
    expect([...fo].sort()).toEqual(['local:', 'local:a']);
    expect(computeForceOpen(indexTree(root), '').size).toBe(0);
  });
});

describe('boxModel', () => {
  it('heights by state', () => {
    const empty = node('x');
    expect(boxModel(empty, ctx()).h).toBe(HEAD + COUNTS + LIST_PAD + 2 * ROW + 2);
    const plain = node('p', { managed: false });
    expect(boxModel(plain, ctx()).h).toBe(HEAD + 2);
    const many = node('m', { items: Array.from({ length: 12 }, (_, i) => it_({ title: 'i' + i })), children: [] });
    const b = boxModel(many, ctx());
    expect(b.shown.length).toBe(MAX_ROWS);
    expect(b.more).toBe(4);
    expect(b.h).toBe(HEAD + COUNTS + LIST_PAD + ROW + MAX_ROWS * ROW + ROW + 2);
    const open = boxModel(many, ctx({ moreOpen: new Set(['local:m']) }));
    expect(open.shown.length).toBe(12);
    expect(open.h).toBe(HEAD + COUNTS + LIST_PAD + ROW + 12 * ROW + ROW + 2);
  });
  it('active agents add the bars row only on initialised cards', () => {
    const n = node('a');
    expect(boxModel(n, ctx(), 2).h).toBe(boxModel(n, ctx()).h + BARS);
    expect(boxModel(node('p', { managed: false }), ctx(), 2).h).toBe(HEAD + 2);
  });
  it('collapsed adds rollup row and drops list', () => {
    const n = node('c', { items: [it_()] });
    const b = boxModel(n, ctx({ collapsed: new Set(['local:c']) }));
    expect(b.collapsed).toBe(true);
    expect(b.h).toBe(HEAD + COUNTS + ROLLUP + 2);
  });
  it('hidden meta no longer collapses a card; only the collapsed set does (unless force-open)', () => {
    const n = node('h', { meta: { hidden: true }, items: [it_()] });
    expect(boxModel(n, ctx()).collapsed).toBe(false);
    expect(boxModel(n, ctx({ collapsed: new Set(['local:h']) })).collapsed).toBe(true);
    expect(boxModel(n, ctx({ collapsed: new Set(['local:h']), forceOpen: new Set(['local:h']) })).collapsed).toBe(false);
  });
  it('filter shrinks visible rows', () => {
    const n = node('f', { items: [it_({ phase: 'done' }), it_()] });
    expect(boxModel(n, ctx({ hideDone: true })).visible.length).toBe(1);
  });
});

describe('priority and list', () => {
  it('compareItems: pri then due then title, done last', () => {
    const items = [
      it_({ title: 'z', phase: 'done', pri: 1 }),
      it_({ title: 'none' }),
      it_({ title: 'p3', pri: 3 }),
      it_({ title: 'p1late', pri: 1, due: '2026-12-01' }),
      it_({ title: 'p1soon', pri: 1, due: '2026-11-01' }),
      it_({ title: 'p1nodue', pri: 1 }),
      it_({ title: 'p2', pri: 2 }),
    ];
    expect(items.sort(compareItems).map((i) => i.title)).toEqual(['p1soon', 'p1late', 'p1nodue', 'p2', 'p3', 'none', 'z']);
  });
  it('dueBucket', () => {
    expect(dueBucket(it_({ due: '2026-10-18' }), TODAY)).toBe('overdue');
    expect(dueBucket(it_({ due: TODAY }), TODAY)).toBe('today');
    expect(dueBucket(it_({ due: '2026-10-26' }), TODAY)).toBe('week');
    expect(dueBucket(it_({ due: '2026-10-27' }), TODAY)).toBe('later');
    expect(dueBucket(it_(), TODAY)).toBe('none');
    expect(dueBucket(it_({ due: '2026-10-01', phase: 'done' }), TODAY)).toBe('done');
  });
  const root = node('', {
    children: [node('a', { source: 'local', items: [it_({ title: 'b', pri: 2 }), it_({ title: 'a', phase: 'working', due: '2026-10-01' })] })],
    items: [it_({ title: 'c', phase: 'done' })],
  });
  const f = { today: TODAY, filter: 'all' as const, hideDone: false };
  it('flattenItems', () => {
    const rows = flattenItems(root, f);
    expect(rows.map((r) => r.item.title)).toEqual(['c', 'b', 'a']);
    expect(rows[1].id).toBe('local:a#0');
    expect(flattenItems(root, { ...f, hideDone: true }).length).toBe(2);
    expect(flattenItems(root, f, 'B').map((r) => r.item.title)).toEqual(['b']);
  });
  it('sortRows and groupRows', () => {
    const rows = flattenItems(root, f);
    expect(sortRows(rows, 'title', 'asc', TODAY).map((r) => r.item.title)).toEqual(['a', 'b', 'c']);
    expect(sortRows(rows, 'title', 'desc', TODAY).map((r) => r.item.title)).toEqual(['c', 'b', 'a']);
    expect(sortRows(rows, 'due', 'asc', TODAY)[0].item.title).toBe('a');
    expect(sortRows(rows, 'due', 'desc', TODAY)[0].item.title).toBe('a');
    expect(sortRows(rows, 'pri', 'asc', TODAY)[0].item.title).toBe('b');
    expect(sortRows(rows, 'status', 'asc', TODAY).map((r) => r.item.phase)).toEqual(['working', 'todo', 'done']);
    expect(groupRows(rows, 'none', TODAY)).toHaveLength(1);
    expect(groupRows(rows, 'status', TODAY).map((g) => g.id)).toEqual(['working', 'todo', 'done']);
    expect(groupRows(rows, 'due', TODAY).map((g) => g.id)).toEqual(['overdue', 'none', 'done']);
    expect(groupRows(rows, 'node', TODAY).map((g) => g.id)).toEqual(['local:', 'local:a']);
  });
});

describe('due time', () => {
  const NOW = '2026-10-19T14:30';
  it('splits and joins', () => {
    expect(dueDate('2026-10-21T14:30')).toBe('2026-10-21');
    expect(dueTime('2026-10-21T14:30')).toBe('14:30');
    expect(dueTime('2026-10-21')).toBeNull();
    expect(joinDue('2026-10-21', '09:05')).toBe('2026-10-21T09:05');
    expect(joinDue('2026-10-21', '')).toBe('2026-10-21');
    expect(joinDue('2026-10-21', null)).toBe('2026-10-21');
  });
  it('dayDiff ignores the time', () => {
    expect(dayDiff('2026-10-20T23:59', TODAY)).toBe(1);
    expect(dayDiff('2026-10-19T00:00', '2026-10-19')).toBe(0);
  });
  it('date-only is overdue after the end of that day; date-time after that minute', () => {
    expect(isOverdue(it_({ due: TODAY }), TODAY, NOW)).toBe(false);
    expect(isOverdue(it_({ due: '2026-10-18' }), TODAY, NOW)).toBe(true);
    expect(isOverdue(it_({ due: '2026-10-19T14:29' }), TODAY, NOW)).toBe(true);
    expect(isOverdue(it_({ due: '2026-10-19T14:30' }), TODAY, NOW)).toBe(false);
    expect(isOverdue(it_({ due: '2026-10-19T14:31' }), TODAY, NOW)).toBe(false);
    expect(isOverdue(it_({ due: '2026-10-19T09:00', phase: 'done' }), TODAY, NOW)).toBe(false);
    expect(isOverdue(it_({ due: '2026-10-18T23:59' }), TODAY, undefined)).toBe(true);
    expect(isOverdue(it_({ due: '2999-01-01T00:00' }), '2999-01-01', undefined)).toBe(false);
  });
  it('dueClass', () => {
    expect(dueClass(it_({ due: '2026-10-19T08:00' }), TODAY, NOW)).toBe('overdue');
    expect(dueClass(it_({ due: '2026-10-19T20:00' }), TODAY, NOW)).toBe('soon');
    expect(dueClass(it_({ due: '2026-10-23T08:00' }), TODAY, NOW)).toBe('');
    expect(dueClass(it_({ due: '2026-10-19T08:00', phase: 'done' }), TODAY, NOW)).toBe('');
  });
  it('dueLabel', () => {
    expect(dueLabel(it_({ due: '2026-10-21' }), TODAY)).toBe('10-21');
    expect(dueLabel(it_({ due: '2026-10-21T14:30' }), TODAY)).toBe('10-21 14:30');
    expect(dueLabel(it_({ due: '2026-10-19T14:30' }), TODAY)).toBe('today 14:30');
    expect(dueLabel(it_({ due: '2026-10-20' }), TODAY)).toBe('tomorrow');
    expect(dueLabel(it_({ due: '2026-10-20T08:00' }), TODAY)).toBe('tomorrow 08:00');
    expect(dueLabel(it_({ due: '2027-01-05T08:00' }), TODAY)).toBe('2027-01-05 08:00');
    expect(dueLabel(it_({ due: '2026-10-19T14:30', phase: 'done' }), TODAY)).toBe('10-19 14:30');
    expect(dueLabel(it_({ due: '2026-10-21T14:30' }), TODAY, true)).toBe('2026-10-21 14:30');
    expect(dueLabel(it_({ due: '2026-10-21' }), TODAY, true)).toBe('2026-10-21');
  });
  it('compareDue: timed before date-only on one day, none last', () => {
    const v = ['2026-10-21', null, '2026-10-21T14:30', '2026-10-20', '2026-10-21T09:00'];
    expect([...v].sort(compareDue)).toEqual(['2026-10-20', '2026-10-21T09:00', '2026-10-21T14:30', '2026-10-21', null]);
  });
  it('compareItems and sortRows use it', () => {
    const items = [it_({ title: 'a', due: '2026-10-21' }), it_({ title: 'b', due: '2026-10-21T09:00' }), it_({ title: 'c' }), it_({ title: 'd', due: '2026-10-20' })];
    expect([...items].sort(compareItems).map((i) => i.title)).toEqual(['d', 'b', 'a', 'c']);
    const rows = items.map((item, index) => ({ id: String(index), nodeKey: 'local:', source: 'local', path: '', nodeName: 'r', index, item }));
    expect(sortRows(rows, 'due', 'asc', TODAY).map((r) => r.item.title)).toEqual(['d', 'b', 'a', 'c']);
    expect(sortRows(rows, 'due', 'desc', TODAY).map((r) => r.item.title)).toEqual(['a', 'b', 'd', 'c']);
  });
  it('dueBucket and filters see the time', () => {
    expect(dueBucket(it_({ due: '2026-10-19T08:00' }), TODAY, NOW)).toBe('overdue');
    expect(dueBucket(it_({ due: '2026-10-19T18:00' }), TODAY, NOW)).toBe('today');
    expect(dueBucket(it_({ due: TODAY }), TODAY, NOW)).toBe('today');
    expect(passes(it_({ due: '2026-10-19T08:00' }), { today: TODAY, now: NOW, filter: 'overdue', hideDone: false })).toBe(true);
    expect(passes(it_({ due: '2026-10-19T18:00' }), { today: TODAY, now: NOW, filter: 'overdue', hideDone: false })).toBe(false);
  });
  it('rollup: overdue by minute, next is the earliest due', () => {
    const n = node('', { items: [it_({ due: '2026-10-19T08:00' }), it_({ due: '2026-10-19' }), it_({ due: '2026-10-19T20:00' })] });
    const r = rollup(n, TODAY, NOW);
    expect(r.overdue).toBe(1);
    expect(r.next).toBe('2026-10-19T08:00');
  });
});

describe('isHidden', () => {
  it('accepts a boolean and a hand-edited "true" string, nothing else', () => {
    expect(isHidden({ meta: { hidden: true } })).toBe(true);
    expect(isHidden({ meta: { hidden: 'true' } })).toBe(true);
    expect(isHidden({ meta: { hidden: 'True ' } })).toBe(true);
    expect(isHidden({ meta: { hidden: false } })).toBe(false);
    expect(isHidden({ meta: { hidden: 'no' } })).toBe(false);
    expect(isHidden({ meta: {} })).toBe(false);
  });
  it('a "true" string hides in the tree and shows again with Show hidden', () => {
    const arch = node('arch', { meta: { hidden: 'true' } });
    const r = node('', { children: [node('work'), arch] });
    const ps = (n: Node): string[] => [n.path, ...n.children.flatMap(ps)];
    expect(ps(visibleTree(r, { mode: 'folders', showHidden: false }))).toEqual(['', 'work']);
    expect(ps(visibleTree(r, { mode: 'folders', showHidden: true }))).toEqual(['', 'work', 'arch']);
  });
});

describe('view modes and hidden nodes', () => {
  const a = node('work/plain/a', { via: null, items: [it_({ title: 'in a' })] });
  const plain = node('work/plain', { managed: false, children: [a], via: null });
  const work = node('work', { children: [plain], items: [it_({ phase: 'working' })] });
  const b = node('plain2/deep/b', { via: 'x' });
  const deep = node('plain2/deep', { managed: false, children: [b] });
  const plain2 = node('plain2', { managed: false, children: [deep] });
  const old = node('arch/old', { items: [it_({ title: 'old stuff' })] });
  const arch = node('arch', { meta: { hidden: true }, children: [old], items: [it_({ title: 'arch item' })] });
  const root = node('', { children: [work, plain2, arch] });
  const paths = (n: Node): string[] => [n.path, ...n.children.flatMap(paths)];

  it('folders keeps plain dirs; nodes drops them and re-parents with via', () => {
    expect(paths(visibleTree(root, { mode: 'folders', showHidden: true }))).toEqual(['', 'work', 'work/plain', 'work/plain/a', 'plain2', 'plain2/deep', 'plain2/deep/b', 'arch', 'arch/old']);
    const t = visibleTree(root, { mode: 'nodes', showHidden: true });
    expect(paths(t)).toEqual(['', 'work', 'work/plain/a', 'plain2/deep/b', 'arch', 'arch/old']);
    const t2 = visibleTree(root, { mode: 'nodes', showHidden: true });
    expect(t2.children.find((c) => c.path === 'work')!.children[0].via).toBe('plain');
    const only = visibleTree(node('', { children: [plain2] }), { mode: 'nodes', showHidden: false });
    expect(only.children.map((c) => [c.path, c.via])).toEqual([['plain2/deep/b', 'plain2/deep/x']]);
  });
  it('nodes mode keeps source roots even when plain', () => {
    const r = node('', { source: '', name: 'All sources', managed: false, children: [node('', { source: 'a', managed: false, children: [node('x', { source: 'a' })] })] });
    expect(paths(visibleTree(r, { mode: 'nodes', showHidden: false }))).toEqual(['', '', 'x']);
  });
  it('all mode is the same tree as folders client-side', () => {
    expect(paths(visibleTree(root, { mode: 'all', showHidden: true }))).toEqual(paths(visibleTree(root, { mode: 'folders', showHidden: true })));
  });
  it('hidden drops the node and its whole subtree; Show hidden brings it back', () => {
    for (const mode of ['nodes', 'folders', 'all'] as const) {
      const t = visibleTree(root, { mode, showHidden: false });
      expect(paths(t)).not.toContain('arch');
      expect(paths(t)).not.toContain('arch/old');
      expect(paths(visibleTree(root, { mode, showHidden: true }))).toContain('arch/old');
    }
  });
  it('list, search, rollup and counts skip hidden subtrees', () => {
    const f = { today: TODAY, filter: 'all' as const, hideDone: false };
    const shown = visibleTree(root, { mode: 'nodes', showHidden: false });
    const titles = (r: Node, q = '') => flattenItems(r, f, q).map((x) => x.item.title);
    expect(titles(shown)).not.toContain('old stuff');
    expect(titles(shown)).not.toContain('arch item');
    expect(titles(shown, 'old')).toEqual([]);
    const all = visibleTree(root, { mode: 'nodes', showHidden: true });
    expect(titles(all, 'old')).toEqual(['old stuff']);
    expect(rollup(shown, TODAY).nodes).toBe(3);
    expect(rollup(shown, TODAY).todo + rollup(shown, TODAY).active).toBe(2);
    expect(rollup(all, TODAY).nodes).toBe(5);
    expect(indexTree(shown).byKey.has('local:arch')).toBe(false);
    expect([...computeForceOpen(indexTree(shown), 'old')]).toEqual([]);
    expect([...computeForceOpen(indexTree(all), 'old')].sort()).toEqual(['local:', 'local:arch']);
  });
  it('a hidden root is still the root', () => {
    expect(paths(visibleTree(arch, { mode: 'nodes', showHidden: false }))).toEqual(['arch', 'arch/old']);
  });
});

describe('accent colour', () => {
  it('resolveColor: own name / hex, else parent, else app tint', () => {
    expect(resolveColor(node('a', { meta: { color: 'pink' } }), '#123456', 'blue')).toBe('var(--pink)');
    expect(resolveColor(node('a', { meta: { color: '#12aB9f' } }), 'var(--teal)', 'blue')).toBe('#12aB9f');
    expect(resolveColor(node('a'), 'var(--teal)', 'blue')).toBe('var(--teal)');
    expect(resolveColor(node(''), null, 'green')).toBe('var(--green)');
    expect(resolveColor(node(''), null, 'var(--tint)')).toBe('var(--tint)');
    for (const bad of ['Pink', '#fff', 'magenta', '', 3, true]) {
      expect(resolveColor(node('a', { meta: { color: bad } }), 'var(--red)', 'blue')).toBe('var(--red)');
      expect(colorSpec(node('a', { meta: { color: bad } }))).toBeNull();
    }
  });

  it('effectiveColors: inherit, override, invalid -> parent, root -> app tint', () => {
    const root = node('', {
      children: [
        node('research', { meta: { color: 'pink' }, children: [node('research/p', { children: [node('research/p/f', { managed: false })] })] }),
        node('work', { children: [node('work/api', { meta: { color: 'teal' }, children: [node('work/api/docs', { meta: { color: 'nope' } })] })] }),
      ],
    });
    const c = effectiveColors(root, 'indigo');
    expect(c.get('local:')).toBe('var(--indigo)');
    expect(c.get('local:research')).toBe('var(--pink)');
    expect(c.get('local:research/p')).toBe('var(--pink)');
    expect(c.get('local:research/p/f')).toBe('var(--pink)');
    expect(c.get('local:work')).toBe('var(--indigo)');
    expect(c.get('local:work/api')).toBe('var(--teal)');
    expect(c.get('local:work/api/docs')).toBe('var(--teal)');
  });

  it('effectiveColors: the virtual / root takes the app tint; each source root starts from it', () => {
    const v = node('', { source: '', managed: false, name: '/', children: [node('', { source: 'a', meta: { color: '#00ff00' } }), node('', { source: 'b' })] });
    const c = effectiveColors(v);
    expect(c.get(':')).toBe('var(--blue)');
    expect(effectiveColors(v, 'pink').get(':')).toBe('var(--pink)');
    expect(c.get('a:')).toBe('#00ff00');
    expect(c.get('b:')).toBe('var(--blue)');
  });
});

describe('the / root', () => {
  const tree = (): Node => node('', {
    source: '', managed: false, name: '/', version: null, children: [
      node('', { source: 'a', items: [it_(), it_({ phase: 'working', due: '2026-10-01' })], children: [node('x', { source: 'a', items: [it_({ phase: 'done' }), it_({ due: '2026-10-18' })] })] }),
      node('', { source: 'b', name: 'b', managed: false, version: null, warnings: ['ssh: timeout'] }),
    ],
  });
  it('isVirtualRoot / ROOT_KEY', () => {
    expect(isVirtualRoot(tree())).toBe(true);
    expect(isVirtualRoot(tree().children[0])).toBe(false);
    expect(nodeKey(tree())).toBe(ROOT_KEY);
  });
  it('rootCounts: todo and active apart, overdue over the whole tree (the rollup of collapsed cards)', () => {
    expect(rootCounts(tree(), TODAY)).toEqual({ todo: 2, active: 1, overdue: 2 });
    const r = rollup(tree(), TODAY);
    expect(rootCounts(tree(), TODAY)).toEqual({ todo: r.todo, active: r.active, overdue: r.overdue });
  });
  it('sourceLines: name, host or local, open count and the source error', () => {
    const sources = [
      { id: 'a', name: 'Alpha', host: null, root: '/x', prune: null },
      { id: 'b', name: 'Beta', host: 'me@box', root: '~', prune: null },
    ];
    expect(sourceLines(tree(), sources, [{ source: 'b', message: 'ssh: timeout' }], TODAY)).toEqual([
      { key: 'a:', source: 'a', name: 'Alpha', where: 'local', open: 3, error: null, pending: false },
      { key: 'b:', source: 'b', name: 'Beta', where: 'me@box', open: 0, error: 'ssh: timeout', pending: false },
    ]);
    expect(sourceLines(tree(), sources, [], TODAY, undefined, ['b']).map((l) => l.pending)).toEqual([false, true]);
    expect(sourceLines(tree(), [], [], TODAY).map((l) => [l.name, l.where])).toEqual([['root', 'local'], ['b', 'local']]);
  });
  it('boxModel: the / card is a counts row plus one line per source; collapsed it is a rollup', () => {
    expect(boxModel(tree(), ctx()).h).toBe(HEAD + LIST_PAD + 3 * ROW + 2);
    const b = boxModel(tree(), ctx({ collapsed: new Set([ROOT_KEY]) }));
    expect(b.collapsed).toBe(true);
    expect(b.h).toBe(HEAD + ROLLUP + 2);
  });
});

describe('card head (rename regression)', () => {
  const src = { id: 'b', name: 'Beta', host: null };
  it('a source root in a multi-source tree shows its Identity name, not the source name', () => {
    const root = node('', { source: 'b', name: 'Beta', meta: { name: 'Renamed' } });
    expect(cardTitle(root)).toBe('Renamed');
    expect(cardAside(root, src)).toBe('Beta · local');
    expect(cardAside(root, { ...src, host: 'me@box' })).toBe('Beta · me@box');
  });
  it('without a meta name the source root shows the source name once', () => {
    const root = node('', { source: 'b', name: 'Beta' });
    expect(cardTitle(root)).toBe('Beta');
    expect(cardAside(root, src)).toBe('local');
  });
  it('nested nodes: meta.name, else the directory name; aside = kind', () => {
    expect(cardTitle(node('work/api', { meta: { name: 'API', kind: 'project' } }))).toBe('API');
    expect(cardTitle(node('work/api'))).toBe('api');
    expect(cardAside(node('work/api', { meta: { kind: 'project' } }), null)).toBe('project');
  });
  it('the / root', () => {
    const v = node('', { source: '', name: '/', managed: false });
    expect([cardTitle(v), cardAside(v, null)]).toEqual(['/', 'all sources']);
  });
});

describe('visibleTree hideEmpty', () => {
  const f = (o = {}) => ({ today: TODAY, filter: 'all' as const, hideDone: false, ...o });
  const paths = (n: Node): string[] => [`${n.source}:${n.path}`, ...n.children.flatMap(paths)];
  const src = (source: string, children: Node[]) => node('', { source, managed: false, children });
  const tree = (o: { hiddenWip?: boolean } = {}) => node('', {
    source: '', managed: false, children: [
      src('local', [
        node('done', { items: [it_({ phase: 'done' })] }),
        node('proj', { children: [node('proj/wip', { items: [it_({ phase: 'working' })], meta: o.hiddenWip ? { hidden: true } : {} })] }),
        node('p1', { managed: false, children: [node('p1/p2', { managed: false, children: [node('p1/p2/x', { items: [it_()] })] })] }),
        node('p3', { managed: false }),
      ]),
      src('empty', []),
    ],
  });
  const vt = (o: Partial<Parameters<typeof visibleTree>[1]> = {}, t = tree()) =>
    paths(visibleTree(t, { mode: 'folders', showHidden: false, hideEmpty: true, items: f(), ...o }));

  it('is off by default (and without an item filter)', () => {
    const all = paths(visibleTree(tree(), { mode: 'folders', showHidden: false }));
    expect(all).toContain('local:p3');
    expect(paths(visibleTree(tree(), { mode: 'folders', showHidden: false, hideEmpty: true }))).toEqual(all);
  });
  it('drops a leaf with only done items when Done is off; keeps it when Done is shown', () => {
    expect(vt({ items: f({ hideDone: true }) })).not.toContain('local:done');
    expect(vt()).toContain('local:done');
    expect(vt({ items: f({ filter: 'open' }) })).not.toContain('local:done');
  });
  it('keeps an ancestor without items when a descendant matches', () => {
    expect(vt({ items: f({ filter: 'wip' }) })).toEqual([':', 'local:', 'local:proj', 'local:proj/wip', 'empty:']);
  });
  it('never drops / or the source roots, even when empty', () => {
    const t = vt({ items: f({ filter: 'overdue' }) });
    expect(t).toEqual([':', 'local:', 'empty:']);
  });
  it('keeps a plain dir chain only when a kept node is below it', () => {
    const t = vt();
    expect(t).toEqual([':', 'local:', 'local:done', 'local:proj', 'local:proj/wip', 'local:p1', 'local:p1/p2', 'local:p1/p2/x', 'empty:']);
    expect(t).not.toContain('local:p3');
    expect(vt({ items: f({ filter: 'wip' }) })).not.toContain('local:p1');
  });
  it('applies after hidden and Nodes mode', () => {
    expect(vt({ items: f({ filter: 'wip' }) }, tree({ hiddenWip: true }))).toEqual([':', 'local:', 'empty:']);
    expect(vt({ items: f({ filter: 'wip' }), showHidden: true }, tree({ hiddenWip: true }))).toContain('local:proj/wip');
    expect(vt({ mode: 'nodes', items: f({ filter: 'open' }) })).toEqual([':', 'local:', 'local:proj', 'local:proj/wip', 'local:p1/p2/x', 'empty:']);
  });
  it('never drops the kept (focused) node', () => {
    expect(vt({ items: f({ filter: 'wip' }), keep: 'local:p3' })).toContain('local:p3');
  });
});

describe('pending sources', () => {
  it('a pending source root (empty, unmanaged) survives "hide empty"', () => {
    const root = node('', { source: '', name: '/', managed: false, children: [
      node('', { source: 'r', name: 'Remote', managed: false, version: null, children: [] }),
    ] });
    const t = visibleTree(root, { mode: 'nodes', showHidden: false, hideEmpty: true, items: { today: TODAY, filter: 'all', hideDone: true } });
    expect(t.children.map((c) => c.source)).toEqual(['r']);
  });
});

describe('errorParts', () => {
  it('splits text and https links; trailing punctuation stays outside', () => {
    expect(errorParts('Log in to Tailscale to reach box: https://login.tailscale.com/a/abc')).toEqual([
      { text: 'Log in to Tailscale to reach box: ' },
      { text: 'https://login.tailscale.com/a/abc', url: 'https://login.tailscale.com/a/abc' },
    ]);
    expect(errorParts('see https://x.example/a, then https://y.example/b.')).toEqual([
      { text: 'see ' },
      { text: 'https://x.example/a', url: 'https://x.example/a' },
      { text: ', then ' },
      { text: 'https://y.example/b', url: 'https://y.example/b' },
      { text: '.' },
    ]);
    expect(errorParts('ssh: timeout')).toEqual([{ text: 'ssh: timeout' }]);
    expect(errorParts('http://plain.example is not linked')).toEqual([{ text: 'http://plain.example is not linked' }]);
    expect(errorParts('')).toEqual([{ text: '' }]);
  });
  it('a source line with a login error carries a link', () => {
    const msg = 'Log in to Tailscale to reach box: https://login.tailscale.com/a/abc';
    const root = node('', { source: '', name: '/', managed: false, children: [node('', { source: 'b', name: 'Beta', managed: false, version: null })] });
    const [line] = sourceLines(root, [{ id: 'b', name: 'Beta', host: 'box', root: '~', prune: null }], [{ source: 'b', message: msg }], TODAY, undefined, ['b']);
    expect(line.error).toBe(msg);
    expect(errorParts(line.error!).filter((p) => p.url).map((p) => p.url)).toEqual(['https://login.tailscale.com/a/abc']);
  });
});

describe('priority 0 means none', () => {
  it('sorts after 1..3 in compareItems and in the pri column', () => {
    const items = [it_({ id: 'a', title: 'a', pri: 0 }), it_({ id: 'b', title: 'b', pri: 3 }), it_({ id: 'c', title: 'c', pri: 1 })];
    expect([...items].sort(compareItems).map((i) => i.id)).toEqual(['c', 'b', 'a']);
  });
});

describe('suspended items', () => {
  const sus = it_({ title: 'later', phase: 'suspended', due: '2026-10-01' });
  const n = node('a', { items: [it_({ title: 'now' }), sus] });
  it('leave the todo, active and overdue counts and never nag', () => {
    expect(rollup(n, TODAY)).toMatchObject({ todo: 1, active: 0, suspended: 1, overdue: 0, next: null });
    expect(isOverdue(sus, TODAY)).toBe(false);
    expect(passes(sus, { today: TODAY, filter: 'open', hideDone: false })).toBe(false);
  });
});
