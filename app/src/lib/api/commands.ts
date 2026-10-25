import type {
  Tree, Stamp, Node, NodeRef, RemoveNodeReq, SaveTodoReq, SaveMetaReq, Source, TestResult, Settings, ListDirsReq, DirEntry,
  InitChildrenReq, HostKey, TrustResult, PasswordState, InstallKeyReq, InstallKeyResult,
} from './types';

export interface Command<A extends unknown[], R> {
  get?: true;
  req?: (...args: A) => unknown;
  readonly _res?: R;
}

const op = <A extends unknown[], R>(spec: { get?: true; req?: (...args: A) => unknown } = {}): Command<A, R> => spec;
const self = <T>(r: T) => r;

export const COMMANDS = {
  getTree: op<[showAll: boolean, rescan?: boolean], Tree>({ get: true, req: (show_all, rescan = false) => ({ show_all, rescan }) }),
  getStamp: op<[], Stamp>({ get: true }),
  saveTodo: op<[req: SaveTodoReq], Node>({ req: self }),
  saveMeta: op<[req: SaveMetaReq], Node>({ req: self }),
  initNode: op<[req: NodeRef], Node>({ req: self }),
  removeNode: op<[req: RemoveNodeReq], Node>({ req: self }),
  listSources: op<[], Source[]>({ get: true }),
  saveSources: op<[sources: Source[]], Source[]>({ req: (sources) => ({ sources }) }),
  testSource: op<[source: Source], TestResult>({ req: self }),
  getSettings: op<[], Settings>({ get: true }),
  saveSettings: op<[settings: Settings], Settings>({ req: self }),
  openPath: op<[req: NodeRef], void>({ req: self }),
  openUrl: op<[url: string], void>({ req: (url) => ({ url }) }),
  listDirs: op<[req: ListDirsReq], DirEntry[]>({ req: self }),
  initChildren: op<[req: InitChildrenReq], Node>({ req: self }),
  revealPath: op<[path: string], void>({ req: (path) => ({ path }) }),
  hostKey: op<[host: string], HostKey[]>({ req: (host) => ({ host }) }),
  trustHost: op<[host: string, lines: string[]], TrustResult>({ req: (host, lines) => ({ host, lines }) }),
  setSourcePassword: op<[source: string, password: string], PasswordState>({ req: (source, password) => ({ source, password }) }),
  hasSourcePassword: op<[source: string], PasswordState>({ req: (source) => ({ source }) }),
  installKey: op<[req: InstallKeyReq], InstallKeyResult>({ req: self }),
};

type Commands = typeof COMMANDS;

export type Api = {
  [K in keyof Commands]: Commands[K] extends Command<infer A, infer R> ? (...args: A) => Promise<R> : never;
};

export type Transport = (name: string, get: boolean, req: unknown) => Promise<unknown>;

export const snake = (key: string): string => key.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);

export function buildApi(transport: Transport, overrides: Partial<Api> = {}): Api {
  const out: Record<string, unknown> = {};
  for (const [key, spec] of Object.entries(COMMANDS) as [string, Command<unknown[], unknown>][]) {
    const name = snake(key);
    out[key] = (...args: unknown[]) => transport(name, !!spec.get, spec.req ? spec.req(...args) : undefined);
  }
  return { ...(out as Api), ...overrides };
}
