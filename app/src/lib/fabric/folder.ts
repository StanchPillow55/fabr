import { fixtureName } from './source';
import type { State, TaskRecord } from './types';

export type Entry = { path: string; record: TaskRecord };

export const COLUMNS = [
  { id: 'reminder', label: 'Reminder', states: ['reminder'] },
  { id: 'task', label: 'Task', states: ['task'] },
  { id: 'defined', label: 'Defined', states: ['defined'] },
  { id: 'in_progress', label: 'In progress', states: ['in_progress'] },
  { id: 'done', label: 'Succeeded or failed', states: ['succeeded', 'failed'] },
] as const;
export type ColumnId = (typeof COLUMNS)[number]['id'];

export const STATE_LABEL: Record<State, string> = {
  reminder: 'reminder', task: 'task', defined: 'defined', in_progress: 'in progress', succeeded: 'succeeded', failed: 'failed',
};

export const columnOf = (state: State): ColumnId => COLUMNS.find((c) => (c.states as readonly string[]).includes(state))!.id;

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

export function groupByFolder(entries: Entry[]): Map<string, Entry[]> {
  const out = new Map<string, Entry[]>();
  for (const e of entries) {
    const k = folderOf(e.path);
    out.set(k, [...(out.get(k) ?? []), e]);
  }
  return out;
}

export type Counts = { byColumn: Record<ColumnId, number>; waiting: number; total: number };

export function countsFor(entries: Entry[]): Counts {
  const byColumn = Object.fromEntries(COLUMNS.map((c) => [c.id, 0])) as Record<ColumnId, number>;
  for (const e of entries) byColumn[columnOf(e.record.state)]++;
  return { byColumn, waiting: entries.filter((e) => e.record.waiting).length, total: entries.length };
}

export function stepLabel(r: TaskRecord): string | null {
  return r.step && (r.state === 'task' || r.state === 'defined' || r.state === 'in_progress') ? r.step : null;
}

// Absolute folder of a local node, or null for remote sources and the virtual root.
export function nodeFolder(source: { host: string | null; root: string } | undefined, path: string, home: string | null = null): string | null {
  if (!source || source.host) return null;
  let root = source.root;
  if (root === '~' || root.startsWith('~/')) {
    if (!home) return null;
    root = home + root.slice(1);
  }
  root = root.replace(/\/+$/, '');
  const rel = path.replace(/^\/+|\/+$/g, '');
  return rel ? `${root}/${rel}` : root || '/';
}
