import { fixtureName } from './source';
import type { TaskRecord } from './types';

export type Entry = { path: string; record: TaskRecord };

const TASKS_DIR = '/.fabric-space/tasks/';

// The folder that holds .fabric-space; fixture paths keep their per-fixture name.
export function folderOf(path: string): string {
  if (path.includes('/fixtures/')) return fixtureName(path);
  const i = path.indexOf(TASKS_DIR);
  return i > 0 ? path.slice(0, i) : path;
}

export function folderName(folder: string): string {
  return folder.replace(/\/+$/, '').split('/').pop() || folder;
}

export function stepLabel(r: TaskRecord): string | null {
  return r.step && (r.state === 'task' || r.state === 'defined' || r.state === 'in_progress') ? r.step : null;
}

export function nodeFolder(source: { host: string | null; root: string } | undefined, path: string, home: string | null = null): string | null {
  if (!source || source.host) return null;
  let root = source.root.trim().replace(/\/+$/, '');
  if (root === '~' || root.startsWith('~/')) {
    const h = home?.replace(/\/+$/, '');
    if (!h) return null;
    root = h + root.slice(1);
  }
  const rel = path.replace(/^\/+|\/+$/g, '');
  return rel ? `${root}/${rel}` : root || '/';
}
