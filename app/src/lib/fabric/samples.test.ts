import { createRawSnippet } from 'svelte';
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import AgentsScreen from './AgentsScreen.svelte';
import { parseRegistryEntry } from './agents';
import { buildAgents, viewOf } from './stage';
import { parseRecord } from './parse';
import RecordScreen from './RecordScreen.svelte';
import { FabricStore } from './tasks.svelte';
import TaskSection from './TaskSection.svelte';
import type { FabricSource } from './source';

const SPACE = resolve(__dirname, '../../../../sample-fabr-space/home');
const NOW = '2026-10-25T09:00:00Z';

const records: { file: string; text: string }[] = [];
for (const p of readdirSync(join(SPACE, 'projects'))) {
  const dir = join(SPACE, 'projects', p, '.fabric-space/tasks');
  if (!existsSync(dir)) continue;
  for (const f of readdirSync(dir).filter((x) => x.endsWith('.md'))) records.push({ file: `${p}/${f}`, text: readFileSync(join(dir, f), 'utf8') });
}
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
    expect(records.length).toBeGreaterThan(5);
    expect(entries.length).toBe(6);
  });
  it('registry entries carry the new keys for the gate and retry samples', () => {
    const by = (id: string) => entries.find((e) => e.task_id.endsWith(id))!;
    expect(by('order-filament')).toMatchObject({ state: 'task', step: 'define', waiting: 'question', activity: 'parked' });
    expect(by('renew-car-registration')).toMatchObject({ activity: 'retry', retry_in_s: 30, last_error: 'anthropic 429: rate limited' });
    expect(by('order-replacement-nozzle')).toMatchObject({ step: 'approve', waiting: 'approval' });
    expect(Object.keys(by('order-replacement-nozzle').usage!)).toEqual(['define', 'plan']);
  });
  it('the whole sample space builds agents for its active records', () => {
    const held = records.map((r) => ({ path: join(SPACE, 'projects', r.file.replace('/', '/.fabric-space/tasks/')), record: parseRecord(r.text) }));
    const stages = [...buildAgents(held, entries.map((e) => ({ ...e, task_path: e.task_path.replace('<SAMPLE>', resolve(SPACE, '..')) })), 'connected', NOW).values()].map((v) => `${v.stage}:${v.alive}`).sort();
    expect(stages).toEqual(['approve:live', 'define:live', 'define:live', 'execute:live', 'execute:live'].sort());
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
        listRecords: async () => records.map((r) => ({ path: join(SPACE, 'projects', r.file.replace('/', '/.fabric-space/tasks/')), text: r.text })),
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
