import { describe, expect, it, vi } from 'vitest';
import type { Api } from './api/index';
import { ApiError, type Item, type Node, type SaveTodoReq, type Tree } from './api/types';
import { appendItem, patchItem, removeItem, titleDecision } from './panel/itemcommit';
import { Store } from './store.svelte';

const item = (o: Partial<Item> = {}): Item => ({ id: 'a1', status: 'todo', due: null, title: 'Write tests', pri: null, ...o });
const node = (path: string, o: Partial<Node> = {}): Node => ({
  source: 'local', path, name: path.split('/').pop() || 'root', managed: true, meta: {}, items: [],
  warnings: [], version: `v-${path}`, via: null, has_todo: true, children: [], ...o,
});
const vroot = (children: Node[]): Node => ({
  source: '', path: '', name: '/', managed: false, meta: {}, items: [], warnings: [], version: null, via: null, has_todo: false, children,
});
const ITEMS = [item(), item({ id: 'b2', status: 'wip', title: 'Ship' })];
const makeTree = (version = 'v-work'): Tree => ({
  root: vroot([node('', { children: [node('work', { version, items: ITEMS.map((i) => ({ ...i })) })] })]),
  sources: [{ id: 'local', name: 'Home', host: null, root: '/tmp/x', prune: null }],
  today: '2026-10-19',
  truncated: false,
  errors: [],
});

function setup(over: Partial<Api> = {}) {
  let n = 0;
  const saveTodo = vi.fn(async (req: SaveTodoReq) => node(req.path, { version: `v${++n}`, items: req.items }));
  const getTree = vi.fn(async () => makeTree());
  const api = { getTree, getStamp: vi.fn(async () => ({ stamp: 's' })), saveTodo, saveMeta: vi.fn(), ...over } as unknown as Api;
  return { api, saveTodo, getTree, store: new Store(api) };
}
const W = 'local:work';

describe('titleDecision (the title field)', () => {
  it('unchanged -> none, empty -> restore, changed -> save trimmed', () => {
    expect(titleDecision('Ship', 'Ship')).toEqual({ kind: 'none', title: 'Ship' });
    expect(titleDecision('Ship', '  Ship ')).toEqual({ kind: 'none', title: 'Ship' });
    expect(titleDecision('Ship', '   ')).toEqual({ kind: 'restore', title: 'Ship' });
    expect(titleDecision('Ship', ' Ship it ')).toEqual({ kind: 'save', title: 'Ship it' });
  });
});

describe('items autosave', () => {
  it('a title commit sends the held version and the changed item only', async () => {
    const { store, saveTodo } = setup();
    await store.load();
    store.openPanel(W);
    expect(await store.commitItems(W, patchItem(1, { title: 'Ship it' }))).toBe(true);
    const req = saveTodo.mock.calls[0][0];
    expect(req).toMatchObject({ source: 'local', path: 'work', version: 'v-work' });
    expect(req.items.map((i) => i.title)).toEqual(['Write tests', 'Ship it']);
    expect(store.panel!.version).toBe('v1');
    expect(store.panel!.items[1].title).toBe('Ship it');
    expect(store.panel!.itemsNote).toEqual({ kind: 'ok', text: 'Saved' });
  });

  it('unchanged or empty titles make no request (decision + a no-op mutation)', async () => {
    const { store, saveTodo } = setup();
    await store.load();
    store.openPanel(W);
    expect(titleDecision(store.panel!.items[0].title, 'Write tests').kind).toBe('none');
    expect(titleDecision(store.panel!.items[0].title, '').kind).toBe('restore');
    expect(await store.commitItems(W, patchItem(9, { title: 'x' }))).toBe(true);
    expect(saveTodo).not.toHaveBeenCalled();
  });

  it('a status change and a delete each commit', async () => {
    const { store, saveTodo } = setup();
    await store.load();
    store.openPanel(W);
    await store.commitItems(W, patchItem(0, { status: 'done' }));
    expect(saveTodo.mock.calls[0][0].items.map((i) => i.status)).toEqual(['done', 'wip']);
    await store.commitItems(W, removeItem(0));
    expect(saveTodo.mock.calls[1][0].items.map((i) => i.title)).toEqual(['Ship']);
    expect(store.panel!.items.map((i) => i.title)).toEqual(['Ship']);
    expect(store.byKey.get(W)!.items.length).toBe(1);
  });

  it('a draft row is not saved until it has a title', async () => {
    const { store, saveTodo } = setup();
    await store.load();
    store.openPanel(W);
    expect(saveTodo).not.toHaveBeenCalled();
    await store.commitItems(W, appendItem({ id: null, status: 'todo', due: null, title: 'New', pri: null }));
    expect(saveTodo.mock.calls[0][0].items.map((i) => i.title)).toEqual(['Write tests', 'Ship', 'New']);
    expect(store.panel!.items.length).toBe(3);
  });

  it('two quick commits chain versions: the second uses the version the first returned', async () => {
    const { store, saveTodo } = setup();
    await store.load();
    store.openPanel(W);
    const a = store.commitItems(W, patchItem(0, { title: 'A' }));
    const b = store.commitItems(W, patchItem(1, { title: 'B' }));
    expect(await Promise.all([a, b])).toEqual([true, true]);
    expect(saveTodo.mock.calls.map((c) => c[0].version)).toEqual(['v-work', 'v1']);
    expect(saveTodo.mock.calls[1][0].items.map((i) => i.title)).toEqual(['A', 'B']);
    expect(store.byKey.get(W)!.version).toBe('v2');
  });

  it('409: banner, node replaced, the next commit uses the new version', async () => {
    const fresh = node('work', { version: 'v-disk', items: [item({ title: 'Elsewhere' })] });
    let fail = true;
    const saveTodo = vi.fn(async (req: SaveTodoReq) => {
      if (fail) {
        fail = false;
        throw new ApiError(409, 'conflict', fresh);
      }
      return node(req.path, { version: 'v9', items: req.items });
    });
    const { store } = setup({ saveTodo });
    await store.load();
    store.openPanel(W);
    expect(await store.commitItems(W, patchItem(0, { title: 'mine' }))).toBe(false);
    expect(store.panel!.banner?.text).toBe('This node changed on disk; reloaded');
    expect(store.byKey.get(W)!.version).toBe('v-disk');
    expect(store.panel!.items[0].title).toBe('Write tests');
    expect(await store.commitItems(W, patchItem(0, { title: 'mine' }))).toBe(true);
    expect(saveTodo.mock.calls[1][0].version).toBe('v-disk');
    expect(store.panel!.banner).toBeNull();
  });

  it('syncPanel waits while an item field is focused, then reloads on blur', async () => {
    let version = 'v-work';
    const getTree = vi.fn(async () => makeTree(version));
    const { store } = setup({ getTree });
    await store.load();
    store.openPanel(W);
    store.setItemFocus(true);
    version = 'v-new';
    await store.load();
    expect(store.panel!.version).toBe('v-work');
    store.setItemFocus(false);
    expect(store.panel!.version).toBe('v-new');
  });

  it('Done (closePanel) waits for a commit in flight', async () => {
    let release!: () => void;
    const saveTodo = vi.fn(
      (req: SaveTodoReq) => new Promise<Node>((res) => { release = () => res(node(req.path, { version: 'v1', items: req.items })); }),
    );
    const { store } = setup({ saveTodo });
    await store.load();
    store.openPanel(W);
    const p = store.commitItems(W, patchItem(0, { title: 'A' }));
    store.closePanel();
    expect(store.panel).not.toBeNull();
    await vi.waitFor(() => expect(saveTodo).toHaveBeenCalled());
    release();
    await p;
    await vi.waitFor(() => expect(store.panel).toBeNull());
    expect(store.byKey.get(W)!.items[0].title).toBe('A');
  });
});
