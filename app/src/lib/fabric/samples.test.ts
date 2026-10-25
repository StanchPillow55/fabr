import { createRawSnippet } from 'svelte';
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { basename, dirname, join, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import AgentsScreen from './AgentsScreen.svelte';
import { isDead, parseRegistryEntry } from './agents';
import { buildAgents, viewOf } from './stage';
import { parseRecord } from './parse';
import RecordScreen from './RecordScreen.svelte';
import { FabricStore } from './tasks.svelte';
import TaskSection from './TaskSection.svelte';
import type { FabricSource } from './source';

const SPACE = resolve(__dirname, '../../../../sample-fabr-space/home');
const NOW = '2026-10-25T09:00:00Z';

const records: { file: string; text: string }[] = [];
function collect(rel: string) {
  const dir = join(SPACE, rel, '.fabric-space/tasks');
  if (existsSync(dir)) for (const f of readdirSync(dir).filter((x) => x.endsWith('.md'))) records.push({ file: join(rel, f), text: readFileSync(join(dir, f), 'utf8') });
  for (const d of readdirSync(join(SPACE, rel), { withFileTypes: true })) if (d.isDirectory() && !d.name.startsWith('.')) collect(join(rel, d.name));
}
collect('');
const taskPath = (file: string) => join(SPACE, dirname(file), '.fabric-space/tasks', basename(file));
const regDir = join(SPACE, '.fabric-space/registry');
const entries = readdirSync(regDir).filter((f) => f.startsWith('20') && f.endsWith('.yaml')).map((f) => parseRegistryEntry(readFileSync(join(regDir, f), 'utf8')));

function screen(text: string, withEntry: boolean) {
  const record = parseRecord(text);
  const entry = withEntry ? entries.find((e) => record.title && e.task_id === text.match(/^id:\s*(\S+)/m)?.[1]) ?? entries[0] : undefined;
  const view = entry ? viewOf({ path: entry.task_path, record }, entry, 'connected', NOW) : null;
  const agent = createRawSnippet(() => ({
    render: () => `<div>${view ? render(AgentsScreen, { props: { agents: [view], now: NOW, detail: true } }).body : ''}</div>`,
  }));
  const noop = async () => {};
  return render(RecordScreen, { props: { record, name: 'folder', now: NOW, agent, onanswer: noop, onapprove: noop } }).body;
}

describe('sample records render in RecordScreen', () => {
  it('finds the sample records and registry entries', () => {
    expect(records.length).toBe(10);
    expect(entries.length).toBe(5);
  });
  it('registry entries carry the keys for each demo state', () => {
    const by = (id: string) => entries.find((e) => e.task_id.endsWith(id))!;
    expect(by('submit-the-devpost')).toMatchObject({ state: 'task', step: 'define', waiting: 'question', activity: 'parked' });
    expect(by('fix-the-flaky-ci')).toMatchObject({ step: 'execute', activity: 'tool' });
    expect(by('rotate-the-expiring-stripe-key')).toMatchObject({ step: 'execute', activity: 'paused' });
    expect(by('book-the-kyoto-ryokan')).toMatchObject({ step: 'approve', waiting: 'approval', activity: 'parked' });
    expect(Object.keys(by('book-the-kyoto-ryokan').usage!)).toEqual(['define', 'plan']);
    expect(Object.keys(by('fix-the-flaky-ci').usage!)).toEqual(['define', 'plan', 'execute']);
    expect(isDead(by('renew-the-gym-membership'), NOW)).toBe(true);
    expect(entries.filter((e) => isDead(e, NOW)).length).toBe(1);
  });
  it('the whole sample space builds agents for its active records', () => {
    const held = records.map((r) => ({ path: taskPath(r.file), record: parseRecord(r.text) }));
    const stages = [...buildAgents(held, entries.map((e) => ({ ...e, task_path: e.task_path.replace('<SAMPLE>', resolve(SPACE, '..')) })), 'connected', NOW).values()].map((v) => `${v.stage}:${v.alive}`).sort();
    expect(stages).toEqual(['approve:live', 'define:live', 'execute:dead', 'execute:live', 'execute:live'].sort());
  });
  for (const r of records) {
    for (const withEntry of [false, true]) {
      it(`${r.file} ${withEntry ? 'with' : 'without'} a registry entry`, () => {
        const body = screen(r.text, withEntry);
        expect(body).toContain('rec__title');
      });
    }
  }
});

describe('sample records open from the task panel', () => {
  for (const withEntry of [false, true]) {
    it(`every garden and 3d-printing record opens in the sheet ${withEntry ? 'with' : 'without'} the registry`, async () => {
      const regRaw = readdirSync(regDir).filter((f) => f.startsWith('20')).map((f) => ({ path: f, text: readFileSync(join(regDir, f), 'utf8') }));
      const source = {
        listRecords: async () => records.map((r) => ({ path: taskPath(r.file), text: r.text })),
        listRegistry: async () => (withEntry ? regRaw : []),
        readEvents: async () => [],
        eventsKey: () => 'events',
        readBackendHeartbeat: async () => null,
        stamp: async () => 'x',
      } as unknown as FabricSource;
      const fabric = new FabricStore(source);
      await fabric.load();
      for (const h of fabric.records) {
        fabric.open(h.path);
        const folder = h.path.replace(/\/\.fabric-space\/tasks\/[^/]+$/, '');
        const { body } = render(TaskSection, { props: { fabric, folder } });
        expect(body).toContain('rec__title');
      }
    });
  }
});
