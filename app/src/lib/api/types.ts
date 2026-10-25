export type Status = 'todo' | 'wip' | 'done';

export interface Item {
  id: string | null;
  status: Status;
  due: string | null;
  title: string;
  pri: number | null;
}

export type MetaValue = string | number | boolean;
export type Meta = Record<string, MetaValue>;

export interface Node {
  source: string;
  path: string;
  name: string;
  managed: boolean;
  meta: Meta;
  items: Item[];
  warnings: string[];
  version: string | null;
  via: string | null;
  has_todo: boolean;
  children: Node[];
}

export interface Source {
  id: string;
  name: string;
  host: string | null;
  root: string;
  prune: string[] | null;
  search_depth?: number | null;
  password?: string;
}

export interface Settings {
  poll_ms: number;
  remote_poll_ms: number;
  search_depth?: number;
  full_rescan_ms?: number;
  max_nodes?: number;
  log_path?: string | null;
}

export interface TestResult {
  ok: boolean;
  message: string;
  latency_ms: number;
  node_count: number;
  managed_root?: boolean;
  failure_kind?: FailureKind | null;
  detail?: string | null;
}

export type FailureKind = 'host_key_unknown' | 'host_key_changed' | 'auth_failed' | 'connect_failed' | 'login_required' | 'other';

export interface HostKey {
  type: string;
  fingerprint: string;
  line: string;
}

export interface TrustResult { added: number }
export interface PasswordState { stored: boolean }

export interface InstallKeyReq {
  source: string;
  host?: string | null;
  password?: string;
}

export interface InstallKeyResult {
  installed: boolean;
  generated_key: boolean;
  message: string;
}

export interface SourceError {
  source: string;
  message: string;
}

export interface Tree {
  root: Node;
  sources: Source[];
  today: string;
  truncated: boolean;
  truncated_reason?: string | null;
  truncated_by?: 'time' | 'visited' | 'nodes' | null;
  errors: SourceError[];
  pending?: string[];
}

export interface ListDirsReq {
  source: string;
  path: string;
}

export interface DirEntry {
  name: string;
  managed: boolean;
  pruned: boolean;
}

export interface InitChildrenReq extends NodeRef {
  names: string[];
}

export interface NodeRef {
  source: string;
  path: string;
}

export interface RemoveNodeReq {
  source: string;
  path: string;
  version: string;
}

export interface SaveTodoReq {
  source: string;
  path: string;
  version: string;
  items: Item[];
}

export interface SaveMetaReq {
  source: string;
  path: string;
  version: string;
  meta: Meta;
}

export interface Stamp {
  stamp: string;
}

export interface FabricFile { path: string; mtime: number }
export interface FabricListReq { root?: string; home?: string | null }
export interface FabricListRes { records: FabricFile[]; registry: FabricFile[]; events: FabricFile | null; backend?: FabricFile | null }
export interface FabricReadRes { text: string; mtime: number }
export interface FabricWriteReq { path: string; text: string; expected_updated_at: string }
export interface FabricWriteRes { mtime: number }
export interface FabricStampReq { roots: string[]; home?: string | null }

export interface ErrorDto {
  code: number;
  message: string;
  node: Node | null;
}

export class ApiError extends Error {
  code: number;
  node?: Node;
  constructor(code: number, message: string, node?: Node | null) {
    super(message);
    this.name = 'ApiError';
    this.code = code;
    if (node) this.node = node;
  }
  static from(e: unknown, fallbackCode = 500): ApiError {
    if (e instanceof ApiError) return e;
    if (e && typeof e === 'object' && 'message' in e) {
      const dto = e as Partial<ErrorDto>;
      return new ApiError(typeof dto.code === 'number' ? dto.code : fallbackCode, String(dto.message), dto.node ?? null);
    }
    if (typeof e === 'string') {
      try {
        const dto = JSON.parse(e) as Partial<ErrorDto>;
        if (dto && typeof dto.message === 'string') return new ApiError(typeof dto.code === 'number' ? dto.code : fallbackCode, dto.message, dto.node ?? null);
      } catch {}
      return new ApiError(fallbackCode, e);
    }
    return new ApiError(fallbackCode, 'command failed');
  }
}

export function errText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

export interface SetupState { completed_steps: string[]; completed: boolean }
export interface FabricSettings {
  default_folder: string; model: string; telegram_user_id: number | null; auto_mode: boolean;
  question_budget: number; heartbeat_interval_s: number; setup: SetupState;
}
export interface FabricSettingsRead { exists: boolean; settings: FabricSettings }
export interface FabricSecretsWriteReq { anthropic_api_key?: string; telegram_bot_token?: string }
export interface FabricSecretsStatus { anthropic_api_key: boolean; telegram_bot_token: boolean }
export type FabricCheckKind = 'folder' | 'api_key' | 'model' | 'telegram_token';
export interface FabricCheckReq { kind: FabricCheckKind; path?: string; model?: string }
export type FabricCheckResult =
  | { ok: true; reply?: string; bot_name?: string; user_id?: number }
  | { ok: false; reason: string };
