import { describe, expect, it } from 'vitest';
import { applyAnswer, type AnswerInput } from './answer';
import { parseRecord } from './parse';
import { serializeRecord } from './serialize';
import { FixtureSource } from './source';

const records = await new FixtureSource().listRecords();
const load = (n: string) => records.find((r) => r.path.includes(n))!.text;
const open = parseRecord(load('02-open'));
const choice = parseRecord(load('03-choice'));
const grant = parseRecord(load('06-waiting'));
const now = new Date('2026-10-24T19:00:00.500Z');

function diff(a: unknown, b: unknown, path = ''): string[] {
  if (a !== null && b !== null && typeof a === 'object' && typeof b === 'object' && !Array.isArray(a)) {
    const keys = new Set([...Object.keys(a as object), ...Object.keys(b as object)]);
    return [...keys].flatMap((k) => diff((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k], path ? `${path}.${k}` : k));
  }
  return path === 'schema' || JSON.stringify(a) === JSON.stringify(b) ? [] : [path];
}

describe('applyAnswer', () => {
  it('fills an open answer and stamps updated_at', () => {
    const r = applyAnswer(open, { value: ' Prusament ' }, 'gui', now);
    expect(r.question!.answer).toEqual({ value: 'Prusament', replacement: null, granted: null, by: 'gui', at: '2026-10-24T19:00:00Z' });
    expect(r.updated_at).toBe('2026-10-24T19:00:00Z');
    expect(open.question!.answer).toBeNull();
  });

  it('only question.answer fields and updated_at differ after a serialize round trip', () => {
    for (const [rec, input] of ([
      [open, { value: 'x' }],
      [choice, { value: 'no', replacement: 'ship to work' }],
      [grant, { granted: [{ field: 'address.line_2', scope: 'project' }] }],
    ] as [typeof open, AnswerInput][])) {
      const after = parseRecord(serializeRecord(applyAnswer(rec, input, 'gui', now)));
      const changed = diff(rec, after).sort();
      expect(changed.every((p) => p === 'updated_at' || p.startsWith('question.answer'))).toBe(true);
      expect(changed).toContain('updated_at');
      expect(changed.some((p) => p.startsWith('question.answer'))).toBe(true);
    }
  });

  it('refuses a second answer and a missing question', () => {
    const done = applyAnswer(open, { value: 'x' }, 'gui', now);
    expect(() => applyAnswer(done, { value: 'y' }, 'gui', now)).toThrow(/already answered/);
    expect(() => applyAnswer(parseRecord(load('01-reminder')), { value: 'y' }, 'gui')).toThrow(/no pending question/);
  });

  it('validates open', () => {
    expect(() => applyAnswer(open, { value: '  ' }, 'gui')).toThrow();
    expect(() => applyAnswer(open, {}, 'gui')).toThrow();
  });

  it('validates choice', () => {
    expect(applyAnswer(choice, { value: 'always' }, 'gui', now).question!.answer!.value).toBe('always');
    expect(() => applyAnswer(choice, { value: 'maybe' }, 'gui')).toThrow();
    expect(() => applyAnswer(choice, { value: 'no' }, 'gui')).toThrow(/replacement/);
    expect(() => applyAnswer(choice, { value: 'no', replacement: ' ' }, 'gui')).toThrow(/replacement/);
    const r = applyAnswer(choice, { value: 'no', replacement: 'office', granted: [{ field: 'a', scope: 'task' }] }, 'gui', now);
    expect(r.question!.answer).toMatchObject({ value: 'no', replacement: 'office', granted: null });
    expect(applyAnswer(choice, { value: 'once', replacement: 'junk' }, 'gui', now).question!.answer!.replacement).toBeNull();
  });

  it('validates grant', () => {
    const ok = applyAnswer(grant, { granted: [{ field: 'address.line_2', scope: 'always' }] }, 'gui', now);
    expect(ok.question!.answer).toMatchObject({ value: null, granted: [{ field: 'address.line_2', scope: 'always' }] });
    expect(() => applyAnswer(grant, { granted: [] }, 'gui')).toThrow();
    expect(() => applyAnswer(grant, { granted: [{ field: 'name.last', scope: 'task' }] }, 'gui')).toThrow(/not requested/);
    expect(() => applyAnswer(grant, { granted: [{ field: 'address.line_2', scope: 'forever' as never }] }, 'gui')).toThrow(/scope/);
  });
});
