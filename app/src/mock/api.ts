import type { Api } from '../lib/api/index';
import {
  ApiError, type Item, type Meta, type Node, type NodeRef, type Settings, type Source, type TestResult, type Tree, type DirEntry,
} from '../lib/api/types';
import { buildSample, isoDay, LAB, SAMPLE } from './sample';

interface MockOpts {
  empty?: boolean;
  multi?: boolean;
  today?: string;
  latency?: number;
}

export function createMockApi(opts: MockOpts = {}): Api {
  const empty = opts.empty ?? (typeof location !== 'undefined' && new URLSearchParams(location.search).has('empty'));
  const multi = opts.multi ?? (typeof location !== 'undefined' && new URLSearchParams(location.search).has('multi'));
  const today = opts.today ?? isoDay(new Date());
  const latency = opts.latency ?? 60;
  let sources: Source[] = empty ? [] : [
    { id: 'local', name: 'Home', host: null, root: '~/fabr-sample', prune: null },
    ...(multi ? [{ id: 'lab', name: 'Lab server', host: 'lab.local', root: '/srv/fabr', prune: null }] : []),
  ];
  let settings: Settings = { poll_ms: 2000, remote_poll_ms: 30000, search_depth: 6, full_rescan_ms: 30000, max_nodes: 10000, log_path: '/tmp/fabr/fabr.log' };
  let rev = 1;
  const specs: Record<string, typeof SAMPLE> = { local: SAMPLE, lab: LAB };
  const PLAIN_DIRS = ['docs', 'notes', 'projects', 'node_modules'];
  const specFor = (src: Source): typeof SAMPLE =>
    specs[src.id] ?? [['', null, null], ...PLAIN_DIRS.map((d): (typeof SAMPLE)[number] => [d, null, null])];
  const PRUNED = new Set(['node_modules', '.git', 'target', 'dist', 'build', 'venv', '__pycache__']);
  const edits = new Map<string, Node>();
  const created = new Set<string>();
  const removed = new Set<string>();
  const trusted = new Set<string>();
  const passwords = new Map<string, string>();
  const keyInstalled = new Set<string>();
  const MOCK_PW = 'secret';
  const fail = (kind: TestResult['failure_kind'], message: string, detail: string): TestResult =>
    ({ ok: false, message, latency_ms: 0, node_count: 0, failure_kind: kind, detail });

  const wait = () => new Promise((r) => setTimeout(r, latency));
  const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v));
  const key = (s: string, p: string) => `${s}:${p}`;
  const ver = () => (rev++).toString(16).padStart(40, '0');

  function sourceRoot(src: Source, showAll: boolean): Node {
    const root = buildSample(src.id, today, specFor(src).map(([path, meta, todos]): (typeof SAMPLE)[number] =>
      meta === null && created.has(key(src.id, path)) ? [path, { name: path.split('/').pop() || src.name }, []]
        : removed.has(key(src.id, path)) ? [path, null, null] : [path, meta, todos]), showAll);
    root.name = src.name;
    (function walk(n: Node) {
      const k = key(n.source, n.path);
      const e = edits.get(k);
      if (e) Object.assign(n, { ...e, children: n.children });
      else if (created.has(k)) Object.assign(n, { managed: true, has_todo: true, version: ver(), meta: { name: n.name } });
      n.children.forEach(walk);
    })(root);
    return root;
  }

  let last: Tree | null = null;
  function tree(showAll: boolean): Tree {
    const roots = sources.map((s) => sourceRoot(s, showAll));
    const root: Node = {
      source: '', path: '', name: '/', managed: false, meta: {}, items: [], warnings: [],
      version: null, via: null, has_todo: false, children: roots,
    };
    last = { root, sources: clone(sources), today, truncated: false, errors: [], pending: [] };
    return last;
  }

  function find(ref: NodeRef, all = false): Node {
    const look = (t: Tree): Node | null => {
      let hit: Node | null = null;
      (function walk(n: Node) {
        if (n.source === ref.source && n.path === ref.path) hit = n;
        n.children.forEach(walk);
      })(t.root);
      return hit;
    };
    const hit = (!all && last && look(last)) || look(tree(all));
    if (!hit) throw new ApiError(404, `no such node: ${ref.source}:${ref.path}`);
    return hit;
  }

  function write(ref: NodeRef, version: string, patch: Partial<Node>): Node {
    const n = find(ref);
    if (!n.managed) throw new ApiError(404, 'not a fabr node');
    if (n.version !== version) {
      throw new ApiError(409, 'node changed on disk', { ...clone(n), children: [] });
    }
    Object.assign(n, patch, { version: ver() });
    const saved = { ...clone(n), children: [] };
    edits.set(key(n.source, n.path), saved);
    return saved;
  }

  const api: Api = {
    async getTree(showAll) { await wait(); return clone(tree(showAll)); },
    async getStamp() { await wait(); return { stamp: String(rev) }; },
    async saveTodo(req) {
      await wait();
      for (const i of req.items) {
        if (!i.title.trim()) throw new ApiError(400, 'item title must not be empty');
        if (i.due && !/^\d{4}-\d{2}-\d{2}(T([01]\d|2[0-3]):[0-5]\d)?$/.test(i.due)) throw new ApiError(400, `bad date: ${i.due}`);
      }
      const items: Item[] = req.items.map((i) => ({ ...i, id: i.id ?? Math.random().toString(16).slice(2, 6) }));
      return write(req, req.version, { items, has_todo: true });
    },
    async saveMeta(req) {
      await wait();
      const meta: Meta = { ...req.meta };
      return write(req, req.version, { meta });
    },
    async initNode(req) {
      await wait();
      const n = find(req);
      if (n.managed) throw new ApiError(409, 'already a node', { ...clone(n), children: [] });
      created.add(key(req.source, req.path));
      removed.delete(key(req.source, req.path));
      rev++;
      return { ...clone(n), managed: true, has_todo: true, version: ver(), meta: { name: n.name } };
    },
    async removeNode(req) {
      await wait();
      if (!req.source) throw new ApiError(400, 'no source given');
      const n = find(req);
      if (!n.managed) throw new ApiError(404, 'directory is not a fabr node');
      if (n.version !== req.version) throw new ApiError(409, 'the node changed on disk since you loaded it', { ...clone(n), children: [] });
      const k = key(req.source, req.path);
      edits.delete(k);
      created.delete(k);
      removed.add(k);
      rev++;
      return { ...clone(n), managed: false, meta: {}, items: [], warnings: [], version: null, via: null, has_todo: false, children: [] };
    },
    async listDirs(req): Promise<DirEntry[]> {
      await wait();
      const n = find(req, true);
      return n.children.map((c) => ({ name: c.name, managed: c.managed, pruned: false }))
        .concat(PLAIN_DIRS.filter((d) => PRUNED.has(d) && !n.children.some((c) => c.name === d) && !specs[n.source]).map((d) => ({ name: d, managed: false, pruned: true })))
        .sort((a, b) => a.name.localeCompare(b.name));
    },
    async initChildren(req) {
      await wait();
      const n = find(req, true);
      const names = new Set(n.children.map((c) => c.name));
      for (const name of req.names) {
        if (!names.has(name)) throw new ApiError(404, `no such directory: ${name}`);
      }
      created.add(key(req.source, req.path));
      for (const name of req.names) created.add(key(req.source, req.path ? `${req.path}/${name}` : name));
      rev++;
      return { ...clone(find(req, true)), children: [] };
    },
    async listSources() { await wait(); return clone(sources); },
    async saveSources(next) {
      await wait();
      for (const s of next) if (!/^[a-z0-9_-]+$/.test(s.id)) throw new ApiError(400, `source id ${JSON.stringify(s.id)} must match ^[a-z0-9_-]+$`);
      sources = clone(next);
      rev++;
      return clone(sources);
    },
    async testSource(s): Promise<TestResult> {
      if (!/^[a-z0-9_-]+$/.test(s.id)) throw new ApiError(400, `source id ${JSON.stringify(s.id)} must match ^[a-z0-9_-]+$`);
      await new Promise((r) => setTimeout(r, latency ? 400 : 0));
      const h = s.host ?? '';
      if (h.includes('bad')) return fail('connect_failed', `Could not connect: Could not resolve hostname ${h}: nodename nor servname provided, or not known`, `ssh: Could not resolve hostname ${h}: nodename nor servname provided, or not known`);
      if (h.includes('changed')) return fail('host_key_changed', `Host key changed: ${h} presents a different key than the one in your known_hosts`, `Host key for ${h} has changed and you have requested strict checking.`);
      if (h.includes('new') && !trusted.has(h)) return fail('host_key_unknown', `Unknown host key: ${h} is not in your known_hosts yet`, `No ED25519 host key is known for ${h} and you have requested strict checking.`);
      if (h.includes('pw') && !keyInstalled.has(h) && (s.password || passwords.get(s.id)) !== MOCK_PW) {
        return fail('auth_failed', 'Authentication failed: the host did not accept your key or password', `me@${h}: Permission denied (publickey,password).`);
      }
      return { ok: true, message: 'ok', latency_ms: s.host ? 84 : 3, node_count: specs[s.id] ? specs[s.id].length : 0, managed_root: !!specs[s.id] || created.has(key(s.id, '')) };
    },
    async hostKey(host) {
      await wait();
      if (host.includes('bad')) throw new ApiError(502, `Could not connect: no host keys from ${host}:22`);
      return [
        { type: 'ssh-ed25519', fingerprint: 'SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU', line: `${host} ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl` },
        { type: 'ecdsa-sha2-nistp256', fingerprint: 'SHA256:p2QAMXNIC1TJYWeIOttrVc98/R1BUFWu3/LiyKgUfQM', line: `${host} ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTY=` },
      ];
    },
    async trustHost(host, lines) {
      await wait();
      if (host.includes('changed')) throw new ApiError(409, `host_key_changed: your known_hosts already has a different key for ${host}; nothing was changed`);
      const added = trusted.has(host) ? 0 : lines.length;
      trusted.add(host);
      return { added };
    },
    async setSourcePassword(source, password) {
      if (!/^[a-z0-9_-]+$/.test(source)) throw new ApiError(400, `source id ${JSON.stringify(source)} must match ^[a-z0-9_-]+$`);
      await wait();
      if (password) passwords.set(source, password);
      else passwords.delete(source);
      return { stored: !!password };
    },
    async hasSourcePassword(source) { await wait(); return { stored: passwords.has(source) }; },
    async installKey(req) {
      await wait();
      const host = req.host ?? sources.find((s) => s.id === req.source)?.host ?? '';
      if (!host) throw new ApiError(400, `source "${req.source}" has no ssh host`);
      if (host.includes('pw') && (req.password || passwords.get(req.source)) !== MOCK_PW && !keyInstalled.has(host)) {
        throw new ApiError(502, 'Authentication failed: the host did not accept your key or password');
      }
      keyInstalled.add(host);
      return { installed: true, generated_key: false, message: `Key installed: ${host} now accepts ~/.ssh/id_ed25519 without a password.` };
    },
    async getSettings() { await wait(); return clone(settings); },
    async saveSettings(next) { await wait(); settings = { ...settings, ...clone(next) }; return clone(settings); },
    async revealPath() {
      await wait();
      throw new ApiError(501, 'reveal_path is only available in the desktop app');
    },
    async openUrl() {},
    async openPath(req) {
      await wait();
      const src = sources.find((s) => s.id === req.source);
      if (src?.host) throw new ApiError(400, 'cannot open a remote path');
      throw new ApiError(501, 'open_path is not available in the browser');
    },
  };
  return api;
}
