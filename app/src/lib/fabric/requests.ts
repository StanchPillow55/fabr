import { isoNow } from './answer';
import { submitEdit } from './edit';
import type { FabricSource } from './source';
import type { TaskRecord } from './types';

export type RequestKey = 'delegate' | 'cancel' | 'redelegate' | 'pause' | 'resume';

const DRIVEN: TaskRecord['state'][] = ['task', 'defined', 'in_progress'];

export const REQUEST_STATES: Record<RequestKey, TaskRecord['state'][]> = {
  delegate: ['reminder'],
  cancel: ['in_progress'],
  redelegate: ['failed'],
  pause: DRIVEN,
  resume: DRIVEN,
};

export const STATE_REQUESTS: RequestKey[] = ['delegate', 'cancel', 'redelegate'];
export const AGENT_REQUESTS: RequestKey[] = ['pause', 'resume'];

export function canPauseResume(record: TaskRecord): 'pause' | 'resume' | null {
  if (!DRIVEN.includes(record.state)) return null;
  if (record.requests.pause || record.requests.resume) return null;
  return record.paused ? 'resume' : 'pause';
}

export const BACKEND_SILENT_SECONDS = 60;

export function requestFor(state: TaskRecord['state']): RequestKey | null {
  return STATE_REQUESTS.find((k) => REQUEST_STATES[k].includes(state)) ?? null;
}

export function applyRequest(record: TaskRecord, key: RequestKey, now: Date = new Date()): TaskRecord {
  if (!(key in REQUEST_STATES)) throw new Error(`unknown request "${String(key)}"`);
  if (!REQUEST_STATES[key].includes(record.state)) throw new Error(`${key} does not apply to a record in state ${record.state}`);
  if (record.requests[key]) throw new Error(`${key} is already requested`);
  if (key === 'pause' && (record.paused || record.requests.resume)) throw new Error('pause does not apply to a paused or resuming task');
  if (key === 'resume' && (!record.paused || record.requests.pause)) throw new Error('resume applies only to a paused task');
  return { ...record, updated_at: isoNow(now), requests: { ...record.requests, [key]: true } };
}

export function pendingRequests(record: TaskRecord): RequestKey[] {
  return (Object.keys(REQUEST_STATES) as RequestKey[]).filter((k) => record.requests[k]);
}

export function backendSilent(record: TaskRecord, now: Date | string): boolean {
  if (pendingRequests(record).length === 0) return false;
  const t = Date.parse(record.updated_at);
  const n = now instanceof Date ? now.getTime() : Date.parse(now);
  return Number.isFinite(t) && Number.isFinite(n) && n - t > BACKEND_SILENT_SECONDS * 1000;
}

export const submitRequest = (source: FabricSource, path: string, readText: string, key: RequestKey) =>
  submitEdit(source, path, readText, (r) => applyRequest(r, key));
