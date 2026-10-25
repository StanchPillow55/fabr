import { parseRegistryEntry, type RegistryEntry } from './agents';
import type { RawRecord } from './source';

export function loadRegistry(raw: RawRecord[]): { entries: RegistryEntry[]; unreadable: number } {
  const byPath = new Map<string, RegistryEntry>();
  let unreadable = 0;
  for (const { text } of raw) {
    try {
      const e = parseRegistryEntry(text);
      const prev = byPath.get(e.task_path);
      if (!prev || Date.parse(e.heartbeat_at) > Date.parse(prev.heartbeat_at)) byPath.set(e.task_path, e);
    } catch {
      unreadable += 1;
    }
  }
  return { entries: [...byPath.values()], unreadable };
}

const base = (p: string) => (p.split('/').pop() ?? p).replace(/\.md$/, '');

// Exact path first; then the same task id, preferring a record that is in progress (mock paths differ from registry paths).
export function findByTaskPath<T extends { path: string; record: { id: string; state: string } }>(held: T[], taskPath: string): T | undefined {
  const exact = held.find((h) => h.path === taskPath);
  if (exact) return exact;
  const id = base(taskPath);
  const same = held.filter((h) => h.record.id === id);
  return same.find((h) => h.record.state === 'in_progress') ?? same[0];
}

export function entryMap<T extends { path: string; record: { id: string; state: string } }>(held: T[], entries: RegistryEntry[]): Map<string, RegistryEntry> {
  const byPath = new Map<string, T>();
  const byId = new Map<string, T[]>();
  for (const h of held) {
    byPath.set(h.path, h);
    byId.set(h.record.id, [...(byId.get(h.record.id) ?? []), h]);
  }
  const out = new Map<string, RegistryEntry>();
  for (const e of entries) {
    let target = byPath.get(e.task_path);
    if (!target) {
      const same = byId.get(base(e.task_path)) ?? [];
      target = same.find((h) => h.record.state === 'in_progress') ?? same[0];
    }
    if (!target) continue;
    const prev = out.get(target.path);
    if (!prev || Date.parse(e.heartbeat_at) > Date.parse(prev.heartbeat_at)) out.set(target.path, e);
  }
  return out;
}
