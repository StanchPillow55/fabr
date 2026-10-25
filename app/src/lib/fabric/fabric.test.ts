import { describe, expect, it } from 'vitest';
import { parseFrontMatter, parseLogLine, parseRecord } from './parse';
import { FixtureSource, fixtureName } from './source';
import { FIELDS } from './types';

const records = await new FixtureSource().listRecords();

describe('fixtures', () => {
  it('finds all ten fixtures', () => {
    expect(new Set(records.map((r) => fixtureName(r.path))).size).toBe(9);
    expect(records.length).toBe(10);
  });

  for (const { path, text } of records) {
    it(`parses ${path.split('fixtures/')[1]}`, () => {
      expect(Object.keys(parseFrontMatter(text).data)).toEqual([...FIELDS]);
      const r = parseRecord(text);
      expect(r.schema).toBe(2);
      expect(typeof r.state).toBe('string');
      expect(r.step === null || typeof r.step === 'string').toBe(true);
      expect(typeof r.waiting).toBe('boolean');
      for (const k of ['context', 'spec', 'success', 'plan', 'log'] as const) expect(typeof r.sections[k]).toBe('string');
      expect(r.sections.logEntries.length).toBe(r.sections.log ? r.sections.log.split('\n').length : 0);
    });
  }

  it('reads state, step and waiting as the fixtures say', () => {
    const by = (n: string) => parseRecord(records.find((r) => r.path.includes(n))!.text);
    expect(by('01-reminder')).toMatchObject({ state: 'reminder', step: null, waiting: false });
    expect(by('02-open-question')).toMatchObject({ state: 'task', step: 'define', waiting: true });
    expect(by('04-awaiting-approval')).toMatchObject({ state: 'defined', step: 'approve', waiting: true });
    expect(by('07-succeeded').verdict?.result).toBe('pass');
    expect(by('03-choice-question').question?.options).toEqual(['once', 'always', 'no']);
    expect(by('04-awaiting-approval').grants.requested).toEqual(['address.line_1', 'address.zip', 'name.first']);
  });
});

describe('malformed records', () => {
  const good = records[0].text;
  it('names a missing heading', () => {
    expect(() => parseRecord(good.replace('## Plan\n', ''))).toThrow('## Plan');
  });
  it('names a missing field', () => {
    expect(() => parseRecord(good.replace(/^waiting:.*\n/m, ''))).toThrow('"waiting"');
  });
  it('rejects an unknown state', () => {
    expect(() => parseRecord(good.replace('state: reminder', 'state: sleeping'))).toThrow('sleeping');
  });
});

describe('parseLogLine', () => {
  it('splits timestamp, step and message', () => {
    expect(parseLogLine('- 2026-10-24T18:03:00Z gather: delegate accepted, folder lock taken')).toEqual({
      ts: '2026-10-24T18:03:00Z', step: 'gather', message: 'delegate accepted, folder lock taken',
    });
  });
  it('returns null for other lines', () => {
    expect(parseLogLine('not a log line')).toBeNull();
  });
});
