import { describe, expect, it } from 'vitest';
import { parseRecord } from './parse';
import { FixtureSource } from './source';
import { ConflictError, submitAnswer } from './write';

async function setup() {
  const src = new FixtureSource();
  const { path, text } = (await src.listRecords()).find((r) => r.path.includes('02-open'))!;
  return { src, path, text };
}

describe('submitAnswer', () => {
  it('writes through the source and returns what is on disk', async () => {
    const { src, path, text } = await setup();
    const out = await submitAnswer(src, path, text, { value: 'Prusament' });
    expect(out.record.question!.answer!.value).toBe('Prusament');
    expect(parseRecord(await src.readRecord(path)).question!.answer!.by).toBe('gui');
  });

  it('refuses a second answer, from the fresh text and from the stale text', async () => {
    const { src, path, text } = await setup();
    const first = await submitAnswer(src, path, text, { value: 'one' });
    await expect(submitAnswer(src, path, first.text, { value: 'two' })).rejects.toThrow(/already answered/);
    await expect(submitAnswer(src, path, text, { value: 'two' })).rejects.toBeInstanceOf(ConflictError);
    expect(parseRecord(await src.readRecord(path)).question!.answer!.value).toBe('one');
  });
});
