import { describe, expect, it, vi } from 'vitest';
import { createMockApi } from '../../mock/api';
import { Store } from '../store.svelte';
import { SettingsController } from './controller.svelte';
import sheet from './SettingsSheet.svelte?raw';
import source from './SourcePage.svelte?raw';

async function open() {
  const api = createMockApi({ latency: 0 });
  const store = new Store(api);
  await store.start();
  store.stop();
  store.sheetOpen = true;
  const ctl = new SettingsController(store);
  return { api, store, ctl };
}

describe('settings navigation flow (mock backend)', () => {
  it('Sources > Add > fill > Back persists and the list shows it', async () => {
    const { api, store, ctl } = await open();
    const before = store.sources.length;
    ctl.push({ kind: 'sources' });
    const d = ctl.addSource();
    expect(ctl.title).toBe('New source');
    expect(ctl.backLabel).toBe('Cancel');
    d.src.name = 'Work box';
    d.src.root = '/tmp/work';
    expect(ctl.backLabel).toBe('Sources');
    expect(await ctl.back()).toBe(true);
    expect(ctl.page.kind).toBe('sources');
    expect(ctl.drafts).toHaveLength(before + 1);
    const saved = (await api.listSources()).find((s) => s.name === 'Work box');
    expect(saved).toMatchObject({ id: 'work-box', root: '/tmp/work', host: null });
    expect(store.sources.map((s) => s.id)).toContain('work-box');
    expect(ctl.drafts.at(-1)?.isNew).toBe(false);
  });

  it('Cancel on an untouched new draft discards it; an invalid one stays open', async () => {
    const { api, store, ctl } = await open();
    const n = store.sources.length;
    ctl.push({ kind: 'sources' });
    ctl.addSource();
    expect(await ctl.back()).toBe(true);
    expect(ctl.drafts).toHaveLength(n);
    expect((await api.listSources()).length).toBe(n);
    const d = ctl.addSource();
    d.src.name = 'No root';
    expect(await ctl.back()).toBe(false);
    expect(ctl.page.kind).toBe('source');
    expect(ctl.errors.root).toBeTruthy();
    d.depthText = '40';
    d.src.root = '/x';
    expect(await ctl.back()).toBe(false);
    expect(ctl.errors.depth).toBeTruthy();
    d.depthText = '3';
    expect(await ctl.done()).toBe(true);
    expect(store.sheetOpen).toBe(false);
    expect(store.sources.find((s) => s.name === 'No root')?.search_depth).toBe(3);
  });

  it('a new draft never reaches the server with an empty id (test, password, delete of another source)', async () => {
    const { api, store, ctl } = await open();
    const test = vi.spyOn(api, 'testSource');
    const save = vi.spyOn(api, 'saveSources');
    ctl.push({ kind: 'sources' });
    const d = ctl.addSource();
    d.src.name = '🚀';
    d.src.host = 'devbox';
    d.src.root = '/tmp/x';
    await d.flow.test();
    expect(d.flow.error).toBeNull();
    expect(test.mock.calls[0][0].id).toBe('source');
    d.flow.password = 'pw';
    await d.flow.clearPassword();
    expect(d.flow.error).toBeNull();
    ctl.stack = [{ kind: 'root' }, { kind: 'sources' }, { kind: 'source', uid: ctl.drafts[0].uid }];
    ctl.drafts[0].src.name = 'Home';
    await ctl.deleteDraft();
    for (const call of save.mock.calls) for (const s of call[0]) expect(s.id).toMatch(/^[a-z0-9_-]+$/);
    expect(store.sources.every((s) => s.id)).toBe(true);
  });

  it('a cancelled draft clears its error; validation errors name the row', async () => {
    const { ctl } = await open();
    ctl.push({ kind: 'sources' });
    const d = ctl.addSource();
    d.src.name = 'Lost';
    expect(await ctl.back()).toBe(false);
    expect(ctl.error).toMatch(/^“Lost”: /);
    d.src.name = '';
    expect(await ctl.back()).toBe(true);
    expect(ctl.error).toBeNull();
    expect(ctl.drafts.some((x) => x === d)).toBe(false);
  });

  it('Back from root closes; Done closes from a deep page; reset restores the root page', async () => {
    const { store, ctl } = await open();
    ctl.push({ kind: 'appearance' });
    expect(await ctl.back()).toBe(true);
    expect(store.sheetOpen).toBe(true);
    expect(ctl.page.kind).toBe('root');
    ctl.push({ kind: 'keys' });
    expect(await ctl.done()).toBe(true);
    expect(store.sheetOpen).toBe(false);
    ctl.reset();
    expect(ctl.stack).toEqual([{ kind: 'root' }]);
  });

  it('openSettings("sources") opens the sheet on Settings > Sources; Back leads to the root page', async () => {
    const { store, ctl } = await open();
    store.sheetOpen = false;
    store.openSettings('sources');
    expect([store.sheetOpen, store.sheetPage]).toEqual([true, 'sources']);
    ctl.reset(store.sheetPage);
    expect(ctl.page.kind).toBe('sources');
    expect(await ctl.back()).toBe(true);
    expect(ctl.page.kind).toBe('root');
    store.openSettings();
    expect(store.sheetPage).toBe(null);
  });

  it('delete removes the source and pops; the last source cannot be deleted', async () => {
    const { api, store, ctl } = await open();
    ctl.push({ kind: 'sources' });
    const d = ctl.addSource();
    d.src.root = '/tmp/gone';
    await ctl.back();
    ctl.push({ kind: 'source', uid: d.uid });
    expect(ctl.canDelete).toBe(true);
    await ctl.deleteDraft();
    expect(ctl.page.kind).toBe('sources');
    expect((await api.listSources()).some((s) => s.root === '/tmp/gone')).toBe(false);
    expect(store.sources.length).toBeGreaterThan(0);
    while (ctl.drafts.length > 1) {
      ctl.push({ kind: 'source', uid: ctl.drafts[0].uid });
      await ctl.deleteDraft();
    }
    ctl.push({ kind: 'source', uid: ctl.drafts[0].uid });
    expect(ctl.canDelete).toBe(false);
  });

  it('Advanced saves converted units on commit and keeps invalid fields out', async () => {
    const { api, ctl } = await open();
    ctl.push({ kind: 'advanced' });
    ctl.adv.poll = '0.5';
    ctl.adv.remote = '45';
    ctl.adv.rescan = '10';
    ctl.adv.depth = '7';
    ctl.adv.maxNodes = '5000';
    expect(await ctl.commitAdvanced()).toBe(true);
    expect(await api.getSettings()).toMatchObject({ poll_ms: 500, remote_poll_ms: 45000, full_rescan_ms: 600000, search_depth: 7, max_nodes: 5000 });
    ctl.adv.poll = '0.1';
    ctl.adv.depth = '9';
    expect(await ctl.commitAdvanced()).toBe(false);
    expect(ctl.advErrors.poll).toBeTruthy();
    const s = await api.getSettings();
    expect(s.poll_ms).toBe(500);
    expect(s.search_depth).toBe(9);
    expect(await ctl.back()).toBe(false);
    ctl.adv.poll = '2';
    expect(await ctl.back()).toBe(true);
    expect((await api.getSettings()).poll_ms).toBe(2000);
  });
});

describe('settings sheet markup', () => {
  it('has no Save button, Esc goes back, and the source page keeps the ssh panels', () => {
    expect(sheet).not.toMatch(/>\s*Save\s*</);
    expect(sheet).toContain("e.key !== 'Escape'");
    expect(sheet).toContain('ctl.back()');
    expect(sheet).toContain('ctl.reset(untrack(() => store.sheetPage))');
    expect(source).toContain('Trust this host');
    expect(source).toContain('Delete source');
  });
});

describe('stale test results', () => {
  it('a later clean tree load clears an old failed Test on a saved, unchanged source', async () => {
    const { store, ctl } = await open();
    const d = ctl.drafts[0];
    d.flow.result = { ok: false, message: 'Unknown host key: matrix', latency_ms: 0, node_count: 0, failure_kind: 'host_key_unknown' };
    expect(ctl.status(d)).toBe('error');
    ctl.reconcile();
    expect(d.flow.result).toBeNull();
    expect(ctl.status(d)).toBe('ok');
    d.flow.result = { ok: false, message: 'x', latency_ms: 0, node_count: 0 };
    d.src.root = '/changed';
    ctl.reconcile();
    expect(d.flow.result).not.toBeNull();
    expect(store.tree?.errors).toEqual([]);
  });
});
