import { parseYaml } from './yaml';

export type Activity = 'model' | 'tool' | 'check' | 'parked' | 'retry' | 'queued' | 'paused';
export type Waiting = 'question' | 'approval';

export interface StageUsage { calls: number; input_tokens: number; output_tokens: number; usd: number }

export interface RegistryEntry {
  task_path: string;
  task_id: string;
  folder: string;
  step: string;
  started_at: string;
  heartbeat_at: string;
  heartbeat_interval_s: number;
  cost_usd: number;
  log_tail: string[];
  question: Record<string, unknown> | null;
  pid: number;
  state?: string;
  step_at?: string;
  waiting?: Waiting | null;
  activity?: Activity;
  retry_in_s?: number;
  last_error?: string;
  usage?: Record<string, StageUsage>;
  source?: string | null;
}

const ACTIVITIES: readonly string[] = ['model', 'tool', 'check', 'parked', 'retry', 'queued', 'paused'];
const num = (v: unknown): number => (typeof v === 'number' && Number.isFinite(v) ? v : 0);

function parseUsage(v: unknown): Record<string, StageUsage> | undefined {
  if (!v || typeof v !== 'object' || Array.isArray(v)) return undefined;
  const out: Record<string, StageUsage> = {};
  for (const [k, raw] of Object.entries(v as Record<string, unknown>)) {
    if (!raw || typeof raw !== 'object' || Array.isArray(raw)) continue;
    const u = raw as Record<string, unknown>;
    out[k] = { calls: num(u.calls), input_tokens: num(u.input_tokens ?? u.in), output_tokens: num(u.output_tokens ?? u.out), usd: num(u.usd) };
  }
  return out;
}

const STRINGS = ['task_path', 'task_id', 'folder', 'step'] as const;
const TIMES = ['started_at', 'heartbeat_at'] as const;
const NUMBERS = ['heartbeat_interval_s', 'cost_usd', 'pid'] as const;

export function parseRegistryEntry(text: string): RegistryEntry {
  const y = parseYaml(text);
  for (const k of STRINGS) if (typeof y[k] !== 'string' || y[k] === '') throw new Error(`registry entry: ${k} must be a non-empty string`);
  for (const k of TIMES) if (typeof y[k] !== 'string' || Number.isNaN(Date.parse(y[k] as string))) throw new Error(`registry entry: ${k} must be an ISO timestamp`);
  for (const k of NUMBERS) if (typeof y[k] !== 'number') throw new Error(`registry entry: ${k} must be a number`);
  if ((y.heartbeat_interval_s as number) <= 0) throw new Error('registry entry: heartbeat_interval_s must be positive');
  const log = y.log_tail ?? [];
  if (!Array.isArray(log) || log.some((l) => typeof l !== 'string')) throw new Error('registry entry: log_tail must be a list of strings');
  const q = y.question ?? null;
  if (q !== null && (typeof q !== 'object' || Array.isArray(q))) throw new Error('registry entry: question must be a map or null');
  const base = { ...(y as unknown as RegistryEntry), log_tail: log as string[], question: q as Record<string, unknown> | null };
  const str = (v: unknown) => (typeof v === 'string' && v !== '' ? v : undefined);
  const stepAt = str(y.step_at);
  base.state = str(y.state);
  base.step_at = stepAt && !Number.isNaN(Date.parse(stepAt)) ? stepAt : undefined;
  base.waiting = y.waiting === 'question' || y.waiting === 'approval' ? y.waiting : y.waiting === null ? null : undefined;
  base.activity = typeof y.activity === 'string' && ACTIVITIES.includes(y.activity) ? (y.activity as Activity) : undefined;
  base.retry_in_s = typeof y.retry_in_s === 'number' ? y.retry_in_s : undefined;
  base.last_error = str(y.last_error);
  base.usage = parseUsage(y.usage);
  return base;
}

const ms = (t: Date | string) => (t instanceof Date ? t.getTime() : Date.parse(t));

export function ageSeconds(entry: RegistryEntry, now: Date | string): number {
  return Math.max(0, (ms(now) - Date.parse(entry.heartbeat_at)) / 1000);
}

export function elapsedSeconds(entry: RegistryEntry, now: Date | string): number {
  return Math.max(0, (ms(now) - Date.parse(entry.started_at)) / 1000);
}

export function isDead(entry: RegistryEntry, now: Date | string): boolean {
  return ageSeconds(entry, now) > 3 * entry.heartbeat_interval_s;
}

export function sortEntries(entries: RegistryEntry[]): RegistryEntry[] {
  return [...entries].sort((a, b) => Date.parse(b.started_at) - Date.parse(a.started_at));
}

export function formatDuration(s: number): string {
  const t = Math.floor(s);
  if (t < 60) return `${t}s`;
  if (t < 3600) return `${Math.floor(t / 60)}m ${t % 60}s`;
  return `${Math.floor(t / 3600)}h ${Math.floor((t % 3600) / 60)}m`;
}
