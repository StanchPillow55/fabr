import { describe, expect, it } from 'vitest';
import { parseFrontMatter, parseRecord } from './parse';
import { serializeRecord } from './serialize';
import { FixtureSource } from './source';
import { FIELDS } from './types';

const records = await new FixtureSource().listRecords();

describe('serializeRecord', () => {
  for (const { path, text } of records) {
    const name = path.split('fixtures/')[1];
    it(`round-trips ${name}`, () => {
      const out = serializeRecord(parseRecord(text));
      expect(parseRecord(out)).toEqual({ ...parseRecord(text), schema: 3 });
      expect(Object.keys(parseFrontMatter(out).data)).toEqual([...FIELDS]);
    });
  }

  it('quotes strings that would read back as something else', () => {
    const r = parseRecord(records[0].text);
    const tricky = ['no', 'true', '123', '', 'a: b', 'x # y', ' lead', 'say "hi"', 'back\\slash', 'two\nlines', '[a]', '- dash'];
    for (const t of tricky) {
      const out = parseRecord(serializeRecord({ ...r, title: t }));
      expect(out.title).toBe(t);
    }
  });

  it('keeps no quoted and timestamps bare', () => {
    const c = records.find((x) => x.path.includes('03-choice'))!;
    const out = serializeRecord(parseRecord(c.text));
    expect(out).toContain('options: [once, always, "no"]');
    expect(out).toMatch(/^updated_at: 2026-10-24T18:05:30Z$/m);
    expect(out).toMatch(/^requests: \{delegate: false, cancel: false, redelegate: false\}$/m);
  });

  it('reads schema 2 as pri 0 and writes schema 3 with pri', () => {
    const old = parseRecord(records.find((x) => x.path.includes('01-reminder'))!.text);
    expect([old.schema, old.pri]).toEqual([2, 0]);
    const out = serializeRecord(old);
    expect(out).toMatch(/^schema: 3$/m);
    expect(out).toMatch(/^pri: 0$/m);
    expect(out.indexOf('\ndue:')).toBeLessThan(out.indexOf('\npri:'));
    expect(parseRecord(out)).toMatchObject({ schema: 3, pri: 0 });
  });

  it('keeps pri and a timed due through a round trip', () => {
    const r = parseRecord(records[0].text);
    const out = parseRecord(serializeRecord({ ...r, pri: 2, due: '2026-11-01T09:30' }));
    expect([out.pri, out.due]).toEqual([2, '2026-11-01T09:30']);
  });

  it('refuses an unknown schema and a bad pri', () => {
    const text = records[0].text;
    expect(() => parseRecord(text.replace(/^schema: 2$/m, 'schema: 4'))).toThrow(/schema/);
    expect(() => parseRecord(serializeRecord(parseRecord(text)).replace(/^pri: 0$/m, 'pri: 9'))).toThrow(/pri/);
  });
});
