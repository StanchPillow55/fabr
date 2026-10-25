import type { Tree } from './api/types';
import type { DirEntry, TestResult } from './api/types';

export const needsOnboarding = (tree: Pick<Tree, 'sources'> | null): boolean => !!tree && tree.sources.length === 0;

export function slugify(s: string): string {
  const slug = s.trim().toLowerCase().replace(/[^a-z0-9_-]+/g, '-').replace(/^-+|-+$/g, '');
  return slug || 'source';
}

export function nameFromPath(root: string): string {
  const parts = root.trim().replace(/[\\/]+$/, '').split(/[\\/]/);
  return parts[parts.length - 1] || 'Home';
}

export const selectable = (dirs: DirEntry[]): string[] => dirs.filter((d) => !d.managed).map((d) => d.name);

export function toggleAll(dirs: DirEntry[], selected: ReadonlySet<string>): Set<string> {
  const all = selectable(dirs);
  return all.every((n) => selected.has(n)) ? new Set() : new Set(all);
}

export function toggleOne(selected: ReadonlySet<string>, name: string): Set<string> {
  const next = new Set(selected);
  if (next.has(name)) next.delete(name);
  else next.add(name);
  return next;
}

export function namesToInit(dirs: DirEntry[], selected: ReadonlySet<string>): string[] {
  return selectable(dirs).filter((n) => selected.has(n));
}

export function rootState(r: TestResult | null): 'managed' | 'plain' | null {
  if (!r || !r.ok || r.managed_root === undefined) return null;
  return r.managed_root ? 'managed' : 'plain';
}

export const shouldInitRoot = (r: TestResult | null, initRoot: boolean): boolean => rootState(r) === 'plain' && initRoot;
