import { describe, expect, it, vi } from 'vitest';
import { render } from 'svelte/server';
import type { Api } from '../api/index';
import type { Node, Tree } from '../api/types';
import { Store } from '../store.svelte';
import NodeSettingsSheet from '../settings/NodeSettingsSheet.svelte';
import { FixtureSource, taskSlug, updatedAtOf } from './source';
import { FabricStore } from './tasks.svelte';
import NewTaskSheet from './NewTaskSheet.svelte';
import { parseRecord } from './parse';

const today = new Date().toISOString().slice(0, 10);

describe('FixtureSource.createRecord', () => {
  it('derives YYYY-MM-DD-slug and suffixes collisions inside the folder', async () => {
    const s = new FixtureSource();
    const a = await s.createRecord('/home/me/p', { title: '  Order 2 kg of Black PLA!! ' });
    const b = await s.createRecord('/home/me/p', { title: 'order 2 kg of black pla' });
    const c = await s.createRecord('/home/me/other', { title: 'order 2 kg of black pla' });
    expect(a.id).toBe(`${today}-order-2-kg-of-black-pla`);
    expect(b.id).toBe(`${a.id}-2`);
    expect(c.id).toBe(a.id);
    expect(a.path).toBe(`/home/me/p/.fabric-space/tasks/${a.id}.md`);
    expect(updatedAtOf(await s.readRecord(a.path))).not.toBe('');
    expect(await s.stamp()).toBe('3');
  });
  it('caps the slug at 40 characters and refuses an empty title', async () => {
    expect(taskSlug('x'.repeat(80))).toHaveLength(40);
    expect(taskSlug('!!!')).toBe('task');
    await expect(new FixtureSource().createRecord('/a', { title: '  ' })).rejects.toThrow(/title/);
  });
  it('writes a reminder that parses back with the given sections', async () => {
    const s = new FixtureSource();
    const { path } = await s.createRecord('/a', { title: 'Pay rent', due: '2026-11-01', context: 'ctx', success: 'paid' });
    const r = parseRecord(await s.readRecord(path));
    expect(r).toMatchObject({ state: 'reminder', title: 'Pay rent', due: '2026-11-01', waiting: false, requests: { delegate: false, cancel: false, redelegate: false, pause: false, resume: false, complete: false, redo: null } });
    expect(r.sections).toMatchObject({ context: 'ctx', spec: '', success: 'paid', plan: '' });
  });
});

describe('FabricStore.createTask', () => {
  it('adds a reminder under the folder', async () => {
    const f = new FabricStore(new FixtureSource());
    await f.load();
    const h = await f.createTask('/home/me/p', { title: 'Call the bank' });
    expect(f.tasksFor('/home/me/p')).toHaveLength(1);
    expect(h.record.state).toBe('reminder');
    expect(h.record.requests).toEqual({ delegate: false, cancel: false, redelegate: false, pause: false, resume: false, complete: false, redo: null });
  });
  it('generate leaves only requests.delegate set', async () => {
    const f = new FabricStore(new FixtureSource());
    await f.load();
    const h = await f.createAndDelegate('/home/me/p', { title: 'Call the bank' });
    expect(h.record.requests).toEqual({ delegate: true, cancel: false, redelegate: false, pause: false, resume: false, complete: false, redo: null });
    expect(h.record).toMatchObject({ state: 'reminder', step: null, waiting: false, question: null });
    expect(f.tasksFor('/home/me/p')[0].record.requests.delegate).toBe(true);
  });
});

describe('add task UI', () => {
  it('the node settings sheet has the Hide row and no task section or Add task line', async () => {
    const n = (path: string, o: Partial<Node> = {}): Node => ({
      source: 'local', path, name: path || 'root', managed: true, meta: {}, items: [], warnings: [], version: 'v', via: null, children: [], ...o,
    });
    const tree: Tree = {
      root: { ...n('', { managed: false, source: '' }), children: [n('', { children: [n('work')] })] },
      sources: [{ id: 'local', name: 'Home', host: null, root: '/tmp/x', prune: null }],
      today, truncated: false, errors: [],
    };
    const api = { getTree: vi.fn(async () => tree), getStamp: vi.fn(async () => ({ stamp: 's' })) } as unknown as Api;
    const store = new Store(api);
    await store.load();
    store.openNodeSettings('local:work');
    const { body } = render(NodeSettingsSheet, { props: { store } });
    expect(body).toContain('Hide');
    expect(body).not.toContain('Add task');
    expect(body).not.toContain('data-testid="task-section"');
    expect(body).not.toContain('data-testid="remote-tasks"');
  });
  it('the New task sheet has the title, due, time, priority and three text fields and both actions', () => {
    const { body } = render(NewTaskSheet, { props: { title: 'Call the bank', onsave: () => {}, ongenerate: () => {}, oncancel: () => {} } });
    for (const t of ['New task', 'Title', 'Due', 'Time', 'Priority', 'High', 'Medium', 'Low', 'Context', 'Spec', 'Success', 'what you know', 'Save', 'Generate with AI', 'Cancel']) expect(body).toContain(t);
    expect(body).toContain('value="Call the bank"');
    expect(body.match(/<textarea/g)).toHaveLength(3);
  });
  it('an empty title disables both actions', () => {
    const { body } = render(NewTaskSheet, { props: { title: '', onsave: () => {}, ongenerate: () => {}, oncancel: () => {} } });
    expect(body.match(/<button[^>]*disabled[^>]*>/g)?.length).toBe(2);
  });
});
