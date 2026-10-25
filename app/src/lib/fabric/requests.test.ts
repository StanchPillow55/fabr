import { describe, expect, it } from 'vitest';
import { parseEvents, unreadCount } from './events';
import { parseRecord } from './parse';
import { applyRequest, backendSilent, pendingRequests as pendingRequestsOf, submitRequest } from './requests';
import { serializeRecord } from './serialize';
import { FixtureSource, type RawRecord } from './source';
import { findByTaskPath, loadRegistry } from './view';
import { ConflictError } from './write';

const src = new FixtureSource();
const records = await src.listRecords();
const entry = (n: string) => records.find((r) => r.path.includes(n))!;
const now = new Date('2026-10-24T19:00:00.500Z');

function diff(a: unknown, b: unknown, path = ''): string[] {
  if (a !== null && b !== null && typeof a === 'object' && typeof b === 'object' && !Array.isArray(a)) {
    const keys = new Set([...Object.keys(a as object), ...Object.keys(b as object)]);
    return [...keys].flatMap((k) => diff((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k], path ? `${path}.${k}` : k));
  }
  return path === 'schema' || JSON.stringify(a) === JSON.stringify(b) ? [] : [path];
}

describe('applyRequest', () => {
  it.each([
    ['01-reminder', 'delegate'],
    ['05-running', 'cancel'],
    ['08-failed', 'redelegate'],
  ] as const)('%s %s changes only the flag and updated_at', (name, key) => {
    const before = parseRecord(entry(name).text);
    const after = parseRecord(serializeRecord(applyRequest(before, key, now)));
    expect(after.requests[key]).toBe(true);
    expect(after.updated_at).toBe('2026-10-24T19:00:00Z');
    expect(diff(before, after).sort()).toEqual(['updated_at', `requests.${key}`].sort());
    expect(before.requests[key]).toBe(false);
  });

  it('throws when the request does not apply or is already flagged', () => {
    expect(() => applyRequest(parseRecord(entry('05-running').text), 'delegate', now)).toThrow(/does not apply/);
    expect(() => applyRequest(parseRecord(entry('01-reminder').text), 'cancel', now)).toThrow(/does not apply/);
    expect(() => applyRequest(parseRecord(entry('04-awaiting').text), 'cancel', now)).toThrow(/does not apply/);
    expect(() => applyRequest(parseRecord(entry('08-failed').text), 'delegate', now)).toThrow(/does not apply/);
    const once = applyRequest(parseRecord(entry('01-reminder').text), 'delegate', now);
    expect(() => applyRequest(once, 'delegate', now)).toThrow(/already requested/);
  });
});

describe('submitRequest', () => {
  it('writes the flag, re-reads, and the second submit throws', async () => {
    const s = new FixtureSource();
    const e = (await s.listRecords()).find((r) => r.path.includes('01-reminder'))!;
    const { record, text } = await submitRequest(s, e.path, e.text, 'delegate');
    expect(record.requests.delegate).toBe(true);
    expect(await s.stamp()).toBe('1');
    await expect(submitRequest(s, e.path, text, 'delegate')).rejects.toThrow(/already requested/);
  });

  it('raises ConflictError when the record changed after it was read', async () => {
    const s = new FixtureSource();
    const e = (await s.listRecords()).find((r) => r.path.includes('05-running'))!;
    const other = serializeRecord({ ...parseRecord(e.text), updated_at: '2026-10-24T19:30:00Z' });
    await s.writeRecord(e.path, other, parseRecord(e.text).updated_at);
    await expect(submitRequest(s, e.path, e.text, 'cancel')).rejects.toBeInstanceOf(ConflictError);
  });
});

describe('backendSilent', () => {
  it('is false without a flag, false within 60 s, true after', () => {
    const r = parseRecord(entry('01-reminder').text);
    expect(backendSilent(r, '2030-01-01T00:00:00Z')).toBe(false);
    const f = applyRequest(r, 'delegate', new Date('2026-10-24T19:00:00Z'));
    expect(backendSilent(f, '2026-10-24T19:00:59Z')).toBe(false);
    expect(backendSilent(f, '2026-10-24T19:01:01Z')).toBe(true);
  });
});

describe('agents and notices wiring helpers', () => {
  it('fixture 10 gives two live and one dead entries, deduped by task path', async () => {
    const reg = loadRegistry(await src.listRegistry());
    expect(reg.entries.length).toBe(3);
    expect(reg.unreadable).toBe(0);
    expect(loadRegistry([{ path: 'x', text: 'nope: 1' }] as RawRecord[]).unreadable).toBe(1);
  });

  it('finds the in-progress record for a registry path, and none for an unknown one', async () => {
    const held = records.map((r) => ({ path: r.path, record: parseRecord(r.text) }));
    const hit = findByTaskPath(held, '/home/me/3d/.fabric-space/tasks/2026-10-24-order-filament.md');
    expect(hit?.record.state).toBe('in_progress');
    expect(findByTaskPath(held, '/home/me/docs/.fabric-space/tasks/2026-10-24-file-tax-receipts.md')).toBeUndefined();
  });

  it('raises the unread count when an event line is appended', async () => {
    const lines = await src.readEvents();
    const before = unreadCount(parseEvents(lines), parseEvents(lines).length);
    const more = [...lines, '2026-10-24T19:05:00Z failed /home/me/3d/.fabric-space/tasks/2026-10-24-order-filament.md boom'];
    expect(before).toBe(0);
    expect(unreadCount(parseEvents(more), parseEvents(lines).length)).toBe(1);
  });
});

describe('RecordScreen request actions', async () => {
  const { render } = await import('svelte/server');
  const RecordScreen = (await import('./RecordScreen.svelte')).default;
  const html = (text: string, at: Date | string = now) => render(RecordScreen, { props: { record: parseRecord(text), name: 'x', now: at, onrequest: async () => {} } }).body;
  const buttons = (b: string) => ['Delegate', 'Re-delegate', 'Cancel'].filter((l) => new RegExp(`<button[^>]*>(<!---->)*\\s*${l}\\s*(<!---->)*<`).test(b));

  it('shows one button per state and none on an awaiting-approval record', () => {
    expect(buttons(html(entry('01-reminder').text))).toEqual(['Delegate']);
    expect(buttons(html(entry('05-running').text))).toEqual(['Cancel']);
    expect(buttons(html(entry('08-failed').text))).toEqual(['Re-delegate']);
    expect(buttons(html(entry('04-awaiting').text))).toEqual([]);
  });

  it('replaces the button with queued, and adds the backend note after a minute', () => {
    const flagged = serializeRecord(applyRequest(parseRecord(entry('01-reminder').text), 'delegate', new Date('2026-10-24T19:00:00Z')));
    const fresh = html(flagged, new Date('2026-10-24T19:00:10Z'));
    expect(fresh).toContain('queued');
    expect(buttons(fresh)).toEqual([]);
    expect(fresh).not.toContain('Agent backend not connected');
    expect(html(flagged, new Date('2026-10-24T19:02:00Z'))).toContain('Agent backend not connected');
  });
});

describe('section editing', async () => {
  const { applySectionEdit, applyAssertionsEdit, applyRedo, submitSection, provenance, FROZEN_MESSAGE } = await import('./sections');
  const raw = (n: string) => records.find((r) => r.path.includes(n))!;
  const fresh = () => new FixtureSource();
  const at = new Date('2026-10-24T19:00:00Z');

  it('sets by and marks a plan redo when spec changes after define', async () => {
    const s = fresh();
    const e = (await s.listRecords()).find((r) => r.path.includes('04-awaiting'))!;
    const { record } = await submitSection(s, e.path, e.text, 'spec', 'My own spec');
    expect(record.sections.spec).toBe('My own spec');
    expect(record.by.spec).toBe('human');
    expect(record.requests.redo).toBe('plan');
    expect(provenance(record, 'spec')).toBe('you');
    expect(provenance(record, 'plan')).toBeNull();
  });

  it('marks no redo for context, plan or a pre-define edit', () => {
    const task = parseRecord(raw('02-open-question').text);
    expect(applySectionEdit(task, 'spec', 'x', at).requests.redo).toBeNull();
    const defined = parseRecord(raw('04-awaiting').text);
    expect(applySectionEdit(defined, 'context', 'x', at)).toMatchObject({ by: { context: 'human' }, requests: { redo: null } });
    expect(applySectionEdit(defined, 'plan', 'x', at).requests.redo).toBeNull();
    expect(applyAssertionsEdit(defined, 'one\ntwo', at)).toMatchObject({ by: { success: 'human' }, requests: { redo: 'plan' }, assertions: [{ id: 'a1', text: 'one' }, { id: 'a2', text: 'two' }] });
  });

  it('refuses edits once execution starts and on terminal tasks, in the apply step and the mock core', async () => {
    for (const n of ['05-running', '07-succeeded', '08-failed']) {
      const r = parseRecord(raw(n).text);
      expect(() => applySectionEdit(r, 'spec', 'x', at)).toThrow(FROZEN_MESSAGE);
      expect(() => applyRedo(r, 'plan', at)).toThrow(/frozen/);
      const s = fresh();
      const e = (await s.listRecords()).find((x) => x.path.includes(n))!;
      await expect(s.writeRecord(e.path, serializeRecord({ ...r, sections: { ...r.sections, spec: 'sneaky' } }), r.updated_at)).rejects.toThrow(/task is frozen once execution starts/);
      await expect(s.writeRecord(e.path, serializeRecord({ ...r, title: 'sneaky' }), r.updated_at)).rejects.toThrow(/frozen/);
    }
  });

  it('the mock core stamps by and redo when a client forgets to', async () => {
    const s = fresh();
    const e = (await s.listRecords()).find((r) => r.path.includes('04-awaiting'))!;
    const r = parseRecord(e.text);
    await s.writeRecord(e.path, serializeRecord({ ...r, sections: { ...r.sections, success: 'new success' } }), r.updated_at);
    const out = parseRecord(await s.readRecord(e.path));
    expect([out.by.success, out.requests.redo]).toEqual(['human', 'plan']);
  });

  it('shows Edit on todo, defined, paused and needs-harness records, not on a running or finished one', async () => {
    const { render } = await import('svelte/server');
    const RecordScreen = (await import('./RecordScreen.svelte')).default;
    const html = (rec: ReturnType<typeof parseRecord>) => render(RecordScreen, { props: { record: rec, name: 'x', now, onedit: async () => {}, onfields: async () => {} } }).body;
    const working = { ...parseRecord(raw('02-open-question').text), waiting: false, question: null };
    expect(html(working)).toContain('data-testid="edit-spec"');
    expect(html({ ...working, paused: true })).toContain('data-testid="edit-spec"');
    expect(html({ ...working, needs: 'harness: write_file' })).toContain('needs harness: harness: write_file');
    expect(html(parseRecord(raw('01-reminder').text))).toContain('data-testid="edit-context"');
    expect(html(parseRecord(raw('04-awaiting').text))).toContain('data-testid="edit-plan"');
    expect(html(parseRecord(raw('05-running').text))).not.toContain('data-testid="edit-');
    expect(html(parseRecord(raw('07-succeeded').text))).not.toContain('data-testid="edit-');
  });

  it('shows provenance and the redo label', async () => {
    const { render } = await import('svelte/server');
    const RecordScreen = (await import('./RecordScreen.svelte')).default;
    const rec = applySectionEdit(parseRecord(raw('04-awaiting').text), 'spec', 'mine', at);
    const body = render(RecordScreen, { props: { record: rec, name: 'x', now, onedit: async () => {} } }).body;
    expect(body).toContain('data-testid="prov-spec"');
    expect(body).toContain('redo requested');
  });

  it('a working task must be paused before editing, a parked or paused one opens at once', async () => {
    const { editStart } = await import('./sections');
    const { phaseOf } = await import('./phase');
    const working = { ...parseRecord(raw('02-open-question').text), waiting: false, question: null };
    const ph = (r: typeof working) => phaseOf(r, '2026-10-24T19:00');
    expect([ph(working), editStart(working, ph(working))]).toEqual(['working', 'pause']);
    expect(editStart({ ...working, paused: true }, 'paused')).toBe('open');
    expect(editStart({ ...working, needs: 'harness: write_file' }, 'needs_harness')).toBe('open');
    expect(editStart(parseRecord(raw('01-reminder').text), 'todo')).toBe('open');
    expect(editStart(parseRecord(raw('05-running').text), 'working')).toBe('frozen');
  });
});

describe('redo requests', () => {
  it('writes requests.redo for define and plan on a task or defined record only', async () => {
    const s = new FixtureSource();
    const e = (await s.listRecords()).find((r) => r.path.includes('04-awaiting'))!;
    const { record, text } = await submitRequest(s, e.path, e.text, 'redo_define');
    expect(record.requests.redo).toBe('define');
    expect(pendingRequestsOf(record)).toEqual(['redo_define']);
    await expect(submitRequest(s, e.path, text, 'redo_plan')).rejects.toThrow(/already requested/);
    expect(() => applyRequest(parseRecord(entry('05-running').text), 'redo_plan', now)).toThrow(/frozen/);
  });
});
