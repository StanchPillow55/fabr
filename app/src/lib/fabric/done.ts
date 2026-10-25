import { isoNow } from './answer';
import { parseRecord } from './parse';
import { serializeRecord } from './serialize';
import { updatedAtOf, type FabricSource } from './source';
import type { TaskRecord } from './types';
import { ConflictError } from './write';

export function canMarkDone(record: Pick<TaskRecord, 'state'>): boolean {
  return record.state === 'reminder';
}

export function applyDone(record: TaskRecord, now: Date = new Date()): TaskRecord {
  if (!canMarkDone(record)) throw new Error(`only a reminder can be marked done, this one is ${record.state}`);
  return { ...record, state: 'succeeded', updated_at: isoNow(now) };
}

export async function submitDone(source: FabricSource, path: string, readText: string): Promise<{ text: string; record: TaskRecord }> {
  const applied = applyDone(parseRecord(readText));
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
