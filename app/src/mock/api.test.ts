import { describe, expect, it } from 'vitest';
import { ApiError, type Node } from '../lib/api/types';
import { COMMANDS } from '../lib/api/commands';
import { createMockApi } from './api';
import { markStep } from '../lib/setup/wizard';

const count = (n: Node): number => 1 + n.children.reduce((a, c) => a + count(c), 0);
const find = (n: Node, path: string): Node | undefined =>
  n.path === path ? n : n.children.map((c) => find(c, path)).find(Boolean);

describe('mock api', () => {
  it('serves the 19-node sample (node_modules pruned) with the show_depth rule', async () => {
    const api = createMockApi({ today: '2026-10-19', latency: 0 });
    const t = await api.getTree(false);
    expect(t.root.children.length).toBe(1);
    expect([t.root.name, t.root.source, t.root.managed]).toEqual(['/', '', false]);
    expect(count(t.root.children[0])).toBe(18);
    expect(find(t.root, 'personal/finance/receipts')?.managed).toBe(false);
    expect(find(t.root, 'personal/finance/receipts/2026')?.managed).toBe(true);
    expect(find(t.root, 'work/frontend/node_modules')).toBeUndefined();
    expect(find(t.root, 'archive')?.meta.hidden).toBe(true);
  });

  it('multi-source: virtual root, via labels', async () => {
    const t = await createMockApi({ today: '2026-10-19', latency: 0, multi: true }).getTree(false);
    expect(t.root.name).toBe('/');
    expect(t.root.children.map((c) => c.source)).toEqual(['local', 'lab']);
    expect(find(t.root.children[1], 'srv/wiki')?.via).toBe('srv/wiki');
  });

  it('saves bump the version and stamp; stale versions get 409 with the node', async () => {
    const api = createMockApi({ today: '2026-10-19', latency: 0 });
    const t = await api.getTree(false);
    const work = find(t.root, 'work')!;
    const s1 = (await api.getStamp()).stamp;
    const saved = await api.saveTodo({ source: 'local', path: 'work', version: work.version!, items: work.items });
    expect(saved.version).not.toBe(work.version);
    expect((await api.getStamp()).stamp).not.toBe(s1);
    const err = await api.saveTodo({ source: 'local', path: 'work', version: work.version!, items: [] }).catch((e) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err.code).toBe(409);
    expect(err.node.version).toBe(saved.version);
  });

  it('implements exactly the COMMANDS operations', () => {
    expect(Object.keys(createMockApi({ latency: 0 })).sort()).toEqual(Object.keys(COMMANDS).sort());
  });

  it('completing all five setup steps flips setup.completed', async () => {
    const api = createMockApi({ latency: 0 });
    const first = await api.fabricSettingsRead();
    expect([first.exists, first.settings.setup.completed]).toEqual([false, false]);
    let cur = first.settings;
    for (const step of ['folder', 'api_key', 'model', 'telegram_bot', 'telegram_link'] as const) {
      cur = markStep(cur, step);
      const r = await api.fabricSettingsWrite(cur);
      expect(r.settings.setup.completed).toBe(step === 'telegram_link');
    }
    expect(await api.fabricSettingsRead()).toMatchObject({ exists: true, settings: { setup: { completed: true } } });
  });
});
