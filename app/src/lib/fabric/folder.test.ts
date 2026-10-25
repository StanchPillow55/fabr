import { describe, expect, it } from 'vitest';
import { nodeFolder, folderName, folderOf, type Entry } from './folder';
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
});

describe('fixture folders', () => {
  it('derives the folder from the path', () => {
    expect(folderOf(find('01-')[0].path)).toBe('01-reminder');
  });
});

describe('nodeFolder for a hosted source', () => {
  const at = (root: string, path: string, home: string | null = '/home/me') => nodeFolder({ host: 'vm', root }, path, home);
  it('keeps the root as configured, ~ unexpanded, and trims slashes', () => {
    expect(at('/x', 'a')).toBe('/x/a');
    expect(at('~/sample/', '/a/b/')).toBe('~/sample/a/b');
    expect(at('~', '')).toBe('~');
    expect(at('~/sample', '', null)).toBe('~/sample');
    expect(at('/srv/p//', 'a')).toBe('/srv/p/a');
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
  it('is null for ~ without a home and for a missing source', () => {
    expect(at('~', 'a', null)).toBeNull();
    expect(at('~/x', '', null)).toBeNull();
    expect(nodeFolder(undefined, 'a')).toBeNull();
  });
});
