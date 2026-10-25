import { describe, expect, it } from 'vitest';
import { createMockApi } from '../mock/api';
import { ApiError, type Node } from './api/types';
import { Store } from './store.svelte';

const find = (n: Node, path: string): Node | undefined => (n.path === path ? n : n.children.map((c) => find(c, path)).find(Boolean));

describe('remove node against the mock', () => {
  it('children re-attach to the nearest managed ancestor with via; the sheet closes', async () => {
    const store = new Store(createMockApi({ latency: 0, today: '2026-10-19' }));
    await store.load();
    store.openNodeSettings('local:work/api-server');
    expect(await store.removeNode()).toBe(true);
    expect(store.nodeSettings).toBeNull();
    const root = store.tree!.root;
    expect(find(root, 'work/api-server')).toBeUndefined();
    const work = find(root, 'work')!;
    for (const [p, via] of [['work/api-server/docs', 'api-server/docs'], ['work/api-server/tests', 'api-server/tests']]) {
      const c = work.children.find((x) => x.path === p)!;
      expect(c.managed).toBe(true);
      expect(c.via).toBe(via);
    }
    expect(store.notice).toBe('Removed node api-server; the folder is still there');
  });

  it('refuses a stale version (409 with the node), a plain dir (404) and the virtual root (400)', async () => {
    const api = createMockApi({ latency: 0, today: '2026-10-19', multi: true });
    const t = await api.getTree(false);
    const work = find(t.root, 'work')!;
    const e = await api.removeNode({ source: 'local', path: 'work', version: 'stale' }).catch((x) => x);
    expect(e).toBeInstanceOf(ApiError);
    expect([e.code, e.node?.path]).toEqual([409, 'work']);
    expect((await api.removeNode({ source: 'local', path: 'work/frontend/node_modules', version: 'x' }).catch((x) => x)).code).toBe(404);
    expect((await api.removeNode({ source: '', path: '', version: 'x' }).catch((x) => x)).code).toBe(400);
    const plain = await api.removeNode({ source: 'local', path: 'work', version: work.version! });
    expect(plain.managed).toBe(false);
  });
});
