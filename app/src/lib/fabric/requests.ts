import { isoNow } from './answer';
import { submitEdit } from './edit';
import { isActive, isOpen, phaseOf, type Phase } from './phase';
import { minuteOf } from '../model';
import type { FabricSource } from './source';
import type { TaskRecord } from './types';

export type RequestKey = 'delegate' | 'cancel' | 'redelegate' | 'pause' | 'resume' | 'complete';

export const REQUEST_STATES: Record<'delegate' | 'cancel' | 'redelegate', TaskRecord['state'][]> = {
  delegate: ['reminder'],
  cancel: ['in_progress'],
  redelegate: ['failed'],
};

export const REQUEST_KEYS: RequestKey[] = ['delegate', 'cancel', 'redelegate', 'pause', 'resume', 'complete'];

export function canPauseResume(record: TaskRecord, phase: Phase): 'pause' | 'resume' | null {
  if (!isActive(phase) || record.requests.pause || record.requests.resume) return null;
  return phase === 'paused' ? 'resume' : 'pause';
}

export const COMPLETE_PENDING = 'completion requested';

export function canComplete(record: TaskRecord, phase: Phase): boolean {
  return isOpen(phase) && phase !== 'todo' && !record.requests.complete;
}

export const BACKEND_SILENT_SECONDS = 60;

export function requestFor(state: TaskRecord['state']): 'delegate' | 'cancel' | 'redelegate' | null {
  return (Object.keys(REQUEST_STATES) as (keyof typeof REQUEST_STATES)[]).find((k) => REQUEST_STATES[k].includes(state)) ?? null;
}

export function applyRequest(record: TaskRecord, key: RequestKey, now: Date = new Date()): TaskRecord {
  if (!REQUEST_KEYS.includes(key)) throw new Error(`unknown request "${String(key)}"`);
  const phase = phaseOf(record, minuteOf(now));
  if (key === 'pause' || key === 'resume') {
    if (!isActive(phase)) throw new Error(`${key} does not apply to a ${phase} task`);
    if (record.requests[key]) throw new Error(`${key} is already requested`);
    if (key === 'pause' && (phase === 'paused' || record.requests.resume)) throw new Error('pause does not apply to a paused or resuming task');
    if (key === 'resume' && (phase !== 'paused' || record.requests.pause)) throw new Error('resume applies only to a paused task');
  } else if (key === 'complete') {
    if (!isOpen(phase)) throw new Error(`complete does not apply to a ${phase} task`);
    if (record.requests.complete) throw new Error('complete is already requested');
  } else {
    if (!REQUEST_STATES[key].includes(record.state)) throw new Error(`${key} does not apply to a record in state ${record.state}`);
    if (record.requests[key]) throw new Error(`${key} is already requested`);
  }
  return { ...record, updated_at: isoNow(now), requests: { ...record.requests, [key]: true } };
}

export function pendingRequests(record: TaskRecord): RequestKey[] {
  return REQUEST_KEYS.filter((k) => record.requests[k]);
}

export function backendSilent(record: TaskRecord, now: Date | string): boolean {
  if (pendingRequests(record).length === 0) return false;
  const t = Date.parse(record.updated_at);
  const n = now instanceof Date ? now.getTime() : Date.parse(now);
  return Number.isFinite(t) && Number.isFinite(n) && n - t > BACKEND_SILENT_SECONDS * 1000;
}

export const submitRequest = (source: FabricSource, path: string, readText: string, key: RequestKey) =>
  submitEdit(source, path, readText, (r) => applyRequest(r, key));
