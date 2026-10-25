import { formatDuration, isDead, type Activity, type RegistryEntry, type StageUsage, type Waiting } from './agents';
import type { BackendState } from './backend';
import { minuteOf } from '../model';
import { folderOf } from './folder';
import { PHASE_LABEL, phaseOf, type Phase } from './phase';
import type { TaskRecord } from './types';
import { entryMap } from './view';

export const STAGES = ['delegate', 'gather', 'define', 'plan', 'approve', 'execute', 'evaluate', 'learn', 'notify'] as const;
export type Stage = (typeof STAGES)[number];
export const GATES: readonly Stage[] = ['define', 'approve'];

export type Alive = 'live' | 'dead' | 'backend-down' | 'idle';

export interface AgentView {
  path: string;
  taskPath: string;
  taskId: string;
  title: string;
  folder: string;
  stage: Stage;
  index: number;
  gate: boolean;
  activity: Activity;
  waiting: Waiting | null;
  since: string;
  cost: number;
  tokens: { input: number; output: number } | null;
  stageUsage: Record<string, StageUsage> | null;
  alive: Alive;
  lastError: string | null;
  retryInS: number | null;
  finished: 'succeeded' | 'failed' | null;
  phase: Phase;
  suspendUntil: string | null;
  entry: RegistryEntry | null;
}

type Held = { path: string; record: TaskRecord };

const isStage = (s: string | null | undefined): s is Stage => !!s && (STAGES as readonly string[]).includes(s);

function failedIn(record: TaskRecord): Stage {
  const log = record.sections.logEntries;
  for (let i = log.length - 1; i >= 0; i--) {
    const s = log[i].step;
    if (isStage(s) && s !== 'delegate' && s !== 'learn' && s !== 'notify') return s;
  }
  return 'evaluate';
}

export function suspendLabel(r: Pick<TaskRecord, 'suspend_until'>): string {
  return r.suspend_until ? `Suspended until ${r.suspend_until.replace('T', ' ')}` : 'Suspended';
}

export function isFinished(record: TaskRecord): boolean {
  return (record.state === 'succeeded' || record.state === 'failed') && record.step !== 'learn' && record.step !== 'notify';
}

export function stageOf(record: TaskRecord, entry?: RegistryEntry | null): Stage | null {
  const step = isStage(record.step) ? record.step : null;
  switch (record.state) {
    case 'reminder':
      return record.requests.delegate ? 'delegate' : null;
    case 'task':
      return step === 'define' || step === 'plan' ? step : 'gather';
    case 'defined':
      return record.approval?.status === 'refused' ? 'plan' : 'approve';
    case 'in_progress':
      if (step) return step === 'evaluate' ? 'evaluate' : 'execute';
      return entry?.step === 'evaluate' ? 'evaluate' : 'execute';
    default:
      if (step === 'learn' || step === 'notify') return step;
      return record.state === 'failed' ? failedIn(record) : null;
  }
}

export function waitingOf(record: TaskRecord, entry?: RegistryEntry | null): Waiting | null {
  if (record.state === 'defined' && record.approval?.status === 'pending') return 'approval';
  if (record.waiting && record.question && !record.question.answer) return 'question';
  if (record.state === 'task' || record.state === 'defined' || record.state === 'in_progress') return entry?.waiting ?? null;
  return null;
}

export function activityOf(record: TaskRecord, entry?: RegistryEntry | null): Activity {
  if (entry?.activity) return entry.activity;
  if (record.paused && (record.state === 'task' || record.state === 'defined' || record.state === 'in_progress')) return 'paused';
  if (waitingOf(record, entry)) return 'parked';
  if (record.state === 'defined' && record.approval?.status === 'approved') return 'queued';
  return record.state === 'in_progress' && entry ? 'tool' : 'model';
}

export function aliveOf(record: TaskRecord, entry: RegistryEntry | null | undefined, backend: BackendState, now: Date | string = new Date(), skewMs = 0): Alive {
  if (isFinished(record)) return 'idle';
  if (entry) return isDead(entry, new Date(new Date(now).getTime() + skewMs)) ? 'dead' : 'live';
  return backend === 'connected' ? 'live' : 'backend-down';
}

function tokensOf(usage: Record<string, StageUsage> | undefined): { input: number; output: number } | null {
  const all = usage ? Object.values(usage) : [];
  if (all.length === 0) return null;
  return { input: all.reduce((a, u) => a + u.input_tokens, 0), output: all.reduce((a, u) => a + u.output_tokens, 0) };
}

export function viewOf(held: Held, entry: RegistryEntry | null, backend: BackendState, now: Date | string = new Date(), phase: Phase = phaseOf(held.record, minuteOf(now)), skewMs = 0): AgentView | null {
  const r = held.record;
  const finished = isFinished(r) ? (r.state as 'succeeded' | 'failed') : null;
  const stage = stageOf(r, entry) ?? (finished ? 'notify' : null);
  if (!stage) return null;
  const sameStep = entry?.step_at && entry.step === stage;
  return {
    path: held.path,
    taskPath: entry?.task_path ?? held.path,
    taskId: r.id,
    title: r.title,
    folder: folderOf(held.path),
    stage,
    index: STAGES.indexOf(stage),
    gate: GATES.includes(stage),
    activity: activityOf(r, entry),
    waiting: waitingOf(r, entry),
    since: sameStep ? entry!.step_at! : r.updated_at,
    cost: Math.max(r.cost_usd, entry?.cost_usd ?? 0),
    tokens: tokensOf(entry?.usage),
    stageUsage: entry?.usage ?? null,
    alive: aliveOf(r, entry, backend, now, skewMs),
    lastError: entry?.last_error ?? null,
    retryInS: entry?.activity === 'retry' && typeof entry.retry_in_s === 'number' ? entry.retry_in_s : null,
    finished,
    phase,
    suspendUntil: r.suspend_until,
    entry,
  };
}

export function buildAgents(records: Held[], entries: RegistryEntry[], backend: BackendState, now: Date | string = new Date(), phases?: Map<string, Phase>, skewMs = 0): Map<string, AgentView> {
  const byRecord = entryMap(records, entries);
  const out = new Map<string, AgentView>();
  for (const h of records) {
    const phase = phases?.get(h.path) ?? phaseOf(h.record, minuteOf(now));
    if (isFinished(h.record) || phase === 'suspended') continue;
    const v = viewOf(h, byRecord.get(h.path) ?? null, backend, now, phase, skewMs);
    if (v) out.set(h.path, v);
  }
  return out;
}

export function suspendedView(held: Held, backend: BackendState, now: Date | string = new Date()): AgentView | null {
  const phase = phaseOf(held.record, minuteOf(now));
  return phase === 'suspended' ? viewOf(held, null, backend, now, phase) : null;
}

export function finishedView(held: Held): AgentView | null {
  return isFinished(held.record) ? viewOf(held, null, 'absent') : null;
}

export type AgentGroup = 'needs-you' | 'paused' | 'dead' | 'running' | 'queued';
export const GROUPS: readonly AgentGroup[] = ['needs-you', 'paused', 'dead', 'running', 'queued'];
export const GROUP_LABEL: Record<AgentGroup, string> = { 'needs-you': 'Needs you', paused: 'Paused', dead: 'Dead', running: 'Running', queued: 'Queued' };

export function groupOf(v: AgentView): AgentGroup {
  if (v.phase === 'needs_you') return 'needs-you';
  if (v.alive === 'dead') return 'dead';
  if (v.phase === 'paused') return 'paused';
  return v.activity === 'queued' ? 'queued' : 'running';
}

export function sortAgents(list: AgentView[]): AgentView[] {
  const order = (v: AgentView) => GROUPS.indexOf(groupOf(v));
  return [...list].sort((a, b) => order(a) - order(b) || Date.parse(a.since) - Date.parse(b.since));
}

export function leadOf(list: AgentView[]): AgentView | null {
  return mostAdvanced(list.filter((v) => v.phase === 'working')) ?? mostAdvanced(list.filter((v) => v.phase === 'paused'));
}

export function sinceSeconds(v: AgentView, now: Date | string): number {
  return Math.max(0, (new Date(now).getTime() - Date.parse(v.since)) / 1000);
}

export function mostAdvanced(list: AgentView[]): AgentView | null {
  return list.reduce<AgentView | null>((best, v) => (!best || v.index > best.index ? v : best), null);
}

export function formatTokens(n: number): string {
  return n >= 1000 ? `${(n / 1000).toFixed(1)}k` : String(n);
}

export function usageLine(v: AgentView): string {
  const parts = [`$${v.cost.toFixed(2)}`];
  if (v.tokens) parts.push(`${formatTokens(v.tokens.input)} in / ${formatTokens(v.tokens.output)} out`);
  return parts.join(' · ');
}

export function statusOf(v: AgentView, now: Date | string = new Date()): string {
  if (v.phase === 'suspended') return v.suspendUntil ? `Suspended until ${v.suspendUntil.replace('T', ' ')}` : 'Suspended';
  if (v.finished) return v.finished === 'failed' ? `failed in ${v.stage}` : 'finished';
  if (v.alive === 'dead' && v.entry) return `Dead ${formatDuration(Math.max(0, (new Date(now).getTime() - Date.parse(v.entry.heartbeat_at)) / 1000))}`;
  if (v.alive === 'backend-down') return 'Backend not responding';
  if (v.phase === 'paused') return 'Paused';
  if (v.phase === 'needs_you') return PHASE_LABEL.needs_you;
  if (v.activity === 'retry') return v.retryInS === null ? 'retrying' : `retry in ${v.retryInS}s`;
  if (v.activity === 'queued') return 'queued';
  return v.activity === 'parked' ? 'parked' : 'working';
}

export function ariaText(v: AgentView, now: Date | string = new Date()): string {
  return `Stage ${v.index + 1} of ${STAGES.length}: ${v.stage}, ${statusOf(v, now)}`;
}
