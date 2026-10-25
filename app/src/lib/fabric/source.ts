import { isoNow } from './answer';
import { serializeRecord } from './serialize';
import type { TaskRecord } from './types';

export interface RawRecord { path: string; text: string }

export interface CreateFields { title: string; due?: string | null; context?: string; spec?: string; success?: string }

export interface FabricSource {
  listRecords(): Promise<RawRecord[]>;
  readRecord(path: string): Promise<string>;
  writeRecord(path: string, text: string, expectedUpdatedAt: string): Promise<void>;
  createRecord(folder: string, fields: CreateFields): Promise<{ path: string; id: string }>;
  listRegistry(): Promise<RawRecord[]>;
  readEvents(): Promise<string[]>;
  eventsKey(): string;
  readBackendHeartbeat(): Promise<string | null>;
  stamp(): Promise<string>;
}

export function updatedAtOf(text: string): string {
  const lines = text.split('\n');
  if (lines[0]?.trimEnd() !== '---') return '';
  for (const l of lines.slice(1)) {
    if (l.trimEnd() === '---') break;
    if (l.startsWith('updated_at:')) return l.slice('updated_at:'.length).trim().replace(/^["']|["']$/g, '');
  }
  return '';
}

export function taskSlug(title: string): string {
  const slug = title.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+/, '').slice(0, 40).replace(/-+$/, '');
  return slug || 'task';
}

export function newRecord(id: string, f: CreateFields, now: Date = new Date()): TaskRecord {
  const at = isoNow(now);
  return {
    schema: 1, id, title: f.title.trim(), due: f.due || null, state: 'reminder', step: null, waiting: false,
    created_at: at, updated_at: at, requests: { delegate: false, cancel: false, redelegate: false },
    question: null, answers: [], grants: { requested: [], granted: [] }, plan_meta: null, assertions: [],
    approval: null, verdict: null, cost_usd: 0, links: { redelegated_from: null, redelegated_to: null },
    sections: { context: (f.context ?? '').trim(), spec: (f.spec ?? '').trim(), success: (f.success ?? '').trim(), plan: '', log: '', logEntries: [] },
  };
}

const files = import.meta.glob('../../../../fixtures/**/project/.fabric-space/tasks/*.md', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;
const registry = import.meta.glob('../../../../fixtures/**/home/.fabric-space/registry/*.yaml', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;
const events = import.meta.glob('../../../../fixtures/**/home/.fabric-space/events.log', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;

export class FixtureSource implements FabricSource {
  private copy: Record<string, string> = { ...files };
  private writes = 0;

  async listRecords(): Promise<RawRecord[]> {
    return Object.keys(this.copy).sort().map((path) => ({ path, text: this.copy[path] }));
  }
  async readRecord(path: string): Promise<string> {
    if (!(path in this.copy)) throw new Error(`no record at ${path}`);
    return this.copy[path];
  }
  async writeRecord(path: string, text: string, expectedUpdatedAt: string): Promise<void> {
    const current = updatedAtOf(await this.readRecord(path));
    if (current !== expectedUpdatedAt) {
      throw new Error(`${path} changed: updated_at is "${current}", expected "${expectedUpdatedAt}"`);
    }
    this.copy[path] = text;
    this.writes += 1;
  }
  async createRecord(folder: string, fields: CreateFields): Promise<{ path: string; id: string }> {
    if (!fields.title.trim()) throw new Error('a task needs a title');
    const dir = folder.includes('/') ? `${folder.replace(/\/+$/, '')}/.fabric-space/tasks` : `/fixtures/${folder}/project/.fabric-space/tasks`;
    const base = `${isoNow().slice(0, 10)}-${taskSlug(fields.title)}`;
    let id = base;
    for (let n = 2; `${dir}/${id}.md` in this.copy; n++) id = `${base}-${n}`;
    const path = `${dir}/${id}.md`;
    this.copy[path] = serializeRecord(newRecord(id, fields));
    this.writes += 1;
    return { path, id };
  }
  async listRegistry(): Promise<RawRecord[]> {
    return Object.keys(registry).sort().map((path) => ({ path, text: registry[path] }));
  }
  async readEvents(): Promise<string[]> {
    return Object.keys(events).sort().flatMap((p) => events[p].split('\n').filter((l) => l.trim() !== ''));
  }
  eventsKey(): string {
    return 'events';
  }
  async readBackendHeartbeat(): Promise<string | null> {
    return null;
  }
  async stamp(): Promise<string> {
    return String(this.writes);
  }
}

export function fixtureName(path: string): string {
  return /fixtures\/([^/]+)\//.exec(path)?.[1] ?? path;
}
