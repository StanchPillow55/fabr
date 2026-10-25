import { isoNow } from './answer';
import { submitEdit } from './edit';
import type { FabricSource } from './source';
import type { TaskRecord } from './types';

export function canMarkDone(record: Pick<TaskRecord, 'state'>): boolean {
  return record.state === 'reminder';
}

export function applyDone(record: TaskRecord, now: Date = new Date()): TaskRecord {
  if (!canMarkDone(record)) throw new Error(`only a reminder can be marked done, this one is ${record.state}`);
  return { ...record, state: 'succeeded', updated_at: isoNow(now) };
}

export const submitDone = (source: FabricSource, path: string, readText: string) => submitEdit(source, path, readText, (r) => applyDone(r));
