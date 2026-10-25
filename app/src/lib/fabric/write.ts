import { applyAnswer, type AnswerInput } from './answer';
import { parseRecord } from './parse';
import { serializeRecord } from './serialize';
import { updatedAtOf, type FabricSource } from './source';
import type { TaskRecord } from './types';

export class ConflictError extends Error {
  constructor(path: string) {
    super(`${path} changed on disk since it was read`);
    this.name = 'ConflictError';
  }
}

export async function submitAnswer(source: FabricSource, path: string, readText: string, input: AnswerInput, by: 'gui' | 'telegram' = 'gui'): Promise<{ text: string; record: TaskRecord }> {
  const applied = applyAnswer(parseRecord(readText), input, by);
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
