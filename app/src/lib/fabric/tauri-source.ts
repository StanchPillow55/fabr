import type { Api } from '../api/commands';
import type { CreateFields, FabricSource, RawRecord } from './source';
import { heartbeatOf } from './backend';

type FabricApi = Pick<Api, 'fabricList' | 'fabricRead' | 'fabricWrite' | 'fabricStamp' | 'fabricCreate'>;

export class TauriSource implements FabricSource {
  constructor(private api: FabricApi, private roots: string[], private home: string | null = null) {}

  private async lists() {
    return Promise.all(this.roots.map((root, i) => this.api.fabricList({ root, home: i === 0 ? this.home : null })));
  }

  private async load(paths: string[]): Promise<RawRecord[]> {
    return Promise.all(paths.map(async (path) => ({ path, text: (await this.api.fabricRead(path)).text })));
  }

  async listRecords(): Promise<RawRecord[]> {
    const paths = new Set((await this.lists()).flatMap((l) => l.records.map((f) => f.path)));
    return this.load([...paths].sort());
  }
  async readRecord(path: string): Promise<string> {
    return (await this.api.fabricRead(path)).text;
  }
  async writeRecord(path: string, text: string, expectedUpdatedAt: string): Promise<void> {
    await this.api.fabricWrite({ path, text, expected_updated_at: expectedUpdatedAt });
  }
  async createRecord(folder: string, fields: CreateFields): Promise<{ path: string; id: string }> {
    return this.api.fabricCreate({ folder, ...fields });
  }
  async listRegistry(): Promise<RawRecord[]> {
    if (!this.home) return [];
    const l = await this.api.fabricList({ home: this.home });
    return this.load(l.registry.map((f) => f.path));
  }
  async readEvents(): Promise<string[]> {
    if (!this.home) return [];
    const l = await this.api.fabricList({ home: this.home });
    if (!l.events) return [];
    return (await this.api.fabricRead(l.events.path)).text.split('\n').filter((x) => x.trim() !== '');
  }
  eventsKey(): string {
    return `${this.home ?? ''}/.fabric-space/events.log`;
  }
  async readBackendHeartbeat(): Promise<string | null> {
    if (!this.home) return null;
    try {
      const l = await this.api.fabricList({ home: this.home });
      if (!l.backend) return null;
      return heartbeatOf((await this.api.fabricRead(l.backend.path)).text);
    } catch {
      return null;
    }
  }
  async stamp(): Promise<string> {
    return (await this.api.fabricStamp({ roots: this.roots, home: this.home })).stamp;
  }
}
