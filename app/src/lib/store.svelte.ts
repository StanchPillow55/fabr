import { SvelteSet } from 'svelte/reactivity';
import type { Api } from './api/index';
import { isTauri } from './api/env';
import { ApiError, errText, type Item, type Meta, type Node, type Settings, type Source, type Status, type Tree } from './api/types';
import type { Filter, GroupBy, Key, SortBy, SortDir, ViewMode } from './model';
import { COLOR_NAMES, COLOR_RE, colorSpec, computeForceOpen, effectiveColors, indexTree, isCollapsed, isVirtualRoot, nodeKey, pruneEmpty, visibleTree } from './model';
import { getPref, persist, restore, setPref } from './prefs';
import { crumbSegments, type PathSegment } from './viewstate';
import { nameFromPath, slugify } from './addnodes';

export type View = 'tree' | 'list';
export type SheetPage = 'sources';
export type Theme = 'system' | 'light' | 'dark';
export type Tint = (typeof COLOR_NAMES)[number];
type NoteSlot = 'metaNote' | 'itemsNote';

export interface Banner {
  kind: '' | 'error';
  text: string;
}

export interface PanelState {
  key: Key;
  source: string;
  path: string;
  version: string | null;
  managed: boolean;
  name: string;
  original: Meta;
  warnings: string[];
  items: Item[];
  meta: [string, string | number | boolean][];
  metaNote: { key: string; kind: 'ok' | 'error'; text: string } | null;
  itemsNote: { kind: 'ok' | 'error'; text: string } | null;
  banner: Banner | null;
}

export interface ListState {
  sortBy: SortBy;
  sortDir: SortDir;
  groupBy: GroupBy;
}

export class Store {
  api: Api;

  tree = $state<Tree | null>(null);
  today = $state('');
  stamp = $state<string | null>(null);
  loading = $state(false);
  notice = $state<string | null>(null);

  view = $state<View>('tree');
  viewMode = $state<ViewMode>('folders');
  showHidden = $state(false);
  filter = $state<Filter>('all');
  hideDone = $state(false);
  hideEmpty = $state(false);
  query = $state('');
  focus = $state<Key | null>(null);
  selected = $state<Key | null>(null);

  collapsed = new SvelteSet<Key>();
  moreOpen = new SvelteSet<Key>();

  byKey = $state(new Map<Key, Node>());
  parentOf = $state(new Map<Key, Node>());

  panel = $state<PanelState | null>(null);
  list = $state<ListState>({ sortBy: 'pri', sortDir: 'asc', groupBy: 'none' });

  settings = $state<Settings>({ poll_ms: 2000, remote_poll_ms: 30000, max_nodes: 10000 });
  sources = $state<Source[]>([]);
  sheetOpen = $state(false);
  sheetPage = $state<SheetPage | null>(null);
  addNodesFor = $state<{ source: string; path: string; name: string } | null>(null);
  noticeRescan = $state(false);

  theme = $state<Theme>('system');
  tint = $state<Tint>('blue');

  constructor(api: Api) {
    this.api = api;
  }

  get showAll(): boolean {
    return this.viewMode === 'all';
  }

  #shaped = $derived(this.tree ? visibleTree(this.tree.root, { mode: this.viewMode, showHidden: this.showHidden }) : null);
  #shown = $derived(
    this.#shaped && this.hideEmpty
      ? pruneEmpty(this.#shaped, { today: this.today, filter: this.filter, hideDone: this.hideDone }, this.focus)
      : this.#shaped,
  );
  #tallyIndex = $derived(this.hideEmpty && this.#shaped ? indexTree(this.#shaped) : null);
  tallyNode(n: Node): Node {
    return this.#tallyIndex?.byKey.get(nodeKey(n)) ?? n;
  }
  #viewIndex = $derived(this.#shown ? indexTree(this.#shown) : { byKey: new Map<Key, Node>(), parentOf: new Map<Key, Node>() });

  #colors = $derived(this.tree ? effectiveColors(this.tree.root, this.tint) : new Map<Key, string>());
  get colors(): Map<Key, string> {
    return this.#colors;
  }

  layoutVersion = $state(0);
  relayout(): void {
    this.layoutVersion++;
  }

  get viewIndex() {
    return this.#viewIndex;
  }

  #timer: ReturnType<typeof setTimeout> | null = null;
  #polling = false;
  #loadSeq = 0;
  revealUnsupported = $state(!isTauri());

  async start(): Promise<void> {
    this.restorePrefs();
    this.applyAppearance();
    try {
      const [settings, sources] = await Promise.all([this.api.getSettings(), this.api.listSources()]);
      this.settings = settings;
      this.sources = sources;
    } catch (e) {
      this.notice = `Could not load settings: ${errText(e)}`;
    }
    await this.load();
    await this.poll();
    this.schedulePoll();
  }

  stop(): void {
    if (this.#retryTimer) clearTimeout(this.#retryTimer);
    this.#retryTimer = null;
    if (this.#timer) clearTimeout(this.#timer);
    this.#timer = null;
  }

  schedulePoll(): void {
    this.stop();
    const ms = Math.max(250, this.settings.poll_ms || 2000);
    this.#timer = setTimeout(async () => {
      await this.poll();
      this.schedulePoll();
    }, ms);
  }

  restorePrefs(): void {
    restore(this, 'prefs');
    restore(this.list, 'list');
    this.collapsed.clear();
    for (const k of getPref<string[]>('collapsed', [])) this.collapsed.add(k);
    const t = getPref<string>('theme', 'system');
    this.theme = t === 'light' || t === 'dark' ? t : 'system';
    const c = getPref<string>('tint', 'blue');
    if (TINTS.includes(c as Tint)) this.tint = c as Tint;
  }

  savePrefs(): void {
    this.relayout();
    persist(this, 'prefs');
    persist(this.list, 'list');
  }

  saveFolds(): void {
    this.relayout();
    setPref('collapsed', [...this.collapsed]);
  }

  setTheme(t: Theme): void {
    this.theme = t;
    setPref('theme', t);
    this.applyAppearance();
  }

  setTint(t: Tint): void {
    this.tint = t;
    setPref('tint', t);
    this.applyAppearance();
  }

  applyAppearance(): void {
    if (typeof document === 'undefined') return;
    const r = document.documentElement;
    if (this.theme === 'system') r.removeAttribute('data-theme');
    else r.setAttribute('data-theme', this.theme);
    r.setAttribute('data-tint', this.tint);
  }

  async setViewMode(m: ViewMode): Promise<void> {
    this.viewMode = m;
    this.savePrefs();
    this.#fixFocus();
    if (this.showAll !== this.#treeShowAll) await this.load();
  }

  setView(field: 'hideDone' | 'hideEmpty' | 'showHidden', v: boolean): void {
    this[field] = v;
    this.savePrefs();
    if (field === 'showHidden') this.#fixFocus();
  }

  setHideDone(v: boolean): void {
    this.setView('hideDone', v);
  }

  setHideEmpty(v: boolean): void {
    this.setView('hideEmpty', v);
  }

  setShowHidden(v: boolean): void {
    this.setView('showHidden', v);
  }

  #fixFocus(): void {
    if (this.focus !== null && !this.#viewIndex.byKey.has(this.focus)) {
      this.focus = null;
      this.savePrefs();
    }
  }

  toggleMore(k: Key): void {
    if (this.moreOpen.has(k)) this.moreOpen.delete(k);
    else this.moreOpen.add(k);
  }

  get displayRoot(): Node | null {
    if (!this.tree || !this.#shown) return null;
    return (this.focus !== null && this.#viewIndex.byKey.get(this.focus)) || this.#shown;
  }

  pathSegments(): PathSegment[] {
    const at = this.focus === null ? null : (this.#viewIndex.byKey.get(this.focus) ?? this.byKey.get(this.focus) ?? null);
    return crumbSegments(at, (id) => this.sourceOf(id)?.name ?? '');
  }

  openSettings(page: SheetPage | null = null): void {
    this.sheetPage = page;
    this.sheetOpen = true;
  }

  get rootKey(): Key | null {
    return this.tree ? nodeKey(this.tree.root) : null;
  }

  get multiSource(): boolean {
    return (this.tree?.sources.length ?? 0) > 1;
  }

  sourceOf(id: string): Source | undefined {
    return this.tree?.sources.find((s) => s.id === id) ?? this.sources.find((s) => s.id === id);
  }

  async saveConfig(sources: Source[], settings: Settings): Promise<void> {
    this.sources = await this.api.saveSources(sources);
    this.settings = await this.api.saveSettings(settings);
    this.schedulePoll();
    void this.load();
  }

  async saveSourceList(sources: Source[]): Promise<void> {
    this.sources = await this.api.saveSources(sources);
    void this.load();
  }

  async saveSettingsOnly(settings: Settings): Promise<void> {
    this.settings = await this.api.saveSettings(settings);
    this.schedulePoll();
    void this.load();
  }

  async openUrl(url: string): Promise<void> {
    try {
      await this.api.openUrl(url);
    } catch (e) {
      this.notice = `Could not open the link: ${errText(e)}`;
      this.noticeRescan = false;
    }
  }

  async reveal(): Promise<void> {
    const p = this.panel;
    if (!p) return;
    try {
      await this.api.openPath({ source: p.source, path: p.path });
    } catch (e) {
      if (e instanceof ApiError && e.code === 501) this.revealUnsupported = true;
      else p.banner = { kind: 'error', text: errText(e) };
    }
  }

  async rescan(): Promise<void> {
    await this.load(true);
  }

  async load(rescan = false): Promise<void> {
    await this.#load(rescan, 0);
  }

  #retryTimer: ReturnType<typeof setTimeout> | null = null;
  #treeShowAll: boolean | null = null;

  async #load(rescan: boolean, retry: number): Promise<void> {
    const seq = ++this.#loadSeq;
    if (this.#retryTimer) clearTimeout(this.#retryTimer);
    this.#retryTimer = null;
    this.loading = true;
    try {
      const showAll = this.showAll;
      const t: Tree = await this.api.getTree(showAll, rescan || undefined);
      if (seq !== this.#loadSeq) return;
      this.#treeShowAll = showAll;
      const before = this.tree ? this.#viewIndex.byKey.size : -1;
      this.tree = t;
      this.today = t.today;
      const idx = indexTree(this.tree.root);
      this.byKey = idx.byKey;
      this.parentOf = idx.parentOf;
      if (this.focus !== null && !this.#viewIndex.byKey.has(this.focus)) this.focus = null;
      if (this.#viewIndex.byKey.size !== before) this.relayout();
      if (this.selected !== null && !this.byKey.has(this.selected) && !this.panel) this.selected = null;
      this.notice = t.truncated ? t.truncated_reason || 'Tree truncated: too many directories. Switch the view to Nodes or pick a smaller root.' : null;
      this.noticeRescan = t.truncated;
      if (t.truncated && t.truncated_by === 'time' && visitedIn(t.truncated_reason) < SLOW_SCAN_DIRS) {
        if (retry < 3) {
          this.noticeRescan = false;
          this.#retryTimer = setTimeout(() => void this.#load(true, retry + 1), 1000);
        } else {
          this.notice = (t.truncated_reason ?? '').replace(/;?\s*rescanning…/, '') + '. Press Rescan to try again.';
        }
      }
      this.syncPanel();
    } catch (e) {
      if (seq === this.#loadSeq) this.notice = `Could not load tree: ${errText(e)}`;
    } finally {
      if (seq === this.#loadSeq) this.loading = false;
    }
  }

  async poll(): Promise<void> {
    if (this.#polling) return;
    this.#polling = true;
    try {
      const { stamp } = await this.api.getStamp();
      if (this.notice === LOST) this.notice = null;
      if (stamp !== this.stamp) {
        const had = this.stamp !== null;
        this.stamp = stamp;
        if (had) await this.load();
      }
    } catch {
      this.notice = LOST;
      this.noticeRescan = false;
    } finally {
      this.#polling = false;
    }
  }

  replaceNode(node: Node): void {
    const old = this.byKey.get(nodeKey(node));
    if (!old) return;
    const { children: _children, via: _via, ...rest } = node;
    Object.assign(old, rest);
  }

  async cycleStatus(k: Key, index: number): Promise<void> {
    const n = this.byKey.get(k);
    if (!n || !n.managed || !n.version || !n.items[index]) return;
    await this.commitItems(k, (items) => (items[index] ? items.map((i, j) => (j === index ? { ...i, status: nextStatus(i.status) } : i)) : null));
  }

  async quickAdd(k: Key, title: string): Promise<boolean> {
    const n = this.byKey.get(k);
    const text = title.trim();
    if (!n || !n.managed || !n.version || !text) return false;
    return this.commitItems(k, (items) => [...items, { id: null, status: 'todo' as const, due: null, title: text, pri: null }]);
  }

  openPanel(k: Key): void {
    const n = this.byKey.get(k);
    if (!n) return;
    this.selected = k;
    if (this.panel && this.panel.key === k) return;
    const was = !!this.panel;
    this.itemFocus = false;
    this.#resetPanel(n);
    if (!was) this.relayout();
  }

  closePanel(): void {
    if (this.#inflight > 0) {
      const k = this.panel?.key;
      const close = () => {
        if (this.panel && this.panel.key === k) this.#closeNow();
      };
      this.#chain.then(close, close);
      return;
    }
    this.#closeNow();
  }

  #closeNow(): void {
    this.itemFocus = false;
    this.#pendingSync = false;
    const was = !!this.panel;
    this.panel = null;
    this.selected = null;
    if (was) this.relayout();
  }

  syncPanel(): void {
    const p = this.panel;
    if (!p) return;
    const n = this.byKey.get(p.key);
    if (!n) {
      p.banner = { kind: 'error', text: 'This directory is no longer in the tree.' };
      return;
    }
    if (n.version === p.version && n.managed === p.managed) return;
    if (this.itemFocus) {
      this.#pendingSync = true;
      return;
    }
    this.#resetPanel(n);
  }

  itemFocus = false;
  #pendingSync = false;

  setItemFocus(on: boolean): void {
    this.itemFocus = on;
    if (on || !this.#pendingSync) return;
    const flush = () => {
      if (this.itemFocus || !this.#pendingSync) return;
      this.#pendingSync = false;
      this.syncPanel();
    };
    if (this.#inflight === 0) flush();
    else this.#chain.then(flush, flush);
  }

  #inflight = 0;

  #chain: Promise<unknown> = Promise.resolve();

  #enqueue(run: () => Promise<boolean>): Promise<boolean> {
    const out = this.#chain.then(run, run);
    this.#chain = out.catch(() => false);
    return out;
  }

  commitItems(k: Key, mutate: (items: Item[]) => Item[] | null): Promise<boolean> {
    this.#inflight++;
    return this.#enqueue(async () => {
      try {
        const n = this.byKey.get(k);
        if (!n || !n.managed || !n.version) return false;
        const next = mutate(n.items.map((i) => ({ ...i })));
        if (!next) return true;
        let node: Node;
        try {
          node = await this.api.saveTodo({ source: n.source, path: n.path, version: n.version, items: next });
        } catch (e) {
          if (this.#conflict(e, (p, fresh) => (p.version = fresh.version))) return false;
          const msg = errText(e) || 'save failed';
          if (this.panel?.key === k) this.#flash('itemsNote', { kind: 'error', text: msg });
          else this.notice = msg;
          return false;
        }
        this.replaceNode(node);
        const p = this.panel;
        if (p && p.key === k) {
          p.items = node.items.map((i) => ({ ...i }));
          p.version = node.version;
          p.warnings = [...node.warnings];
          if (p.banner?.text === CONFLICT) p.banner = null;
          this.#flash('itemsNote', { kind: 'ok', text: 'Saved' });
        }
        return true;
      } finally {
        this.#inflight--;
      }
    });
  }

  #seq: Record<NoteSlot, number> = { metaNote: 0, itemsNote: 0 };

  #flash<S extends NoteSlot>(slot: S, note: NonNullable<PanelState[S]>): void {
    if (!this.panel) return;
    this.panel[slot] = note;
    const n = ++this.#seq[slot];
    if (note.kind === 'ok') {
      setTimeout(() => {
        if (this.#seq[slot] === n && this.panel) this.panel[slot] = null;
      }, 1800);
    }
  }

  #conflict(e: unknown, onPanel: (p: PanelState, node: Node) => void): boolean {
    if (!(e instanceof ApiError && e.code === 409 && e.node)) return false;
    const node = e.node;
    this.replaceNode(node);
    const p = this.panel;
    if (p && p.key === nodeKey(node)) {
      onPanel(p, node);
      this.panel!.banner = { kind: '', text: CONFLICT };
    } else {
      this.notice = CHANGED;
    }
    return true;
  }

  async saveMeta(note?: string): Promise<boolean> {
    const p = this.panel;
    if (!p || !p.version) return false;
    let node: Node;
    try {
      node = await this.api.saveMeta({ source: p.source, path: p.path, version: p.version, meta: metaPayload(p) });
    } catch (e) {
      this.#saveFailed(e);
      if (note && this.panel && this.panel.key === p.key && !(e instanceof ApiError && e.code === 409)) {
        this.#flash('metaNote', { key: note, kind: 'error', text: errText(e) || 'save failed' });
      }
      return false;
    }
    this.replaceNode(node);
    if (this.panel && this.panel.key === p.key) {
      this.#resetPanel(node);
      if (note) this.#flash('metaNote', { key: note, kind: 'ok', text: 'Saved' });
    }
    await this.load();
    return true;
  }

  commitMeta(field: string, value: string | number | boolean | undefined): Promise<boolean> {
    return this.#enqueue(async () => {
      const p = this.panel;
      if (!p) return false;
      const i = p.meta.findIndex(([k]) => k === field);
      if (value === undefined) {
        if (i >= 0) p.meta.splice(i, 1);
      } else if (i >= 0) {
        if (p.meta[i][1] === value) return true;
        p.meta[i][1] = value;
      } else {
        p.meta.push([field, value]);
      }
      return this.saveMeta(field);
    });
  }

  async setHidden(hidden: boolean): Promise<boolean> {
    const p = this.panel;
    if (!p) return false;
    const at = p.key;
    const ok = await this.commitMeta('hidden', hidden);
    if (ok) this.relayout();
    if (ok && hidden && !this.showHidden && this.panel?.key === at) {
      this.panel = null;
      this.selected = null;
    }
    return ok;
  }

  inheritedColor(k: Key): string {
    for (let p = this.parentOf.get(k); p; p = this.parentOf.get(nodeKey(p))) {
      const c = colorSpec(p);
      if (c) return c;
    }
    return this.tint;
  }

  setColor(c: string | null): Promise<boolean> {
    return this.commitMeta('color', c ?? undefined);
  }

  async onboard(root: string, name: string): Promise<void> {
    root = root.trim();
    name = name.trim() || nameFromPath(root);
    const src: Source = { id: slugify(name), name, host: null, root, prune: null };
    this.sources = await this.api.saveSources([src]);
    const t = await this.api.testSource(src);
    if ((t as { managed_root?: boolean }).managed_root === false) await this.api.initNode({ source: src.id, path: '' });
    await this.load();
    this.schedulePoll();
    this.addNodesFor = { source: src.id, path: '', name };
  }

  async initRoots(ids: string[]): Promise<void> {
    for (const id of ids) await this.api.initNode({ source: id, path: '' });
    if (ids.length) void this.load();
  }

  openAddNodes(k: Key): void {
    const n = this.byKey.get(k);
    if (!n) return;
    this.addNodesFor = { source: n.source, path: n.path, name: String(n.meta.name || n.name) };
  }

  async addNodes(source: string, path: string, names: string[]): Promise<void> {
    const node = await this.api.initChildren({ source, path, names });
    this.replaceNode(node);
    await this.load();
    this.relayout();
  }

  async init(k: Key): Promise<void> {
    const n = this.byKey.get(k);
    if (!n) return;
    try {
      await this.api.initNode({ source: n.source, path: n.path });
    } catch (e) {
      if (this.panel && this.panel.key === k) this.panel.banner = { kind: 'error', text: errText(e) || 'could not create node' };
      else this.notice = errText(e);
      return;
    }
    await this.load();
    this.relayout();
    const m = this.byKey.get(k);
    if (m && this.panel && this.panel.key === k) this.#resetPanel(m);
  }

  async removeNode(): Promise<boolean> {
    const p = this.panel;
    if (!p || !p.managed || !p.version || !p.source) return false;
    const at = p.key;
    const name = p.name;
    try {
      await this.api.removeNode({ source: p.source, path: p.path, version: p.version });
    } catch (e) {
      if (e instanceof ApiError && e.code === 409 && e.node && e.node.version === p.version) {
        p.banner = { kind: 'error', text: e.message };
      } else {
        this.#saveFailed(e);
      }
      await this.load();
      return false;
    }
    this.panel = null;
    this.selected = null;
    await this.load();
    this.relayout();
    if (!this.#viewIndex.byKey.has(at)) this.notice = `Removed node ${name}; the folder is still there`;
    return true;
  }

  toggleCollapse(k: Key): void {
    const n = this.byKey.get(k);
    if (!n) return;
    const forceOpen = computeForceOpen(this.#viewIndex, this.query);
    if (isCollapsed(n, { collapsed: this.collapsed, forceOpen })) this.collapsed.delete(k);
    else this.collapsed.add(k);
    this.saveFolds();
  }

  setFocus(k: Key | null): void {
    this.focus = k === null || k === this.rootKey || !this.byKey.has(k) ? null : k;
    this.savePrefs();
  }

  expandAll(): void {
    this.collapsed.clear();
    this.saveFolds();
  }

  collapseAll(): void {
    const root = this.displayRoot;
    if (!root) return;
    let parents = [root];
    if (isVirtualRoot(root)) {
      this.collapsed.delete(nodeKey(root));
      for (const s of root.children) this.collapsed.delete(nodeKey(s));
      parents = root.children;
    }
    for (const p of parents) {
      for (const c of p.children) {
        if (c.children.length) this.collapsed.add(nodeKey(c));
      }
    }
    this.saveFolds();
  }

  #resetPanel(n: Node): void {
    this.panel = {
      key: nodeKey(n),
      source: n.source,
      path: n.path,
      version: n.version,
      managed: n.managed,
      name: String(n.meta.name || n.name),
      original: { ...n.meta },
      warnings: [...n.warnings],
      items: n.items.map((i) => ({ ...i })),
      meta: Object.entries({ ...KNOWN_META, ...n.meta }),
      metaNote: null,
      itemsNote: null,
      banner: null,
    };
  }

  #saveFailed(e: unknown): void {
    if (this.#conflict(e, (_, node) => this.#resetPanel(node))) return;
    const msg = errText(e) || 'save failed';
    if (this.panel) this.panel.banner = { kind: 'error', text: msg };
    else this.notice = msg;
  }
}

export const STATUSES: Status[] = ['todo', 'wip', 'done'];
export const TINTS = COLOR_NAMES;
export const KNOWN_META: Meta = { name: '', kind: '', description: '', show_depth: 0, hidden: false };
const SLOW_SCAN_DIRS = 2000;
const CONFLICT = 'This node changed on disk; reloaded';
const CHANGED = 'That node changed on disk; it has been reloaded. Try again.';
const LOST = 'Lost connection to the fabr server.';

export function nextStatus(s: Status): Status {
  return STATUSES[(STATUSES.indexOf(s) + 1) % STATUSES.length];
}

export function metaPayload(p: Pick<PanelState, 'meta' | 'original'>): Meta {
  const out: Meta = {};
  for (const [k, v] of p.meta) {
    if (k in KNOWN_META && !(k in p.original) && v === KNOWN_META[k]) continue;
    if (k === 'color' && !(typeof v === 'string' && COLOR_RE.test(v))) continue;
    out[k] = v;
  }
  return out;
}

export function removeSummary(n: Pick<Node, 'items'>): { todos: number; open: number; text: string } {
  const todos = n.items.length;
  const open = n.items.filter((i) => i.status !== 'done').length;
  const text = todos === 0 ? 'no todos will be deleted' : `${todos} ${todos === 1 ? 'todo' : 'todos'} (${open} open) will be deleted`;
  return { todos, open, text };
}

export function visitedIn(reason: string | null | undefined): number {
  const m = /\((\d+) directories/.exec(reason ?? '');
  return m ? Number(m[1]) : 0;
}
