import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import { parseRegistryEntry, type RegistryEntry } from './agents';
import { newRecord } from './source';
import StageBar from './StageBar.svelte';
import { GATES, STAGES, activityOf, aliveOf, ariaText, buildAgents, finishedView, groupOf, mostAdvanced, sortAgents, stageOf, usageLine, viewOf, waitingOf } from './stage';
import type { Question, TaskRecord } from './types';

const NOW = '2026-10-25T09:00:00Z';

function rec(over: Partial<TaskRecord> = {}): TaskRecord {
  return { ...newRecord('2026-10-25-t', { title: 't' }, new Date('2026-10-25T08:00:00Z')), ...over };
}

const q = (answer: Question['answer'] = null): Question => ({ id: 'q1', kind: 'open', text: 'Which?', options: null, prior: null, fields: null, asked_at: NOW, answer });

function entry(over: Record<string, unknown> = {}): RegistryEntry {
  const base = ['task_path: /x/.fabric-space/tasks/2026-10-25-t.md', 'task_id: 2026-10-25-t', 'folder: /x', 'step: execute', 'started_at: 2026-10-25T08:50:00Z', 'heartbeat_at: 2026-10-25T08:59:55Z', 'heartbeat_interval_s: 10', 'cost_usd: 0.5', 'pid: 1'];
  const lines = [...base.filter((l) => !(l.split(':')[0] in over)), ...Object.entries(over).map(([k, v]) => `${k}: ${JSON.stringify(v)}`)];
  return parseRegistryEntry(lines.join('\n'));
}

describe('stageOf, table A', () => {
  it('lists nine stages with the two gates', () => {
    expect(STAGES).toEqual(['delegate', 'gather', 'define', 'plan', 'approve', 'execute', 'evaluate', 'learn', 'notify']);
    expect(GATES).toEqual(['define', 'approve']);
  });
  it('1 delegate: a reminder with the delegate request', () => {
    expect(stageOf(rec({ requests: { delegate: true, cancel: false, redelegate: false } }))).toBe('delegate');
    expect(stageOf(rec())).toBeNull();
  });
  it('2 gather: task with step gather or null', () => {
    expect(stageOf(rec({ state: 'task', step: 'gather' }))).toBe('gather');
    expect(stageOf(rec({ state: 'task', step: null }))).toBe('gather');
  });
  it('3 define and 4 plan inside task', () => {
    expect(stageOf(rec({ state: 'task', step: 'define' }))).toBe('define');
    expect(stageOf(rec({ state: 'task', step: 'plan' }))).toBe('plan');
  });
  it('4 plan again after a refusal, 5 approve when pending or approved', () => {
    expect(stageOf(rec({ state: 'defined', step: 'approve', approval: { status: 'refused' } }))).toBe('plan');
    expect(stageOf(rec({ state: 'defined', step: 'approve', approval: { status: 'pending' } }))).toBe('approve');
    expect(stageOf(rec({ state: 'defined', step: 'approve', approval: { status: 'approved' } }))).toBe('approve');
  });
  it('6 execute and 7 evaluate; the record beats a stale registry step', () => {
    expect(stageOf(rec({ state: 'in_progress', step: 'execute' }))).toBe('execute');
    expect(stageOf(rec({ state: 'in_progress', step: 'evaluate' }), entry({ step: 'execute' }))).toBe('evaluate');
    expect(stageOf(rec({ state: 'in_progress', step: 'execute' }), entry({ step: 'evaluate' }))).toBe('execute');
    expect(stageOf(rec({ state: 'in_progress', step: null }), entry({ step: 'evaluate' }))).toBe('evaluate');
    expect(stageOf(rec({ state: 'in_progress', step: null }))).toBe('execute');
  });
  it('8 learn and 9 notify live on terminal records', () => {
    expect(stageOf(rec({ state: 'succeeded', step: 'learn' }))).toBe('learn');
    expect(stageOf(rec({ state: 'failed', step: 'notify' }))).toBe('notify');
  });
  it('finished: succeeded has no stage, failed reports where it failed', () => {
    expect(stageOf(rec({ state: 'succeeded', step: null }))).toBeNull();
    expect(stageOf(rec({ state: 'failed', step: null }))).toBe('evaluate');
    const log = (step: string, message: string) => ({ ts: NOW, step, message });
    const sections = { ...rec().sections, logEntries: [log('execute', 'started'), log('execute', 'cancelled by user')] };
    expect(stageOf(rec({ state: 'failed', step: null, sections }))).toBe('execute');
    const tail = { ...rec().sections, logEntries: [log('execute', 'x'), log('notify', 'sent')] };
    expect(stageOf(rec({ state: 'failed', step: null, sections: tail }))).toBe('execute');
  });
});

describe('waiting, activity, alive', () => {
  it('waiting: unanswered question, pending approval, else the entry', () => {
    expect(waitingOf(rec({ state: 'task', step: 'define', waiting: true, question: q() }))).toBe('question');
    expect(waitingOf(rec({ state: 'task', step: 'define', waiting: false, question: q({ value: 'a', replacement: null, granted: null, by: 'gui', at: NOW }) }))).toBeNull();
    expect(waitingOf(rec({ state: 'defined', step: 'approve', waiting: true, approval: { status: 'pending' } }))).toBe('approval');
    expect(waitingOf(rec({ state: 'in_progress', step: 'execute' }), entry({ waiting: 'question' }))).toBe('question');
    expect(waitingOf(rec({ state: 'in_progress', step: 'execute' }), entry({ waiting: null }))).toBeNull();
    expect(waitingOf(rec({ state: 'succeeded', step: 'learn' }), entry({ waiting: 'question' }))).toBeNull();
  });
  it('activity: entry value, parked, queued, tool in execute, model otherwise', () => {
    expect(activityOf(rec({ state: 'task', step: 'plan' }), entry({ activity: 'retry' }))).toBe('retry');
    expect(activityOf(rec({ state: 'task', step: 'define', waiting: true, question: q() }))).toBe('parked');
    expect(activityOf(rec({ state: 'defined', step: 'approve', approval: { status: 'approved' } }))).toBe('queued');
    expect(activityOf(rec({ state: 'in_progress', step: 'execute' }), entry())).toBe('tool');
    expect(activityOf(rec({ state: 'task', step: 'plan' }))).toBe('model');
    expect(activityOf(rec({ state: 'task', step: 'plan' }), entry({ activity: 'bogus' }))).toBe('model');
  });
  it('alive: entry heartbeat, else the backend, idle when finished', () => {
    const run = rec({ state: 'in_progress', step: 'execute' });
    expect(aliveOf(run, entry(), 'connected', NOW)).toBe('live');
    expect(aliveOf(run, entry({ heartbeat_at: '2026-10-25T08:55:00Z' }), 'connected', NOW)).toBe('dead');
    expect(aliveOf(rec({ state: 'task', step: 'plan' }), null, 'connected', NOW)).toBe('live');
    expect(aliveOf(rec({ state: 'task', step: 'plan' }), null, 'stale', NOW)).toBe('backend-down');
    expect(aliveOf(rec({ state: 'task', step: 'plan' }), null, 'absent', NOW)).toBe('backend-down');
    expect(aliveOf(rec({ state: 'succeeded', step: null }), null, 'connected', NOW)).toBe('idle');
  });
});

describe('registry parsing is lenient', () => {
  it('reads the new keys and leaves them undefined when absent', () => {
    const old = entry();
    expect([old.state, old.step_at, old.waiting, old.activity, old.retry_in_s, old.last_error, old.usage]).toEqual([undefined, undefined, undefined, undefined, undefined, undefined, undefined]);
    const e = entry({ state: 'task', step_at: '2026-10-25T08:59:00Z', waiting: 'approval', activity: 'retry', retry_in_s: 30, last_error: 'boom', usage: { plan: { calls: 2, input_tokens: 10, output_tokens: 5, usd: 0.1 }, define: { calls: 1, in: 7, out: 3, usd: 0.2 } } });
    expect(e).toMatchObject({ state: 'task', waiting: 'approval', activity: 'retry', retry_in_s: 30, last_error: 'boom' });
    expect(e.usage).toEqual({ plan: { calls: 2, input_tokens: 10, output_tokens: 5, usd: 0.1 }, define: { calls: 1, input_tokens: 7, output_tokens: 3, usd: 0.2 } });
  });
  it('drops values of the wrong type', () => {
    const e = entry({ waiting: 'someone', activity: 4, step_at: 'later', retry_in_s: 'x', usage: 3 });
    expect([e.waiting, e.activity, e.step_at, e.retry_in_s, e.usage]).toEqual([undefined, undefined, undefined, undefined, undefined]);
  });
});

describe('views', () => {
  const held = (path: string, record: TaskRecord) => ({ path, record });
  const a = held('/x/.fabric-space/tasks/2026-10-25-t.md', rec({ state: 'in_progress', step: 'execute', cost_usd: 0.2 }));
  it('builds one view per active record and keys by record path', () => {
    const idle = held('/x/.fabric-space/tasks/2026-10-25-u.md', rec({ id: '2026-10-25-u' }));
    const done = held('/x/.fabric-space/tasks/2026-10-25-v.md', rec({ id: '2026-10-25-v', state: 'succeeded', step: null }));
    const m = buildAgents([a, idle, done], [entry()], 'connected', NOW);
    expect([...m.keys()]).toEqual([a.path]);
    const v = m.get(a.path)!;
    expect(v).toMatchObject({ stage: 'execute', index: 5, gate: false, alive: 'live', cost: 0.5, folder: '/x', taskId: '2026-10-25-t', activity: 'tool' });
  });
  it('uses step_at only when the entry step is the stage, else updated_at', () => {
    const fresh = viewOf(a, entry({ step_at: '2026-10-25T08:58:00Z' }), 'connected', NOW)!;
    expect(fresh.since).toBe('2026-10-25T08:58:00Z');
    const stale = viewOf(held(a.path, { ...a.record, step: 'evaluate' }), entry({ step_at: '2026-10-25T08:58:00Z' }), 'connected', NOW)!;
    expect(stale.since).toBe(a.record.updated_at);
  });
  it('sums tokens and formats usage', () => {
    const v = viewOf(a, entry({ usage: { plan: { calls: 1, input_tokens: 1500, output_tokens: 500, usd: 0.1 }, define: { calls: 1, input_tokens: 500, output_tokens: 100, usd: 0.1 } } }), 'connected', NOW)!;
    expect(v.tokens).toEqual({ input: 2000, output: 600 });
    expect(usageLine(v)).toBe('$0.50 · 2.0k in / 600 out');
  });
  it('matches an entry by task id when paths differ and picks the freshest', () => {
    const e1 = entry({ task_path: '/elsewhere/2026-10-25-t.md', heartbeat_at: '2026-10-25T08:59:00Z' });
    const e2 = entry({ task_path: '/other/2026-10-25-t.md', heartbeat_at: '2026-10-25T08:59:50Z' });
    expect(buildAgents([a], [e1, e2], 'connected', NOW).get(a.path)!.entry).toBe(e2);
  });
  it('gates, groups and sorts', () => {
    const wait = held('/w/.fabric-space/tasks/w.md', rec({ id: 'w', state: 'task', step: 'define', waiting: true, question: q() }));
    const queued = held('/q/.fabric-space/tasks/q.md', rec({ id: 'q', state: 'defined', step: 'approve', approval: { status: 'approved' } }));
    const deadRun = held('/d/.fabric-space/tasks/d.md', rec({ id: 'd', state: 'in_progress', step: 'execute' }));
    const m = buildAgents([queued, a, wait, deadRun], [entry(), entry({ task_path: '/d/.fabric-space/tasks/d.md', task_id: 'd', heartbeat_at: '2026-10-25T08:00:00Z' })], 'connected', NOW);
    const list = [...m.values()];
    expect(m.get(wait.path)).toMatchObject({ gate: true, waiting: 'question', stage: 'define' });
    expect(list.map(groupOf).sort()).toEqual(['dead', 'needs-you', 'queued', 'running']);
    expect(sortAgents(list).map(groupOf)).toEqual(['needs-you', 'dead', 'running', 'queued']);
    expect(mostAdvanced(list)!.stage).toBe('execute');
    expect(mostAdvanced([])).toBeNull();
  });
  it('finishedView covers terminal records only', () => {
    expect(finishedView(a)).toBeNull();
    const ok = finishedView(held('/p', rec({ state: 'succeeded', step: null })))!;
    expect(ok).toMatchObject({ finished: 'succeeded', alive: 'idle', stage: 'notify' });
    expect(finishedView(held('/p', rec({ state: 'failed', step: null })))).toMatchObject({ finished: 'failed', stage: 'evaluate' });
  });
});

describe('StageBar', () => {
  const held = { path: '/x/.fabric-space/tasks/2026-10-25-t.md' };
  const mk = (record: TaskRecord, e: RegistryEntry | null, backend: 'connected' | 'stale' | 'absent' = 'connected') => viewOf({ ...held, record }, e, backend, NOW)!;
  const count = (html: string, s: string) => html.split(s).length - 1;
  it('renders nine segments with progressbar semantics', () => {
    const v = mk(rec({ state: 'in_progress', step: 'execute' }), entry());
    const { body } = render(StageBar, { props: { view: v, mode: 'compact', now: NOW } });
    expect(count(body, 'sbar__seg ')).toBe(9);
    expect(body).toContain('role="progressbar"');
    expect(body).toContain('aria-valuenow="6"');
    expect(body).toContain('aria-valuemax="9"');
    expect(body).toContain('aria-valuetext="Stage 6 of 9: execute, working"');
    expect(count(body, 'sbar__seg--done')).toBe(5);
    expect(count(body, 'sbar__seg--pulse')).toBe(1);
    expect(count(body, 'sbar__seg--gate')).toBe(2);
    expect(body).not.toContain('sbar__lab');
  });
  it('full mode labels stages and shows since and usage', () => {
    const v = mk(rec({ state: 'in_progress', step: 'execute' }), entry({ step_at: '2026-10-25T08:59:20Z' }));
    const { body } = render(StageBar, { props: { view: v, mode: 'full', now: NOW } });
    for (const s of STAGES) expect(body).toContain(`>${s}<`);
    expect(body).toContain('in execute for 40s');
    expect(body).toContain('$0.50');
  });
  it('a gate waiting on the human says needs you and does not pulse', () => {
    const v = mk(rec({ state: 'defined', step: 'approve', waiting: true, approval: { status: 'pending' } }), null);
    const { body } = render(StageBar, { props: { view: v, mode: 'full', now: NOW } });
    expect(body).toContain('sbar--wait');
    expect(body).toContain('>needs you<');
    expect(body).not.toContain('sbar__seg--pulse');
    expect(ariaText(v, NOW)).toBe('Stage 5 of 9: approve, needs you');
  });
  it('retry shows the countdown, dead shows the age, backend-down is grey', () => {
    const retry = mk(rec({ state: 'task', step: 'plan' }), entry({ step: 'plan', activity: 'retry', retry_in_s: 30, last_error: 'rate limited' }));
    const r = render(StageBar, { props: { view: retry, mode: 'full', now: NOW } }).body;
    expect(r).toContain('sbar--retry');
    expect(r).toContain('retry in 30s');
    expect(r).toContain('title="rate limited"');
    expect(r).not.toContain('sbar__seg--pulse');
    const dead = mk(rec({ state: 'in_progress', step: 'execute' }), entry({ heartbeat_at: '2026-10-25T08:57:30Z' }));
    const d = render(StageBar, { props: { view: dead, mode: 'full', now: NOW } }).body;
    expect(d).toContain('sbar--bad');
    expect(d).toContain('Dead 2m 30s');
    expect(d).not.toContain('sbar__seg--pulse');
    const down = mk(rec({ state: 'task', step: 'plan' }), null, 'stale');
    const o = render(StageBar, { props: { view: down, mode: 'full', now: NOW } }).body;
    expect(o).toContain('sbar--off');
    expect(o).toContain('Backend not responding');
  });
  it('finished bars are read only: all done or red where it failed', () => {
    const ok = finishedView({ ...held, record: rec({ state: 'succeeded', step: null }) })!;
    const b = render(StageBar, { props: { view: ok, mode: 'full', now: NOW } }).body;
    expect(count(b, 'sbar__seg--done')).toBe(9);
    expect(b).toContain('finished');
    const bad = finishedView({ ...held, record: rec({ state: 'failed', step: null }) })!;
    const f = render(StageBar, { props: { view: bad, mode: 'full', now: NOW } }).body;
    expect(f).toContain('sbar--bad');
    expect(f).toContain('failed in evaluate');
    expect(f).not.toContain('sbar__seg--pulse');
  });
});
