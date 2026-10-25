import type { Api } from '../api/index';
import { errText, type FailureKind, type HostKey, type InstallKeyResult, type Source, type TestResult } from '../api/types';

export type Phase = 'idle' | 'testing' | 'ok' | 'host_unknown' | 'host_changed' | 'auth_failed' | 'login_required' | 'failed';

export class SshFlow {
  password = $state('');
  stored = $state(false);
  testing = $state(false);
  result = $state<TestResult | null>(null);
  error = $state<string | null>(null);
  hostKeys = $state<HostKey[] | null>(null);
  hostKeyError = $state<string | null>(null);
  trusting = $state(false);
  focusTick = $state(0);
  usedPassword = $state(false);
  installing = $state(false);
  install = $state<InstallKeyResult | null>(null);
  installError = $state<string | null>(null);
  installHost = $state<string | null>(null);

  private api: Api;
  private source: () => Source;
  private onTrusted: () => void | Promise<void>;

  constructor(api: Api, source: () => Source, onTrusted: () => void | Promise<void> = () => {}) {
    this.api = api;
    this.source = source;
    this.onTrusted = onTrusted;
  }

  clearResult(): void {
    this.result = null;
    this.error = null;
    this.hostKeys = null;
    this.hostKeyError = null;
  }

  get kind(): FailureKind | null {
    return this.result?.failure_kind ?? null;
  }

  get phase(): Phase {
    if (this.testing) return 'testing';
    if (!this.result) return this.error ? 'failed' : 'idle';
    if (this.result.ok) return 'ok';
    switch (this.kind) {
      case 'host_key_unknown': return 'host_unknown';
      case 'host_key_changed': return 'host_changed';
      case 'auth_failed': return 'auth_failed';
      case 'login_required': return 'login_required';
      default: return 'failed';
    }
  }

  get isSsh(): boolean {
    return !!this.source().host;
  }

  get installedHere(): boolean {
    return !!this.install?.installed && this.installHost === this.source().host;
  }

  get canInstall(): boolean {
    if (!this.isSsh || this.installedHere) return false;
    return (this.result?.ok === true && this.usedPassword) || (this.stored && !this.result);
  }

  async loadStored(): Promise<void> {
    const s = this.source();
    if (!s.host) return;
    try {
      this.stored = (await this.api.hasSourcePassword(s.id)).stored;
    } catch {
      this.stored = false;
    }
  }

  async test(): Promise<void> {
    const s = $state.snapshot(this.source()) as Source;
    const pw = this.password;
    this.testing = true;
    this.error = null;
    this.result = null;
    this.hostKeys = null;
    this.hostKeyError = null;
    await this.#try(async () => {
      const req: Source = { ...s };
      if (s.host && pw) req.password = pw;
      else delete req.password;
      const r = await this.api.testSource(req);
      this.result = r;
      if (r.ok) this.usedPassword = !!s.host && (!!pw || this.stored);
      if (this.installHost !== s.host) {
        this.install = null;
        this.installError = null;
      }
      if (r.failure_kind === 'auth_failed') this.focusTick++;
      if (r.failure_kind === 'host_key_unknown' && s.host) await this.fetchHostKeys(s.host);
    }, 'error');
    this.testing = false;
  }

  private async fetchHostKeys(host: string): Promise<void> {
    await this.#try(async () => {
      this.hostKeys = await this.api.hostKey(host);
    }, 'hostKeyError');
  }

  async trust(): Promise<void> {
    const host = this.source().host;
    if (!host || !this.hostKeys?.length) return;
    this.trusting = true;
    this.hostKeyError = null;
    try {
      await this.api.trustHost(host, this.hostKeys.map((k) => k.line));
    } catch (e) {
      this.trusting = false;
      if (e && typeof e === 'object' && 'code' in e && (e as { code: number }).code === 409) {
        this.result = { ok: false, message: `Host key changed: ${host} presents a different key than the one in your known_hosts`, latency_ms: 0, node_count: 0, failure_kind: 'host_key_changed', detail: errText(e) };
        this.hostKeys = null;
      } else {
        this.hostKeyError = errText(e);
      }
      return;
    }
    this.trusting = false;
    this.clearResult();
    await this.#try(() => this.onTrusted());
    await this.test();
  }

  async savePassword(): Promise<void> {
    const s = this.source();
    if (!s.host || !this.password) return;
    const r = await this.api.setSourcePassword(s.id, this.password);
    this.stored = r.stored;
    this.password = '';
  }

  async clearPassword(): Promise<void> {
    this.password = '';
    if (!this.stored) return;
    await this.#try(async () => {
      this.stored = (await this.api.setSourcePassword(this.source().id, '')).stored;
    }, 'error');
  }

  async installKey(): Promise<void> {
    const { id, host } = this.source();
    if (!host) return;
    this.installing = true;
    this.installError = null;
    this.install = null;
    await this.#try(async () => {
      this.install = await this.api.installKey({ source: id, host, password: this.password || undefined });
      this.installHost = host;
    }, 'installError');
    this.installing = false;
  }

  async #try(run: () => unknown, fail?: 'error' | 'hostKeyError' | 'installError'): Promise<void> {
    try {
      await run();
    } catch (e) {
      if (fail) this[fail] = errText(e);
    }
  }
}
