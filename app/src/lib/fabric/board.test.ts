import { describe, expect, it } from 'vitest';
import { COLUMNS, countsFor, folderName, folderOf, groupByFolder, groupByState, type Entry } from './board';
import { parseRecord } from './parse';
import { FixtureSource } from './source';

const entries: Entry[] = (await new FixtureSource().listRecords()).map(({ path, text }) => ({ path, record: parseRecord(text) }));
const grouped = groupByState(entries);
const find = (n: string) => entries.filter((e) => e.path.includes(n));
const col = (id: keyof typeof grouped) => grouped[id].map((e) => e.path);

describe('groupByState', () => {
  it('has the five columns in order', () => {
    expect(Object.keys(grouped)).toEqual(['reminder', 'task', 'defined', 'in_progress', 'done']);
    expect(COLUMNS.map((c) => c.id)).toEqual(Object.keys(grouped));
  });
  it('places the fixtures', () => {
    expect(grouped.reminder).toEqual(find('01-'));
    expect(col('task')).toEqual(expect.arrayContaining([...find('02-'), ...find('03-')].map((e) => e.path)));
    for (const e of [...find('02-'), ...find('03-')]) expect(e.record.waiting).toBe(true);
    expect(grouped.defined).toEqual(find('04-'));
    expect(grouped.defined[0].record.waiting).toBe(true);
    expect(grouped.in_progress).toEqual([...find('05-'), ...find('06-')]);
    expect(find('05-')[0].record.waiting).toBe(false);
    expect(find('06-')[0].record.waiting).toBe(true);
    expect(find('07-')[0].record.state).toBe('succeeded');
    expect(col('done')).toContain(find('07-')[0].path);
    expect(find('08-')[0].record.state).toBe('failed');
    expect(col('done')).toContain(find('08-')[0].path);
  });
  it('splits fixture 09 into a failed old record and a task new record', () => {
    const newer = find('order-filament-2')[0];
    const older = find('09-').find((e) => !e.path.includes('filament-2'))!;
    expect(older.record.state).toBe('failed');
    expect(col('done')).toContain(older.path);
    expect(newer.record.state).toBe('task');
    expect(col('task')).toContain(newer.path);
  });
  it('keeps every record exactly once', () => {
    expect(Object.values(grouped).flat().length).toBe(entries.length);
  });
});

describe('real paths', () => {
  const a = '/home/me/3d/.fabric-space/tasks/x.md';
  const b = '/home/me/3d/sub/.fabric-space/tasks/y.md';
  it('uses the folder that holds .fabric-space', () => {
    expect(folderOf(a)).toBe('/home/me/3d');
    expect(folderOf(b)).toBe('/home/me/3d/sub');
    expect(folderName(folderOf(b))).toBe('sub');
  });
  it('groups two folders separately', () => {
    const rec = entries[0].record;
    const g = groupByFolder([{ path: a, record: rec }, { path: b, record: rec }, { path: '/home/me/3d/.fabric-space/tasks/z.md', record: rec }]);
    expect([...g.keys()]).toEqual(['/home/me/3d', '/home/me/3d/sub']);
    expect(g.get('/home/me/3d')!.length).toBe(2);
  });
});

describe('groupByFolder and countsFor', () => {
  const folders = groupByFolder(entries);
  it('derives the folder from the path', () => {
    expect(folderOf(find('01-')[0].path)).toBe('01-reminder');
  });
  it('gives one row per fixture folder that holds records', () => {
    expect(folders.size).toBe(9);
    expect(folders.get('09-redelegated-pair')!.length).toBe(2);
  });
  it('counts sum to the records in each folder', () => {
    for (const list of folders.values()) {
      const c = countsFor(list);
      expect(Object.values(c.byColumn).reduce((a, b) => a + b, 0)).toBe(list.length);
      expect(c.total).toBe(list.length);
    }
    expect(countsFor(folders.get('09-redelegated-pair')!)).toMatchObject({ byColumn: { task: 1, done: 1 } });
    expect(countsFor(entries).waiting).toBe(entries.filter((e) => e.record.waiting).length);
  });
});
