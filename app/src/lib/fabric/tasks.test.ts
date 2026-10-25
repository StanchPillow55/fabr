import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import { FixtureSource, type FabricSource } from './source';
import { nodeFolder } from './folder';
import TaskSection from './TaskSection.svelte';
import TaskStrip from './TaskStrip.svelte';
import { FabricStore, localRoots } from './tasks.svelte';

const read = (p: string) => readFileSync(new URL(p, import.meta.url), 'utf8');

async function fixtureStore() {
  const f = new FabricStore(new FixtureSource());
  await f.load();
  return f;
}

async function realPathStore(): Promise<FabricStore> {
  const fx = await new FixtureSource().listRecords();
  const pick = (n: string) => fx.find((r) => r.path.includes(n))!.text;
  const src: FabricSource = {
    ...new FixtureSource(),
    listRecords: async () => [
      { path: '/home/me/proj/.fabric-space/tasks/a.md', text: pick('01-reminder') },
      { path: '/home/me/proj/.fabric-space/tasks/b.md', text: pick('02-open-question') },
    ],
    readRecord: async () => '', createRecord: async () => ({ path: '', id: '' }), writeRecord: async () => {}, listRegistry: async () => [], readEvents: async () => [],
    eventsKey: () => 'k', readBackendHeartbeat: async () => null, stamp: async () => '0',
  };
  const f = new FabricStore(src);
  await f.load();
  return f;
}

describe('FabricStore over the fixtures', () => {
  it('groups records by folder', async () => {
    const f = await fixtureStore();
    expect(f.records.length).toBe(10);
    expect(f.byFolder.size).toBe(9);
    expect(f.tasksFor('09-redelegated-pair').length).toBe(2);
    expect(f.tasksFor('nowhere')).toEqual([]);
    expect(f.tasksFor(null)).toEqual([]);
  });
  it('counts the five states and waiting', async () => {
    const f = await fixtureStore();
    expect(f.countsFor('09-redelegated-pair')).toMatchObject({ byColumn: { task: 0, defined: 1, done: 1, reminder: 0 }, waiting: 1, total: 2 });
    expect(f.countsFor('02-open-question')).toMatchObject({ byColumn: { task: 1 }, waiting: 1, total: 1 });
    expect(Object.keys(f.countsFor('02-open-question').byColumn)).toEqual(['reminder', 'task', 'defined', 'in_progress', 'done']);
  });
  it('finds the running agent for the in-progress folder only', async () => {
    const f = await fixtureStore();
    expect(f.runningFor('05-running')?.step).toBeTruthy();
    expect(f.runningFor('02-open-question')).toBeNull();
    expect(f.entryFor(f.tasksFor('05-running')[0].path)).not.toBeNull();
  });
  it('parses events and starts with everything unread', async () => {
    const f = await fixtureStore();
    expect(f.unread).toBe(f.events.length);
    expect(f.backend).toBe('absent');
    expect(f.backendLabel).toBe('Agent backend not connected');
  });
  it('answers through the shared write path and re-reads', async () => {
    const f = await fixtureStore();
    const h = f.tasksFor('02-open-question')[0];
    await f.answer(h, { value: 'Use the blue one' });
    const after = f.tasksFor('02-open-question')[0];
    expect(after.record.question?.answer?.value).toBe('Use the blue one');
  });
  it('requests through the shared write path', async () => {
    const f = await fixtureStore();
    const h = f.tasksFor('01-reminder')[0];
    await f.request(h, 'delegate');
    expect(f.tasksFor('01-reminder')[0].record.requests.delegate).toBe(true);
  });
  it('polls the source on start and stops', async () => {
    const f = new FabricStore(new FixtureSource());
    await f.start();
    expect(f.loaded).toBe(true);
    f.stop();
  });
});

describe('nodeFolder and roots', () => {
  it('joins a local source root and node path', () => {
    expect(nodeFolder({ host: null, root: '/home/me/' }, 'a/b')).toBe('/home/me/a/b');
    expect(nodeFolder({ host: null, root: '/home/me' }, '')).toBe('/home/me');
    expect(nodeFolder({ host: null, root: '~/notes' }, 'x', '/home/me')).toBe('/home/me/notes/x');
    expect(nodeFolder({ host: null, root: '~/notes' }, 'x')).toBeNull();
  });
  it('gives remote nodes no folder and keeps local roots only', () => {
    expect(nodeFolder({ host: 'box', root: '/srv' }, 'a')).toBeNull();
    expect(nodeFolder(undefined, 'a')).toBeNull();
    const s = (id: string, host: string | null, root: string) => ({ id, name: id, host, root, prune: null });
    expect(localRoots([s('a', null, '/x'), s('b', 'h', '/y')])).toEqual(['/x']);
  });
});

describe('task strip', () => {
  it('shows counts and the waiting mark for two records, one waiting', async () => {
    const f = await realPathStore();
    const { body } = render(TaskStrip, { props: { fabric: f, folder: '/home/me/proj' } });
    expect(body).toContain('data-testid="task-strip"');
    expect(body).toMatch(/1<span class="tstrip__k[^"]*">reminder/);
    expect(body).toMatch(/1<span class="tstrip__k[^"]*">task/);
    expect(body).toContain('waiting');
    expect(body).not.toContain('running');
    expect(body).not.toContain('defined');
  });
  it('renders nothing for a folder without records', async () => {
    const f = await realPathStore();
    expect(render(TaskStrip, { props: { fabric: f, folder: '/home/me/other' } }).body).not.toContain('task-strip');
  });
  it('shows the running mark when the registry has an entry', async () => {
    const f = await fixtureStore();
    expect(render(TaskStrip, { props: { fabric: f, folder: '05-running' } }).body).toContain('running');
  });
});

describe('task section', () => {
  it('lists the folder records', async () => {
    const f = await realPathStore();
    const { body } = render(TaskSection, { props: { fabric: f, folder: '/home/me/proj' } });
    for (const h of f.tasksFor('/home/me/proj')) expect(body).toContain(h.record.title);
    expect(body).toContain('waiting');
    expect(body).not.toContain('role="dialog"');
  });
  it('renders the record screen with the question panel once a record is selected', async () => {
    const f = await fixtureStore();
    const h = f.tasksFor('02-open-question')[0];
    f.open(h.path);
    const { body } = render(TaskSection, { props: { fabric: f, folder: '02-open-question' } });
    expect(body).toContain('role="dialog"');
    expect(body).toContain(h.record.question!.text);
    expect(body).toContain('Close');
  });
  it('shows agent details above the record for a running task', async () => {
    const f = await fixtureStore();
    f.open(f.tasksFor('05-running')[0].path);
    const body = render(TaskSection, { props: { fabric: f, folder: '05-running' } }).body;
    expect(body).toContain('data-testid="agent-row"');
    expect(body).toContain('Cancel');
  });
});

describe('Tasks view is gone', () => {
  const app = read('../../App.svelte');
  const toolbar = read('../Toolbar.svelte');
  it('App has no FabricView, g key or fabricOpen', () => {
    expect(app).not.toMatch(/FabricView|fabricOpen|'fabric'/);
    expect(toolbar).not.toMatch(/onfabric|>Tasks</);
    expect(read('../viewstate.ts')).not.toMatch(/'g'|Open Tasks/);
  });
  it('Panel and NodeCard use the fabric components', () => {
    expect(read('../panel/Panel.svelte')).toContain('TaskSection');
    expect(read('../tree/NodeCard.svelte')).toContain('TaskStrip');
    expect(toolbar).toContain('NoticesBadge');
  });
});
