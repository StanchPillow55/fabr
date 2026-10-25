import { describe, expect, it } from 'vitest';
import { countsFor, nodeFolder, folderName, folderOf, groupByFolder, type Entry } from './folder';
import { parseRecord } from './parse';
import { FixtureSource } from './source';

const entries: Entry[] = (await new FixtureSource().listRecords()).map(({ path, text }) => ({ path, record: parseRecord(text) }));
const find = (n: string) => entries.filter((e) => e.path.includes(n));

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

describe('nodeFolder', () => {
  const at = (root: string, path: string, home: string | null) => nodeFolder({ host: null, root }, path, home);
  it('expands ~ and ~/x with the home and trims slashes', () => {
    expect(at('~', 'a', '/home/me')).toBe('/home/me/a');
    expect(at('~/x', '/a/', '/home/me/')).toBe('/home/me/x/a');
    expect(at('~/x/', '', '/home/me')).toBe('/home/me/x');
    expect(at('/srv/p//', 'a', null)).toBe('/srv/p/a');
    expect(at('/', '', null)).toBe('/');
  });
  it('is null for ~ without a home and for remote or missing sources', () => {
    expect(at('~', 'a', null)).toBeNull();
    expect(at('~/x', '', null)).toBeNull();
    expect(nodeFolder({ host: 'h', root: '/x' }, 'a', '/home/me')).toBeNull();
    expect(nodeFolder(undefined, 'a')).toBeNull();
  });
});
