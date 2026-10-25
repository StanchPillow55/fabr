import { isoNow } from './answer';
import { submitEdit } from './edit';
import { canShelve, phaseOf } from './phase';
import { minuteOf } from '../model';
import type { FabricSource } from './source';
import type { TaskRecord } from './types';

const UNTIL = /^\d{4}-\d{2}-\d{2}(T([01]\d|2[0-3]):[0-5]\d)?$/;

export function applySuspend(record: TaskRecord, until: string | null, now: Date = new Date()): TaskRecord {
  if (!canShelve(phaseOf(record, minuteOf(now)))) throw new Error(`a ${record.state} task cannot be suspended`);
  if (until !== null && !UNTIL.test(until)) throw new Error(`bad until time: ${until}`);
  return { ...record, suspended: true, suspend_until: until, updated_at: isoNow(now) };
}

export function applyUnsuspend(record: TaskRecord, now: Date = new Date()): TaskRecord {
  return { ...record, suspended: false, suspend_until: null, updated_at: isoNow(now) };
}

export const submitSuspend = (source: FabricSource, path: string, readText: string, until: string | null) =>
  submitEdit(source, path, readText, (r) => applySuspend(r, until));

export const submitUnsuspend = (source: FabricSource, path: string, readText: string) =>
  submitEdit(source, path, readText, (r) => applyUnsuspend(r));
