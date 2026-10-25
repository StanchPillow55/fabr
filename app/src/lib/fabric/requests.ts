import { isoNow } from './answer';
import { parseRecord } from './parse';
import { serializeRecord } from './serialize';
import { updatedAtOf, type FabricSource } from './source';
import type { TaskRecord } from './types';
import { ConflictError } from './write';

export type RequestKey = 'delegate' | 'cancel' | 'redelegate';

export const REQUEST_STATE: Record<RequestKey, TaskRecord['state']> = {
  delegate: 'reminder',
  cancel: 'in_progress',
  redelegate: 'failed',
};

export const BACKEND_SILENT_SECONDS = 60;

export function requestFor(state: TaskRecord['state']): RequestKey | null {
  return (Object.keys(REQUEST_STATE) as RequestKey[]).find((k) => REQUEST_STATE[k] === state) ?? null;
}

export function applyRequest(record: TaskRecord, key: RequestKey, now: Date = new Date()): TaskRecord {
  if (!(key in REQUEST_STATE)) throw new Error(`unknown request "${String(key)}"`);
  if (record.state !== REQUEST_STATE[key]) throw new Error(`${key} does not apply to a record in state ${record.state}`);
  if (record.requests[key]) throw new Error(`${key} is already requested`);
  return { ...record, updated_at: isoNow(now), requests: { ...record.requests, [key]: true } };
}

export function pendingRequests(record: TaskRecord): RequestKey[] {
  return (Object.keys(REQUEST_STATE) as RequestKey[]).filter((k) => record.requests[k]);
}

export function backendSilent(record: TaskRecord, now: Date | string): boolean {
  if (pendingRequests(record).length === 0) return false;
  const t = Date.parse(record.updated_at);
  const n = now instanceof Date ? now.getTime() : Date.parse(now);
  return Number.isFinite(t) && Number.isFinite(n) && n - t > BACKEND_SILENT_SECONDS * 1000;
}

export async function submitRequest(source: FabricSource, path: string, readText: string, key: RequestKey): Promise<{ text: string; record: TaskRecord }> {
  const applied = applyRequest(parseRecord(readText), key);
  const expected = updatedAtOf(readText);
  try {
    await source.writeRecord(path, serializeRecord(applied), expected);
  } catch (e) {
    const disk = await source.readRecord(path).catch(() => null);
    if (disk !== null && updatedAtOf(disk) !== expected) throw new ConflictError(path);
    throw e;
  }
  const text = await source.readRecord(path);
  return { text, record: parseRecord(text) };
}
