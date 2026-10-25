import { describe, expect, it, vi } from 'vitest';
import type { Api } from './api/index';
import { ApiError, type Item, type Node, type Tree } from './api/types';
import { cardTitle, flattenItems, rollup } from './model';
import { createMockApi } from '../mock/api';
import { Store, metaPayload, removeSummary } from './store.svelte';

const TODAY = '2026-10-19';
const item = (o: Partial<Item> = {}): Item => ({ id: 'a1', phase: 'todo', due: null, title: 'Write tests', pri: 0, ...o });
const node = (path: string, o: Partial<Node> = {}): Node => ({
  source: 'local', path, name: path.split('/').pop() || 'root', managed: true, meta: {}, items: [],
  warnings: [], version: `v-${path}`, via: null, children: [], ...o,
});
const vroot = (children: Node[]): Node => ({
  source: '', path: '', name: '/', managed: false, meta: {}, items: [], warnings: [], version: null, via: null, children,
});
const makeTree = (version = 'v-work', items: Item[] = [item(), item({ id: 'b2', phase: 'working', title: 'Ship' })]): Tree => ({
  root: vroot([node('', { meta: { name: 'fabr' }, children: [node('work', { version, items }), node('plain', { managed: false, version: null })] })]),
  sources: [{ id: 'local', name: 'Home', host: null, root: '/tmp/x', prune: null }],
  today: TODAY,
  truncated: false,
  errors: [],
});

function mockApi(over: Partial<Api> = {}): Api & { [K in keyof Api]: ReturnType<typeof vi.fn> } {
  const api = {
    getTree: vi.fn(async () => makeTree()),
    getStamp: vi.fn(async () => ({ stamp: 's1' })),
    saveMeta: vi.fn(async (req) => node(req.path, { version: 'v3', meta: req.meta })),
    initNode: vi.fn(async (req) => node(req.path)),
    removeNode: vi.fn(async (req) => node(req.path, { managed: false, version: null })),
    listSources: vi.fn(async () => []),
    saveSources: vi.fn(async (s) => s),
    testSource: vi.fn(async () => ({ ok: true, message: 'ok', latency_ms: 1, node_count: 1 })),
    getSettings: vi.fn(async () => ({ poll_ms: 2000, remote_poll_ms: 30000 })),
    saveSettings: vi.fn(async (s) => s),
    openPath: vi.fn(async () => {}),
    openUrl: vi.fn(async () => {}),
    ...over,
  };
  return api as never;
}

describe('Store', () => {
  it('load indexes the tree', async () => {
    const s = new Store(mockApi());
    await s.load();
    expect(s.today).toBe(TODAY);
    expect([...s.byKey.keys()].sort()).toEqual([':', 'local:', 'local:plain', 'local:work']);
    expect(s.parentOf.get('local:')?.name).toBe('/');
    expect(s.parentOf.get('local:work')?.path).toBe('');
    expect(s.notice).toBeNull();
  });

  it('load reports truncation and errors as a notice', async () => {
    const s = new Store(mockApi({ getTree: vi.fn(async () => ({ ...makeTree(), truncated: true })) }));
    await s.load();
    expect(s.notice).toMatch(/truncated/);
    const s2 = new Store(mockApi({ getTree: vi.fn(async () => { throw new ApiError(500, 'boom'); }) }));
    await s2.load();
    expect(s2.notice).toMatch(/boom/);
  });

  it('has no item mutation path left: every task change is a record operation', () => {
    const s = new Store(mockApi()) as unknown as Record<string, unknown>;
    for (const gone of ['cycleStatus', 'quickAdd', 'commitItems', 'setItemFocus', 'itemFocus']) expect(gone in s).toBe(false);
  });

  it('the node settings reads items from the node, so a reload with new items shows them without a version change', async () => {
    let items = [item()];
    const api = mockApi({ getTree: vi.fn(async () => makeTree('v-work', items)) });
    const s = new Store(api);
    await s.load();
    s.openNodeSettings('local:work');
    expect(s.nodeSettings).not.toHaveProperty('items');
    expect(s.byKey.get('local:work')!.items.map((i) => i.title)).toEqual(['Write tests']);
    items = [item(), item({ id: 'z9', title: 'From a record', pri: 2 })];
    await s.load();
    expect(s.byKey.get('local:work')!.items.map((i) => [i.title, i.pri])).toEqual([['Write tests', 0], ['From a record', 2]]);
    expect(s.nodeSettings!.version).toBe('v-work');
    expect(api.getTree).toHaveBeenCalledTimes(2);
  });

  it('a tree poll whose stamp changed reloads the items', async () => {
    let stamp = 's1';
    let items = [item()];
    const api = mockApi({ getStamp: vi.fn(async () => ({ stamp })), getTree: vi.fn(async () => makeTree('v-work', items)) });
    const s = new Store(api);
    await s.load();
    await s.poll();
    stamp = 's2';
    items = [];
    await s.poll();
    expect(s.byKey.get('local:work')!.items).toEqual([]);
  });

  it('the node settings follows on-disk changes (no stale state)', async () => {
    let version = 'v-work';
    const api = mockApi({ getTree: vi.fn(async () => makeTree(version)) });
    const s = new Store(api);
    await s.load();
    s.openNodeSettings('local:work');
    version = 'v-new';
    await s.load();
    expect(s.nodeSettings!.version).toBe('v-new');
  });

  it('poll failure shows the lost-connection notice and clears it on recovery', async () => {
    let fail = true;
    const s = new Store(mockApi({ getStamp: vi.fn(async () => { if (fail) throw new Error('down'); return { stamp: 'x' }; }) }));
    await s.poll();
    expect(s.notice).toMatch(/Lost connection/);
    fail = false;
    await s.poll();
    expect(s.notice).toBeNull();
  });

  it('saveMeta omits untouched known keys, then reloads', async () => {
    const api = mockApi();
    const s = new Store(api);
    await s.load();
    s.openNodeSettings('local:work');
    expect(metaPayload(s.nodeSettings!)).toEqual({});
    s.nodeSettings!.meta.find((e) => e[0] === 'kind')![1] = 'project';
    await s.saveMeta();
    expect(api.saveMeta.mock.calls[0][0]).toEqual({ source: 'local', path: 'work', version: 'v-work', meta: { kind: 'project' } });
    expect(api.getTree).toHaveBeenCalledTimes(2);
  });

  it('init calls initNode for a plain dir and reloads', async () => {
    const api = mockApi();
    const s = new Store(api);
    await s.load();
    s.openNodeSettings('local:plain');
    await s.init('local:plain');
    expect(api.initNode).toHaveBeenCalledWith({ source: 'local', path: 'plain' });
    expect(api.getTree).toHaveBeenCalledTimes(2);
  });

  it('replaceNode keeps the scanned children and via', async () => {
    const t = makeTree();
    t.root.children[0].children[0].via = 'sub';
    t.root.children[0].children[0].children = [node('work/x')];
    const s = new Store(mockApi({ getTree: vi.fn(async () => t) }));
    await s.load();
    s.replaceNode(node('work', { version: 'v9', via: null, children: [] }));
    const w = s.byKey.get('local:work')!;
    expect(w.version).toBe('v9');
    expect(w.via).toBe('sub');
    expect(w.children.map((c) => c.path)).toEqual(['work/x']);
  });

  it('toggleCollapse / expandAll / collapseAll / setFocus', async () => {
    const s = new Store(mockApi());
    await s.load();
    s.toggleCollapse('local:work');
    expect(s.collapsed.has('local:work')).toBe(true);
    s.toggleCollapse('local:work');
    expect(s.collapsed.has('local:work')).toBe(false);
    s.collapseAll();
    expect(s.collapsed.has('local:work')).toBe(false);
    s.setFocus('local:work');
    expect(s.focus).toBe('local:work');
    expect(s.displayRoot?.path).toBe('work');
    s.setFocus('local:');
    expect(s.focus).toBe('local:');
    expect(s.displayRoot?.source).toBe('local');
    s.setFocus(':');
    expect(s.focus).toBeNull();
    expect(s.displayRoot?.name).toBe('/');
  });
});

describe('Store: empty nodes and collapse all at /', () => {
  const deepTree = (): Tree => ({
    ...makeTree(),
    root: vroot([
      node('', {
        meta: { name: 'fabr' },
        children: [
          node('work', { items: [item({ phase: 'working' })], children: [node('work/a', { items: [item({ phase: 'done' })], children: [node('work/a/b')] })] }),
          node('plain', { managed: false, version: null, children: [node('plain/x', { items: [item()] })] }),
          node('idle', { items: [item({ phase: 'done' })] }),
        ],
      }),
    ]),
  });
  const shown = (s: Store) => [...s.viewIndex.byKey.keys()];

  it('hideEmpty follows See + Done, persists in fabr:prefs and relayouts', async () => {
    const mem = new Map<string, string>();
    vi.stubGlobal('localStorage', { getItem: (k: string) => mem.get(k) ?? null, setItem: (k: string, v: string) => void mem.set(k, v), removeItem: (k: string) => void mem.delete(k) });
    const s = new Store(mockApi({ getTree: vi.fn(async () => deepTree()) }));
    await s.load();
    expect(s.hideEmpty).toBe(false);
    expect(shown(s)).toContain('local:idle');
    const tally = rollup(s.tallyNode(s.displayRoot!), TODAY);
    const v = s.layoutVersion;
    s.filter = 'wip';
    s.setHideEmpty(true);
    expect(s.layoutVersion).toBeGreaterThan(v);
    expect(shown(s)).toEqual([':', 'local:', 'local:work']);
    expect(JSON.parse(mem.get('fabr:prefs')!).hideEmpty).toBe(true);
    expect(rollup(s.tallyNode(s.displayRoot!), TODAY)).toEqual(tally);
    expect(rollup(s.displayRoot!, TODAY)).not.toEqual(tally);
    expect(s.tallyNode(s.byKey.get('local:')!).children.length).toBe(3);
    s.filter = 'all';
    expect(shown(s)).toContain('local:plain/x');
    expect(shown(s)).toContain('local:idle');
    s.setHideDone(true);
    expect(shown(s)).not.toContain('local:idle');
    expect(shown(s)).not.toContain('local:work/a');
    const s2 = new Store(mockApi());
    s2.restorePrefs();
    expect(s2.hideEmpty).toBe(true);
    vi.unstubAllGlobals();
  });

  it('collapseAll at / keeps the source roots and their children visible', async () => {
    const s = new Store(mockApi({ getTree: vi.fn(async () => deepTree()) }));
    await s.load();
    s.collapsed.add('local:');
    s.collapseAll();
    expect(s.collapsed.has(':')).toBe(false);
    expect(s.collapsed.has('local:')).toBe(false);
    expect([...s.collapsed].sort()).toEqual(['local:plain', 'local:work']);
    s.expandAll();
    s.setFocus('local:work');
    s.collapseAll();
    expect([...s.collapsed]).toEqual(['local:work/a']);
  });
});

function liveApi() {
  const tree = makeTree();
  let n = 0;
  const work = () => tree.root.children[0].children[0];
  return mockApi({
    getTree: vi.fn(async () => JSON.parse(JSON.stringify(tree))),
    saveMeta: vi.fn(async (req) => {
      Object.assign(work(), { meta: req.meta, version: `v${++n}` });
      return { ...JSON.parse(JSON.stringify(work())), children: [] };
    }),
  });
}

describe('view modes, hidden nodes, autosave', () => {
  const hiddenTree = (): Tree => ({
    ...makeTree(),
    root: node('', {
      meta: { name: 'fabr' },
      children: [
        node('work', { items: [item()], children: [node('work/plain', { managed: false, version: null, children: [node('work/plain/deep', { items: [item({ id: 'd1', title: 'deep' })] })] })] }),
        node('arch', { meta: { hidden: true }, items: [item({ id: 'h1', title: 'hidden one' })], children: [node('arch/old')] }),
      ],
    }),
  });
  const paths = (n: Node | null): string[] => (n ? [n.path, ...n.children.flatMap(paths)] : []);

  it('viewMode filters plain dirs client-side and only reloads for all', async () => {
    const api = mockApi({ getTree: vi.fn(async () => hiddenTree()) });
    const s = new Store(api);
    await s.load();
    expect(paths(s.displayRoot)).toEqual(['', 'work', 'work/plain', 'work/plain/deep']);
    await s.setViewMode('nodes');
    expect(paths(s.displayRoot)).toEqual(['', 'work', 'work/plain/deep']);
    expect(s.displayRoot!.children[0].children[0].via).toBe('plain');
    expect(api.getTree).toHaveBeenCalledTimes(1);
    await s.setViewMode('all');
    expect(api.getTree).toHaveBeenCalledTimes(2);
    expect(api.getTree.mock.calls[1][0]).toBe(true);
    await s.setViewMode('folders');
    expect(api.getTree.mock.calls[2][0]).toBe(false);
  });

  it('a bound viewMode (set before setViewMode runs, as the toolbar does) still reloads with the new show_all', async () => {
    const api = mockApi({ getTree: vi.fn(async () => hiddenTree()) });
    const s = new Store(api);
    await s.load();
    expect(api.getTree.mock.calls[0][0]).toBe(false);
    s.viewMode = 'all';
    await s.setViewMode('all');
    expect(api.getTree).toHaveBeenCalledTimes(2);
    expect(api.getTree.mock.calls[1][0]).toBe(true);
    s.viewMode = 'folders';
    await s.setViewMode('folders');
    expect(api.getTree).toHaveBeenCalledTimes(3);
    expect(api.getTree.mock.calls[2][0]).toBe(false);
    s.viewMode = 'nodes';
    await s.setViewMode('nodes');
    expect(api.getTree).toHaveBeenCalledTimes(3);
  });

  it('viewMode and showHidden persist in prefs', async () => {
    const mem = new Map<string, string>();
    vi.stubGlobal('localStorage', { getItem: (k: string) => mem.get(k) ?? null, setItem: (k: string, v: string) => void mem.set(k, v), removeItem: (k: string) => void mem.delete(k) });
    try {
      const s = new Store(mockApi());
      await s.load();
      await s.setViewMode('nodes');
      s.setShowHidden(true);
      const s2 = new Store(mockApi());
      s2.restorePrefs();
      expect(s2.viewMode).toBe('nodes');
      expect(s2.showHidden).toBe(true);
      mem.set('fabr:prefs', JSON.stringify({ showAll: true }));
      s2.restorePrefs();
      expect(s2.viewMode).toBe('folders');
      expect(s2.showHidden).toBe(false);
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('restorePrefs takes only values that pass their guard, else the defaults', () => {
    const mem = new Map<string, string>();
    vi.stubGlobal('localStorage', { getItem: (k: string) => mem.get(k) ?? null, setItem: (k: string, v: string) => void mem.set(k, v), removeItem: (k: string) => void mem.delete(k) });
    try {
      mem.set('fabr:prefs', JSON.stringify({ filter: 'wip', hideDone: 1, hideEmpty: true, view_mode: 'bogus', showHidden: 'yes', focus: 7, view: 'list' }));
      mem.set('fabr:list', JSON.stringify({ sortBy: 'nope', sortDir: 'desc', groupBy: 'status' }));
      const s = new Store(mockApi());
      s.restorePrefs();
      expect([s.filter, s.hideDone, s.hideEmpty, s.viewMode, s.showHidden, s.focus, s.view]).toEqual(['wip', false, true, 'folders', false, null, 'list']);
      expect({ ...s.list }).toEqual({ sortBy: 'pri', sortDir: 'desc', groupBy: 'status' });
      s.savePrefs();
      expect(JSON.parse(mem.get('fabr:prefs')!)).toEqual({ filter: 'wip', hideDone: false, hideEmpty: true, view_mode: 'folders', showHidden: false, focus: null, view: 'list' });
      mem.set('fabr:prefs', 'null');
      s.restorePrefs();
      expect(s.filter).toBe('all');
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('hidden nodes are omitted everywhere until Show hidden is on', async () => {
    const s = new Store(mockApi({ getTree: vi.fn(async () => hiddenTree()) }));
    await s.load();
    expect(paths(s.displayRoot)).not.toContain('arch');
    expect(s.viewIndex.byKey.has('local:arch/old')).toBe(false);
    expect(s.byKey.has('local:arch')).toBe(true);
    s.setShowHidden(true);
    expect(paths(s.displayRoot)).toContain('arch/old');
    s.setShowHidden(false);
    expect(paths(s.displayRoot)).not.toContain('arch');
  });

  it('focus on a node that becomes hidden falls back to the root', async () => {
    const s = new Store(mockApi({ getTree: vi.fn(async () => hiddenTree()) }));
    await s.load();
    s.setShowHidden(true);
    s.setFocus('local:arch');
    expect(s.displayRoot?.path).toBe('arch');
    s.setShowHidden(false);
    expect(s.focus).toBeNull();
  });

  it('Hide / Unhide write hidden through saveMeta with the held version', async () => {
    const api = liveApi();
    const s = new Store(api);
    await s.load();
    s.openNodeSettings('local:work');
    expect(await s.setHidden(true)).toBe(true);
    expect(api.saveMeta.mock.calls[0][0]).toEqual({ source: 'local', path: 'work', version: 'v-work', meta: { hidden: true } });
    expect(s.nodeSettings).toBeNull();
    expect(paths(s.displayRoot)).not.toContain('work');
    s.setShowHidden(true);
    s.openNodeSettings('local:work');
    expect(s.nodeSettings!.meta.find((e) => e[0] === 'hidden')![1]).toBe(true);
    expect(await s.setHidden(false)).toBe(true);
    expect(api.saveMeta.mock.calls[1][0]).toEqual({ source: 'local', path: 'work', version: 'v1', meta: { hidden: false } });
    expect(s.nodeSettings?.key).toBe('local:work');
    expect(paths(s.displayRoot)).toContain('work');
    await s.setHidden(true);
    expect(s.nodeSettings?.key).toBe('local:work');
  });

  it('hide, then Show hidden: the hidden node is in the displayed tree and the list rows', async () => {
    let hidden = false;
    const api = mockApi({
      getTree: vi.fn(async () => ({ ...makeTree(), root: node('', { children: [node('work', { meta: hidden ? { hidden: true } : {}, items: [item()] })] }) })),
      saveMeta: vi.fn(async (req) => { hidden = req.meta.hidden === true; return node('work', { version: 'v9', meta: req.meta }); }),
    });
    const s = new Store(api);
    await s.load();
    s.openNodeSettings('local:work');
    await s.setHidden(true);
    expect(paths(s.displayRoot)).toEqual(['']);
    expect(flattenItems(s.displayRoot!, { today: TODAY, filter: 'all', hideDone: false })).toHaveLength(0);
    s.setShowHidden(true);
    expect(paths(s.displayRoot)).toEqual(['', 'work']);
    expect(flattenItems(s.displayRoot!, { today: TODAY, filter: 'all', hideDone: false })).toHaveLength(1);
    expect(s.viewIndex.byKey.has('local:work')).toBe(true);
  });

  it('hide with a 409 reloads the node and keeps the node settings with a banner', async () => {
    const fresh = node('work', { version: 'v-disk' });
    const s = new Store(mockApi({ saveMeta: vi.fn(async () => { throw new ApiError(409, 'conflict', fresh); }) }));
    await s.load();
    s.openNodeSettings('local:work');
    expect(await s.setHidden(true)).toBe(false);
    expect(s.nodeSettings!.banner?.text).toMatch(/changed on disk/);
    expect(s.nodeSettings!.version).toBe('v-disk');
  });

  it('meta autosave: commit saves one field with the current version and shows Saved', async () => {
    const api = liveApi();
    const s = new Store(api);
    await s.load();
    s.openNodeSettings('local:work');
    await s.commitMeta('kind', 'project');
    expect(api.saveMeta).toHaveBeenCalledTimes(1);
    expect(api.saveMeta.mock.calls[0][0]).toEqual({ source: 'local', path: 'work', version: 'v-work', meta: { kind: 'project' } });
    expect(s.nodeSettings!.metaNote).toEqual({ key: 'kind', kind: 'ok', text: 'Saved' });
    expect(s.nodeSettings!.meta.find((e) => e[0] === 'kind')![1]).toBe('project');
    await s.commitMeta('kind', 'project');
    expect(api.saveMeta).toHaveBeenCalledTimes(1);
    await s.commitMeta('description', 'x');
    expect(api.saveMeta.mock.calls[1][0]).toMatchObject({ version: 'v1', meta: { kind: 'project', description: 'x' } });
  });

  it('meta autosave: Enter then blur in quick succession saves once; two fields are serialised', async () => {
    const api = liveApi();
    const s = new Store(api);
    await s.load();
    s.openNodeSettings('local:work');
    await Promise.all([s.commitMeta('kind', 'a'), s.commitMeta('kind', 'a'), s.commitMeta('description', 'd')]);
    expect(api.saveMeta).toHaveBeenCalledTimes(2);
    expect(api.saveMeta.mock.calls[1][0]).toMatchObject({ version: 'v1', meta: { kind: 'a', description: 'd' } });
  });

  it('meta autosave: add and remove a generic key, switch change', async () => {
    const api = liveApi();
    const s = new Store(api);
    await s.load();
    s.openNodeSettings('local:work');
    await s.commitMeta('owner', 'me');
    expect(api.saveMeta.mock.calls[0][0].meta).toEqual({ owner: 'me' });
    await s.commitMeta('owner', undefined);
    expect(api.saveMeta.mock.calls[1][0].meta).toEqual({});
    await s.commitMeta('hidden', true);
    expect(api.saveMeta.mock.calls[2][0].meta).toEqual({ hidden: true });
  });

  it('meta autosave: errors show next to the field; a 409 shows the banner and reloads', async () => {
    const bad = new Store(mockApi({ saveMeta: vi.fn(async () => { throw new ApiError(422, 'meta.toml is not valid'); }) }));
    await bad.load();
    bad.openNodeSettings('local:work');
    await bad.commitMeta('kind', 'x');
    expect(bad.nodeSettings!.metaNote).toEqual({ key: 'kind', kind: 'error', text: 'meta.toml is not valid' });
    expect(bad.nodeSettings!.banner?.kind).toBe('error');
    const fresh = node('work', { version: 'v-disk', meta: { kind: 'disk' } });
    const conflict = new Store(mockApi({ saveMeta: vi.fn(async () => { throw new ApiError(409, 'conflict', fresh); }) }));
    await conflict.load();
    conflict.openNodeSettings('local:work');
    await conflict.commitMeta('kind', 'x');
    expect(conflict.nodeSettings!.banner?.text).toMatch(/changed on disk/);
    expect(conflict.nodeSettings!.version).toBe('v-disk');
    expect(conflict.nodeSettings!.metaNote).toBeNull();
    expect(conflict.nodeSettings!.meta.find((e) => e[0] === 'kind')![1]).toBe('disk');
  });
});

describe('slow-disk retries', () => {
  const slow = (extra: Partial<Tree> = {}): Tree => ({
    ...makeTree(), truncated: true, truncated_by: 'time',
    truncated_reason: 'local: the disk was slow (35 directories in 2.2 s); rescanning…', ...extra,
  });

  it('a time truncation with few directories rescans after 1 s, up to 3 times, then shows Rescan', async () => {
    vi.useFakeTimers();
    try {
      const api = mockApi({ getTree: vi.fn(async () => slow()) });
      const s = new Store(api);
      await s.load();
      expect(s.noticeRescan).toBe(false);
      expect(s.notice).toMatch(/rescanning/);
      expect(api.getTree).toHaveBeenCalledTimes(1);
      for (let n = 2; n <= 4; n++) {
        await vi.advanceTimersByTimeAsync(1000);
        expect(api.getTree).toHaveBeenCalledTimes(n);
        expect(api.getTree.mock.calls[n - 1][1]).toBe(true);
      }
      await vi.advanceTimersByTimeAsync(5000);
      expect(api.getTree).toHaveBeenCalledTimes(4);
      expect(s.noticeRescan).toBe(true);
      expect(s.notice).toMatch(/Press Rescan/);
      expect(s.notice).not.toMatch(/rescanning/);
    } finally {
      vi.useRealTimers();
    }
  });

  it('stops retrying once the rescan is complete', async () => {
    vi.useFakeTimers();
    try {
      let n = 0;
      const api = mockApi({ getTree: vi.fn(async () => (n++ === 0 ? slow() : makeTree())) });
      const s = new Store(api);
      await s.load();
      await vi.advanceTimersByTimeAsync(1000);
      expect(api.getTree).toHaveBeenCalledTimes(2);
      expect(s.notice).toBeNull();
      await vi.advanceTimersByTimeAsync(5000);
      expect(api.getTree).toHaveBeenCalledTimes(2);
    } finally {
      vi.useRealTimers();
    }
  });

  it('does not retry big scans or other budgets', async () => {
    vi.useFakeTimers();
    try {
      for (const t of [slow({ truncated_reason: 'local: the disk was slow (5000 directories in 4.1 s); rescanning…' }), slow({ truncated_by: 'nodes' }), slow({ truncated_by: 'visited' })]) {
        const api = mockApi({ getTree: vi.fn(async () => t) });
        const s = new Store(api);
        await s.load();
        await vi.advanceTimersByTimeAsync(5000);
        expect(api.getTree).toHaveBeenCalledTimes(1);
        expect(s.noticeRescan).toBe(true);
      }
    } finally {
      vi.useRealTimers();
    }
  });
});

describe('accent colour', () => {
  const colorTree = (): Tree => ({
    ...makeTree(),
    root: node('', { meta: { name: 'fabr' }, children: [node('work', { meta: { color: 'pink' }, children: [node('work/api')] })] }),
  });

  it('colors derive from the tree and the app tint; inheritedColor walks the ancestors', async () => {
    const s = new Store(mockApi({ getTree: vi.fn(async () => colorTree()) }));
    await s.load();
    expect(s.colors.get('local:')).toBe('var(--blue)');
    expect(s.colors.get('local:work/api')).toBe('var(--pink)');
    expect(s.inheritedColor('local:work/api')).toBe('pink');
    expect(s.inheritedColor('local:work')).toBe('blue');
    s.tint = 'teal';
    expect(s.colors.get('local:')).toBe('var(--teal)');
    s.setFocus('local:work/api');
    expect(s.colors.get('local:work/api')).toBe('var(--pink)');
  });

  it('swatch selection saves {color} through the autosave path; Inherit removes the key', async () => {
    const api = mockApi({ getTree: vi.fn(async () => colorTree()) });
    const s = new Store(api);
    await s.load();
    s.openNodeSettings('local:work/api');
    expect(await s.setColor('teal')).toBe(true);
    expect(api.saveMeta.mock.calls[0][0].meta.color).toBe('teal');
    expect(api.saveMeta.mock.calls[0][0].version).toBe('v-work/api');
    s.openNodeSettings('local:work');
    expect(await s.commitMeta('color', '#12ab9f')).toBe(true);
    expect(api.saveMeta.mock.calls[1][0].meta.color).toBe('#12ab9f');
    expect(await s.setColor(null)).toBe(true);
    expect('color' in api.saveMeta.mock.calls[2][0].meta).toBe(false);
  });

  it('layoutVersion bumps on programmatic layout changes and on reloads that change the node count', async () => {
    let t = colorTree();
    const s = new Store(mockApi({ getTree: vi.fn(async () => t) }));
    await s.load();
    let v = s.layoutVersion;
    s.toggleCollapse('local:work');
    expect(s.layoutVersion).toBeGreaterThan(v);
    v = s.layoutVersion;
    s.setFocus('local:work');
    expect(s.layoutVersion).toBeGreaterThan(v);
    v = s.layoutVersion;
    await s.load();
    expect(s.layoutVersion).toBe(v);
    t = { ...makeTree(), root: node('', { children: [node('work')] }) };
    await s.load();
    expect(s.layoutVersion).toBeGreaterThan(v);
  });

  it('activate only selects; the chevron folds; the gear opens settings and selects', async () => {
    const s = new Store(mockApi());
    await s.load();
    s.activate('local:work');
    expect(s.selected).toBe('local:work');
    expect(s.collapsed.has('local:work')).toBe(false);
    s.toggleCollapse('local:work');
    expect(s.collapsed.has('local:work')).toBe(true);
    expect(s.nodeSettings).toBeNull();
    s.openNodeSettings('local:plain');
    expect(s.nodeSettings?.key).toBe('local:plain');
    expect(s.selected).toBe('local:plain');
    s.openNodeSettings('local:work');
    expect(s.nodeSettings?.key).toBe('local:work');
    s.closeNodeSettings();
    expect(s.nodeSettings).toBeNull();
  });

  it('metaPayload drops an invalid colour but keeps other metadata, and keeps a valid one', async () => {
    const s = new Store(mockApi({ getTree: vi.fn(async () => ({ ...makeTree(), root: node('', { children: [node('work', { meta: { name: 'W', color: 'chartreuse' } })] }) })) }));
    await s.load();
    s.openNodeSettings('local:work');
    const p = s.nodeSettings!;
    p.meta.find(([k]) => k === 'kind')![1] = 'repo';
    const out = metaPayload(p);
    expect('color' in out).toBe(false);
    expect(out.kind).toBe('repo');
    p.meta.find(([k]) => k === 'color')![1] = '#3a7bd5';
    expect(metaPayload(p).color).toBe('#3a7bd5');
  });

  it('setHideDone persists and relayouts', async () => {
    const s = new Store(mockApi());
    await s.load();
    const v = s.layoutVersion;
    s.setHideDone(true);
    expect(s.hideDone).toBe(true);
    expect(s.layoutVersion).toBeGreaterThan(v);
  });
});

describe('remove node', () => {
  it('removeSummary counts tasks and open tasks and says they stay', () => {
    expect(removeSummary({ items: [] })).toEqual({ tasks: 0, open: 0, text: 'there are no tasks' });
    expect(removeSummary({ items: [item({ phase: 'done' })] }).text).toBe('its tasks stay in .fabric-space (1, 0 open)');
    const r = removeSummary({ items: [item(), item({ id: 'b', phase: 'working' }), item({ id: 'c', phase: 'done' })] });
    expect([r.tasks, r.open, r.text]).toEqual([3, 2, 'its tasks stay in .fabric-space (3, 2 open)']);
  });

  it('success closes the node settings and reloads', async () => {
    const api = mockApi();
    const s = new Store(api);
    await s.load();
    s.openNodeSettings('local:work');
    expect(await s.removeNode()).toBe(true);
    expect(api.removeNode).toHaveBeenCalledWith({ source: 'local', path: 'work', version: 'v-work' });
    expect(s.nodeSettings).toBeNull();
    expect(s.selected).toBeNull();
    expect(api.getTree).toHaveBeenCalledTimes(2);
  });

  it('says so when the folder drops out of view', async () => {
    let n = 0;
    const api = mockApi({
      getTree: vi.fn(async () => {
        const t = makeTree();
        if (n++ > 0) t.root.children[0].children = t.root.children[0].children.filter((c) => c.path !== 'work');
        return t;
      }),
    });
    const s = new Store(api);
    await s.load();
    s.openNodeSettings('local:work');
    await s.removeNode();
    expect(s.notice).toBe('Removed node work; the folder is still there');
  });

  it('409 with a fresh node: banner, node reloaded, settings stay open', async () => {
    const fresh = node('work', { version: 'v-new', items: [item({ title: 'Changed elsewhere' })] });
    let loads = 0;
    const api = mockApi({
      removeNode: vi.fn(async () => { throw new ApiError(409, 'changed', fresh); }),
      getTree: vi.fn(async () => makeTree(loads++ ? 'v-new' : 'v-work')),
    });
    const s = new Store(api);
    await s.load();
    s.openNodeSettings('local:work');
    expect(await s.removeNode()).toBe(false);
    expect(s.nodeSettings).not.toBeNull();
    expect(s.nodeSettings!.banner?.text).toMatch(/changed on disk/);
    expect(s.nodeSettings!.version).toBe('v-new');
    expect(api.getTree).toHaveBeenCalledTimes(2);
  });

  it('409 for extra files shows the server message', async () => {
    const msg = '`.fabr/` contains other files; remove it by hand';
    const api = mockApi({ removeNode: vi.fn(async () => { throw new ApiError(409, msg, node('work', { version: 'v-work' })); }) });
    const s = new Store(api);
    await s.load();
    s.openNodeSettings('local:work');
    expect(await s.removeNode()).toBe(false);
    expect(s.nodeSettings!.banner).toEqual({ kind: 'error', text: msg });
  });

  it('ignores plain directories', async () => {
    const api = mockApi();
    const s = new Store(api);
    await s.load();
    s.openNodeSettings('local:plain');
    expect(await s.removeNode()).toBe(false);
    expect(api.removeNode).not.toHaveBeenCalled();
  });
});

describe('the / root', () => {
  const twoSources = (): Tree => ({
    root: vroot([
      node('', { source: 'a', name: 'Alpha', items: [item()], children: [node('work', { source: 'a', items: [item({ id: 'w', phase: 'working' })], children: [node('work/api', { source: 'a' })] })] }),
      node('', { source: 'b', name: 'Beta', managed: false, version: null, warnings: ['ssh: timeout'] }),
    ]),
    sources: [
      { id: 'a', name: 'Alpha', host: null, root: '/x', prune: null },
      { id: 'b', name: 'Beta', host: 'me@box', root: '~', prune: null },
    ],
    today: TODAY,
    truncated: false,
    errors: [{ source: 'b', message: 'ssh: timeout' }],
  });

  it('displayRoot is / while nothing is focused; rootKey is ":"', async () => {
    const s = new Store(mockApi());
    await s.load();
    expect(s.focus).toBeNull();
    expect(s.rootKey).toBe(':');
    expect([s.displayRoot?.name, s.displayRoot?.source, s.displayRoot?.path]).toEqual(['/', '', '']);
    expect(s.displayRoot?.children.map((c) => c.source)).toEqual(['local']);
    expect(s.colors.get(':')).toBe('var(--blue)');
  });

  it('pathSegments: /, the source name, then one segment per path part', async () => {
    const s = new Store(mockApi({ getTree: vi.fn(async () => twoSources()) }));
    await s.load();
    expect(s.pathSegments()).toEqual([{ label: '/', key: ':' }]);
    s.setFocus('a:');
    expect(s.pathSegments()).toEqual([{ label: '/', key: ':' }, { label: 'Alpha', key: 'a:' }]);
    s.setFocus('a:work/api');
    expect(s.pathSegments()).toEqual([
      { label: '/', key: ':' }, { label: 'Alpha', key: 'a:' }, { label: 'work', key: 'a:work' }, { label: 'api', key: 'a:work/api' },
    ]);
    expect(s.displayRoot?.path).toBe('work/api');
    s.setFocus(':');
    expect(s.focus).toBeNull();
    expect(s.pathSegments()).toEqual([{ label: '/', key: ':' }]);
  });

  it('the / node has no version and is never saved; a source line opens the source root', async () => {
    const api = mockApi({ getTree: vi.fn(async () => twoSources()) });
    const s = new Store(api);
    await s.load();
    s.openNodeSettings(':');
    expect([s.nodeSettings?.key, s.nodeSettings?.managed, s.nodeSettings?.version]).toEqual([':', false, null]);
    expect(await s.commitMeta('name', 'x')).toBe(false);
    expect(api.saveMeta).not.toHaveBeenCalled();
    s.openNodeSettings('b:');
    expect(s.nodeSettings?.key).toBe('b:');
  });

  it('openSettings("sources") sets the one-shot page hint', () => {
    const s = new Store(mockApi());
    s.openSettings('sources');
    expect([s.sheetOpen, s.sheetPage]).toEqual([true, 'sources']);
  });
});

describe('rename through Identity -> Name (regression)', () => {
  it('a source root in a two-source tree: saved, and the displayed card title follows', async () => {
    const api = createMockApi({ today: TODAY, latency: 0, multi: true });
    const s = new Store(api);
    await s.load();
    const k = 'lab:';
    s.openNodeSettings(k);
    expect(await s.commitMeta('name', 'Lab renamed')).toBe(true);
    const shown = s.viewIndex.byKey.get(k)!;
    expect(shown.meta.name).toBe('Lab renamed');
    expect(cardTitle(shown)).toBe('Lab renamed');
    expect(s.nodeSettings?.name).toBe('Lab renamed');
  });

  it('a nested node: saved with its own source and path, card title follows', async () => {
    const api = createMockApi({ today: TODAY, latency: 0, multi: true });
    const spy = vi.spyOn(api, 'saveMeta');
    const s = new Store(api);
    await s.load();
    s.openNodeSettings('local:work');
    expect(await s.commitMeta('name', 'Work renamed')).toBe(true);
    expect(spy.mock.calls[0][0]).toMatchObject({ source: 'local', path: 'work', meta: { name: 'Work renamed' } });
    expect(cardTitle(s.viewIndex.byKey.get('local:work')!)).toBe('Work renamed');
  });
});

describe('settings sheet does not wait on the tree', () => {
  it('saveSourceList (delete a source) resolves before getTree does', async () => {
    let release: (t: Tree) => void = () => {};
    const slow = new Promise<Tree>((r) => (release = r));
    const getTree = vi.fn(async () => makeTree());
    const s = new Store(mockApi({ getTree }));
    await s.load();
    getTree.mockImplementation(() => slow);
    await s.saveSourceList([]);
    expect(s.loading).toBe(true);
    release(makeTree());
    await vi.waitFor(() => expect(s.loading).toBe(false));
  });
  it('saveConfig and saveSettingsOnly do not wait either', async () => {
    const never = new Promise<Tree>(() => {});
    const s = new Store(mockApi({ getTree: vi.fn(() => never) }));
    await s.saveConfig([], { poll_ms: 2000, remote_poll_ms: 30000, max_nodes: 10000 });
    await s.saveSettingsOnly({ poll_ms: 2000, remote_poll_ms: 30000, max_nodes: 10000 });
    expect(s.loading).toBe(true);
  });
});

describe('openUrl', () => {
  it('passes the link to the api; a failure becomes the notice', async () => {
    const api = mockApi();
    const s = new Store(api);
    await s.openUrl('https://login.tailscale.com/a/abc');
    expect(api.openUrl).toHaveBeenCalledWith('https://login.tailscale.com/a/abc');
    expect(s.notice).toBeNull();
    api.openUrl.mockRejectedValueOnce(new ApiError(400, 'only http and https links can be opened'));
    await s.openUrl('file:///x');
    expect(s.notice).toMatch(/Could not open the link/);
  });
});
