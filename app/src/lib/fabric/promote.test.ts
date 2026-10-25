import { describe, expect, it, vi } from 'vitest';
import { render } from 'svelte/server';
import type { Item } from '../api/types';
import ItemEditor from '../panel/ItemEditor.svelte';
import { KEPT_MESSAGE, promote, promoteFields } from './promote';
import { FixtureSource } from './source';
import { FabricStore } from './tasks.svelte';

const item = (o: Partial<Item> = {}): Item => ({ id: null, status: 'todo', due: null, title: ' Call the bank ', pri: null, ...o });

describe('promoteFields', () => {
  it('carries the title and the todo context line', () => {
    expect(promoteFields(item(), '/home/me/work/')).toEqual({ title: 'Call the bank', due: null, context: 'From the todo list in work.' });
  });
  it('carries the due date when present and null when absent', () => {
    expect(promoteFields(item({ due: '2026-11-01' }), '/a/b').due).toBe('2026-11-01');
    expect(promoteFields(item(), '/a/b').due).toBeNull();
  });
});

describe('promote', () => {
  it('creates the record and removes the todo', async () => {
    const f = new FabricStore(new FixtureSource());
    await f.load();
    const remove = vi.fn(async () => true);
    const r = await promote(f, '/home/me/p', promoteFields(item(), '/home/me/p'), false, remove);
    expect(r.kind).toBe('done');
    expect(remove).toHaveBeenCalledOnce();
    const [h] = f.tasksFor('/home/me/p');
    expect(h.record.title).toBe('Call the bank');
    expect(h.record.sections.context).toBe('From the todo list in p.');
  });
  it('keeps the todo when creation fails', async () => {
    const f = new FabricStore(new FixtureSource());
    await f.load();
    const remove = vi.fn(async () => true);
    const r = await promote(f, '/home/me/p', { title: ' ' }, false, remove);
    expect(r).toMatchObject({ kind: 'failed', error: expect.stringMatching(/title/) });
    expect(remove).not.toHaveBeenCalled();
    expect(f.tasksFor('/home/me/p')).toHaveLength(0);
  });
  it('reports a kept task when removal fails or throws', async () => {
    const f = new FabricStore(new FixtureSource());
    await f.load();
    for (const remove of [async () => false, async () => { throw new Error('x'); }]) {
      expect((await promote(f, '/home/me/p', promoteFields(item(), '/home/me/p'), false, remove)).kind).toBe('kept');
    }
    expect(f.tasksFor('/home/me/p')).toHaveLength(2);
    expect(KEPT_MESSAGE).toMatch(/could not be removed/);
  });
});

describe('item editor', () => {
  const props = (items: Item[]) => ({ items, oncommit: async () => true, fabric: new FabricStore(new FixtureSource()), folder: '/a/b' });
  it('shows Make task for todo and wip items, not done ones', () => {
    expect(render(ItemEditor, { props: props([item()]) }).body).toContain('Make task');
    expect(render(ItemEditor, { props: props([item({ status: 'wip' })]) }).body).toContain('Make task');
    expect(render(ItemEditor, { props: props([item({ status: 'done' })]) }).body).not.toContain('Make task');
  });
  it('hides it without a fabric folder', () => {
    expect(render(ItemEditor, { props: { items: [item()], oncommit: async () => true } }).body).not.toContain('Make task');
  });
});
