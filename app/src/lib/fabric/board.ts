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

export function groupByState(entries: Entry[]): Record<ColumnId, Entry[]> {
  const out = Object.fromEntries(COLUMNS.map((c) => [c.id, [] as Entry[]])) as Record<ColumnId, Entry[]>;
  for (const e of entries) out[columnOf(e.record.state)].push(e);
  return out;
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
