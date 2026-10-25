import { isTauri } from '../api/env';
import { createApi } from '../api/index';
import type { Source } from '../api/types';
import type { AnswerInput } from './answer';
import { submitApproval, type ApprovalDecision } from './approval';
import { BACKEND_LABEL, backendState } from './backend';
import { loadReadPosition, parseEvents, reconcileReadPosition, saveReadPosition, unreadCount, type FabricEvent } from './events';
import { COLUMNS, columnOf, folderName, folderOf, nodeFolder, type ColumnId, type Entry } from './folder';
import { createFabricSource } from './live';
import { parseRecord } from './parse';
import { submitRequest, type RequestKey } from './requests';
import type { CreateFields, FabricSource } from './source';
import { findByTaskPath, loadRegistry } from './view';
import { submitAnswer } from './write';
import type { RegistryEntry } from './agents';

export type Held = Entry & { text: string };
export type FolderCounts = { byColumn: Record<ColumnId, number>; waiting: number; total: number };

const EMPTY: Held[] = [];

export function localRoots(sources: Source[] | undefined): string[] {
  return (sources ?? []).filter((s) => !s.host).map((s) => s.root);
}

export class FabricStore {
  records = $state<Held[]>([]);
  registry = $state<RegistryEntry[]>([]);
  events = $state<FabricEvent[]>([]);
  heartbeat = $state<string | null>(null);
  readPosition = $state(0);
  now = $state(new Date());
  loaded = $state(false);
  errors = $state<string[]>([]);
  actionError = $state<string | null>(null);
  openPath = $state<string | null>(null);
  home = $state<string | null>(null);

  #source: FabricSource | null = null;
  #timer: ReturnType<typeof setInterval> | null = null;
  #last = '';
  #dead = false;
  #eventsKey = 'events';

  constructor(source: FabricSource | null = null) {
    this.#source = source;
  }

  backend = $derived(backendState(this.heartbeat, this.now));
  backendLabel = $derived(BACKEND_LABEL[this.backend]);
  unread = $derived(unreadCount(this.events, this.readPosition));
  byFolder = $derived.by(() => {
    const m = new Map<string, Held[]>();
    for (const h of this.records) {
      const k = folderOf(h.path);
      m.set(k, [...(m.get(k) ?? []), h]);
    }
    return m;
  });
  #running = $derived.by(() => {
    const m = new Map<string, RegistryEntry[]>();
    for (const e of this.registry) {
      const held = findByTaskPath(this.records, e.task_path);
      const k = held ? folderOf(held.path) : folderOf(e.task_path);
      m.set(k, [...(m.get(k) ?? []), e]);
    }
    return m;
  });
  openRecord = $derived(this.records.find((h) => h.path === this.openPath) ?? null);

  async start(treeRoots: string[] = []): Promise<void> {
    if (this.#timer) return;
    this.#dead = false;
    this.#timer = setInterval(() => void this.tick(), 1000);
    try {
      if (!this.#source) {
        const api = await createApi();
        const settings = await api.fabricSettingsRead();
        this.#source = await createFabricSource(settings, treeRoots);
        if (isTauri()) this.home = (await (await import('@tauri-apps/api/path')).homeDir()).replace(/\/+$/, '');
      }
    } catch (e) {
      this.errors = [(e as Error).message];
      return;
    }
    if (this.#dead) return;
    this.#eventsKey = this.#source.eventsKey();
    this.readPosition = loadReadPosition(this.#eventsKey);
    await this.tick();
  }

  stop(): void {
    this.#dead = true;
    if (this.#timer) clearInterval(this.#timer);
    this.#timer = null;
  }

  async tick(): Promise<void> {
    this.now = new Date();
    if (!this.#source) return;
    try {
      const st = await this.#source.stamp();
      if (st !== this.#last) {
        this.#last = st;
        await this.load();
      }
    } catch (e) {
      this.errors = [(e as Error).message];
    }
  }

  async load(): Promise<void> {
    const src = this.#source;
    if (!src) return;
    const ok: Held[] = [];
    const bad: string[] = [];
    for (const { path, text } of await src.listRecords()) {
      try { ok.push({ path, record: parseRecord(text), text }); }
      catch (e) { bad.push(`${folderName(folderOf(path))}: ${(e as Error).message}`); }
    }
    this.records = ok;
    this.errors = bad;
    this.registry = loadRegistry(await src.listRegistry()).entries;
    this.events = parseEvents(await src.readEvents());
    this.readPosition = reconcileReadPosition(this.#eventsKey, this.readPosition, this.events.length);
    this.heartbeat = await src.readBackendHeartbeat();
    this.loaded = true;
  }

  folderFor(source: Source | undefined, path: string): string | null {
    return nodeFolder(source, path, this.home);
  }

  tasksFor(folder: string | null): Held[] {
    return (folder && this.byFolder.get(folder)) || EMPTY;
  }

  countsFor(folder: string | null): FolderCounts {
    const list = this.tasksFor(folder);
    const byColumn = Object.fromEntries(COLUMNS.map((c) => [c.id, 0])) as Record<ColumnId, number>;
    for (const h of list) byColumn[columnOf(h.record.state)]++;
    return { byColumn, waiting: list.filter((h) => h.record.waiting).length, total: list.length };
  }

  runningFor(folder: string | null): RegistryEntry | null {
    return (folder && this.#running.get(folder)?.[0]) || null;
  }

  entryFor(recordPath: string): RegistryEntry | null {
    return this.registry.find((e) => findByTaskPath(this.records, e.task_path)?.path === recordPath) ?? null;
  }

  titleFor = (taskPath: string): string | undefined => findByTaskPath(this.records, taskPath)?.record.title;

  locate(taskPath: string): Held | undefined {
    return findByTaskPath(this.records, taskPath);
  }

  open(path: string | null): void {
    this.openPath = path;
    this.actionError = null;
  }

  markAllRead(): void {
    this.readPosition = this.events.length;
    saveReadPosition(this.#eventsKey, this.readPosition);
  }

  async #reread(path: string): Promise<void> {
    const text = await this.#source!.readRecord(path);
    const held = { path, record: parseRecord(text), text };
    this.records = this.records.map((h) => (h.path === path ? held : h));
  }

  async answer(h: Held, input: AnswerInput): Promise<void> {
    await submitAnswer(this.#source!, h.path, h.text, input);
    await this.#reread(h.path);
  }

  async approve(h: Held, d: ApprovalDecision): Promise<void> {
    await submitApproval(this.#source!, h.path, h.text, d);
    await this.#reread(h.path);
  }

  async request(h: Held, key: RequestKey): Promise<void> {
    await submitRequest(this.#source!, h.path, h.text, key);
    await this.#reread(h.path);
  }

  async createTask(folder: string, fields: CreateFields): Promise<Held> {
    const { path } = await this.#source!.createRecord(folder, fields);
    await this.load();
    const held = this.records.find((h) => h.path === path);
    if (!held) throw new Error(`created ${path} but could not read it back`);
    return held;
  }

  async createAndDelegate(folder: string, fields: CreateFields): Promise<Held> {
    const made = await this.createTask(folder, fields);
    await this.request(made, 'delegate');
    return this.records.find((h) => h.path === made.path)!;
  }

  async cancelTask(taskPath: string): Promise<void> {
    const h = this.locate(taskPath);
    if (!h) { this.actionError = `no loaded record for ${taskPath}`; return; }
    try { await this.request(h, 'cancel'); this.actionError = null; }
    catch (e) { this.actionError = (e as Error).message; }
  }

  async reload(path: string): Promise<void> {
    await this.#reread(path);
  }
}
