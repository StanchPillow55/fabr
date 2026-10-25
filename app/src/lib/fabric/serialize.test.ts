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
      expect(parseRecord(out)).toEqual(parseRecord(text));
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
});
