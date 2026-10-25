import { describe, expect, it } from 'vitest';
import { createMockApi } from '../mock/api';
import { namesToInit, nameFromPath, needsOnboarding, rootState, selectable, shouldInitRoot, slugify, toggleAll, toggleOne } from './addnodes';
import type { DirEntry, TestResult } from './api/types';
import { Store } from './store.svelte';

const dirs: DirEntry[] = [
  { name: 'a', managed: true, pruned: false },
  { name: 'b', managed: false, pruned: false },
  { name: 'node_modules', managed: false, pruned: true },
  { name: 'c', managed: false, pruned: false },
];
const res = (o: Partial<TestResult> = {}): TestResult => ({ ok: true, message: 'ok', latency_ms: 1, node_count: 0, ...o });

describe('onboarding decision', () => {
  it('shows onboarding exactly when there are no sources', () => {
    expect(needsOnboarding(null)).toBe(false);
    expect(needsOnboarding({ sources: [] })).toBe(true);
    expect(needsOnboarding({ sources: [{ id: 'x', name: 'X', host: null, root: '/x', prune: null }] })).toBe(false);
  });
  it('slugs and names', () => {
    expect(slugify('My Notes!')).toBe('my-notes');
    expect(slugify('  ')).toBe('source');
    expect(nameFromPath('/home/me/work/')).toBe('work');
  });
  it('onboard() saves the source, inits a plain root and opens Add nodes', async () => {
    const api = createMockApi({ empty: true, latency: 0, today: '2026-10-19' });
    const store = new Store(api);
    await store.load();
    expect(needsOnboarding(store.tree)).toBe(true);
    await store.onboard('/home/me/Work Stuff', '');
    expect(store.tree!.sources).toEqual([{ id: 'work-stuff', name: 'Work Stuff', host: null, root: '/home/me/Work Stuff', prune: null }]);
    expect(needsOnboarding(store.tree)).toBe(false);
    expect(store.tree!.root.name).toBe('/');
    expect(store.tree!.root.children[0].managed).toBe(true);
    expect(store.addNodesFor).toEqual({ source: 'work-stuff', path: '', name: 'Work Stuff' });
  });
});

describe('add-nodes selection', () => {
  it('only unmanaged rows are selectable (pruned ones included)', () => {
    expect(selectable(dirs)).toEqual(['b', 'node_modules', 'c']);
  });
  it('select all toggles between all and none', () => {
    const all = toggleAll(dirs, new Set());
    expect([...all]).toEqual(['b', 'node_modules', 'c']);
    expect(toggleAll(dirs, all).size).toBe(0);
    expect([...toggleAll(dirs, new Set(['b']))]).toEqual(['b', 'node_modules', 'c']);
  });
  it('init list keeps listing order and drops managed or unknown names', () => {
    expect(namesToInit(dirs, new Set(['c', 'a', 'b', 'zzz']))).toEqual(['b', 'c']);
    expect([...toggleOne(new Set(['b']), 'b')]).toEqual([]);
  });
  it('listDirs + initChildren against the mock', async () => {
    const api = createMockApi({ empty: true, latency: 0, today: '2026-10-19' });
    const store = new Store(api);
    await store.onboard('/home/me/w', 'w');
    const list = await api.listDirs({ source: 'w', path: '' });
    expect(list.find((d) => d.name === 'node_modules')?.pruned).toBe(true);
    await store.addNodes('w', '', namesToInit(list, new Set(['docs', 'notes'])));
    const after = await api.listDirs({ source: 'w', path: '' });
    expect(after.filter((d) => d.managed).map((d) => d.name)).toEqual(['docs', 'notes']);
    expect(store.tree!.root.children[0].children.map((c) => c.name)).toEqual(['docs', 'notes']);
  });
});

describe('source form init decision', () => {
  it('states', () => {
    expect(rootState(null)).toBeNull();
    expect(rootState(res({ ok: false, managed_root: false }))).toBeNull();
    expect(rootState(res())).toBeNull();
    expect(rootState(res({ managed_root: true }))).toBe('managed');
    expect(rootState(res({ managed_root: false }))).toBe('plain');
  });
  it('initialises only a plain root with the box ticked', () => {
    expect(shouldInitRoot(res({ managed_root: false }), true)).toBe(true);
    expect(shouldInitRoot(res({ managed_root: false }), false)).toBe(false);
    expect(shouldInitRoot(res({ managed_root: true }), true)).toBe(false);
    expect(shouldInitRoot(null, true)).toBe(false);
  });
});
