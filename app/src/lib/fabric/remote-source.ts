import type { Api } from '../api/commands';
import type { FabricRemoteListRes, Source } from '../api/types';
import { heartbeatOf } from './backend';
import type { CreateFields, FabricSource, RawRecord } from './source';

export type RemoteApi = Pick<Api, 'fabricRemoteList' | 'fabricRemoteStamp' | 'fabricRemoteWrite' | 'fabricRemoteCreate'>;

export class RemoteSource implements FabricSource {
  constructor(private api: RemoteApi | Promise<RemoteApi>, readonly source: Source) {}

  async snapshot(): Promise<FabricRemoteListRes> {
    return (await this.api).fabricRemoteList(this.source);
  }

  async listRecords(): Promise<RawRecord[]> {
    return (await this.snapshot()).records;
  }
  async readRecord(path: string): Promise<string> {
    const r = (await this.snapshot()).records.find((x) => x.path === path);
    if (!r) throw new Error(`no record at ${path}`);
    return r.text;
  }
  async writeRecord(path: string, text: string, expectedUpdatedAt: string): Promise<void> {
    await (await this.api).fabricRemoteWrite({ source: this.source, path, text, expected_updated_at: expectedUpdatedAt });
  }
  async createRecord(folder: string, fields: CreateFields): Promise<{ path: string; id: string }> {
    const { path } = await (await this.api).fabricRemoteCreate({ source: this.source, folder, ...fields });
    return { path, id: (path.split('/').pop() ?? path).replace(/\.md$/, '') };
  }
  async listRegistry(): Promise<RawRecord[]> {
    return (await this.snapshot()).registry.map((r) => ({ path: r.name, text: r.text }));
  }
  async readEvents(): Promise<string[]> {
    return (await this.snapshot()).events.split('\n').filter((l) => l.trim() !== '');
  }
  eventsKey(): string {
    return `remote:${this.source.id}`;
  }
  async readBackendHeartbeat(): Promise<string | null> {
    const b = (await this.snapshot()).backend;
    return b ? heartbeatOf(b) : null;
  }
  async stamp(): Promise<string> {
    return (await (await this.api).fabricRemoteStamp(this.source)).stamp;
  }
}
