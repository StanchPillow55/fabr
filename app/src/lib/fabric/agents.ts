import { parseYaml } from './yaml';

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
  return { ...(y as unknown as RegistryEntry), log_tail: log as string[], question: q as Record<string, unknown> | null };
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
