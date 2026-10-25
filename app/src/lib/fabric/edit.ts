import { parseRecord } from './parse';
import { serializeRecord } from './serialize';
import { updatedAtOf, type FabricSource } from './source';
import type { TaskRecord } from './types';
import { ConflictError } from './write';

export async function submitEdit(source: FabricSource, path: string, readText: string, apply: (r: TaskRecord) => TaskRecord): Promise<{ text: string; record: TaskRecord }> {
  const applied = apply(parseRecord(readText));
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
