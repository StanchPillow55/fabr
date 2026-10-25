import { describe, expect, it } from 'vitest';
import { buildApi } from '../api/commands';
import { FixtureSource, updatedAtOf } from './source';
import { TauriSource } from './tauri-source';

describe('FixtureSource data path', () => {
  it('refuses a stale expectedUpdatedAt and accepts the current one', async () => {
    const s = new FixtureSource();
    const [r] = await s.listRecords();
    const cur = updatedAtOf(r.text);
    expect(cur).not.toBe('');
    const next = r.text.replace(`updated_at: ${cur}`, 'updated_at: 2030-01-01T00:00:00Z');
    await expect(s.writeRecord(r.path, next, 'nope')).rejects.toThrow(/updated_at/);
    expect(await s.readRecord(r.path)).toBe(r.text);
    expect(await s.stamp()).toBe('0');
    await s.writeRecord(r.path, next, cur);
    expect(await s.readRecord(r.path)).toBe(next);
    expect(await s.stamp()).toBe('1');
    await expect(s.writeRecord(r.path, next, cur)).rejects.toThrow();
  });

  it('lists the registry of fixtures 05, 06 and 10', async () => {
    const reg = await new FixtureSource().listRegistry();
    const by = (n: string) => reg.filter((r) => r.path.includes(`/${n}`));
    expect(by('10-registry').map((r) => r.path.split('/').pop())).toEqual([
      '2026-10-24-file-tax-receipts--c34ec935.yaml',
      '2026-10-24-order-filament--ec664532.yaml',
      '2026-10-24-rename-scans--4d789216.yaml',
    ]);
    expect(by('05-running')).toHaveLength(1);
    expect(by('06-waiting-mid-run')).toHaveLength(1);
    expect(reg).toHaveLength(6);
    expect(reg[0].text).toContain('task_path:');
  });

  it('reads the event lines', async () => {
    const lines = await new FixtureSource().readEvents();
    expect(lines.length).toBeGreaterThan(0);
    for (const l of lines) expect(l).toMatch(/^\d{4}-\d\d-\d\dT\S+Z\s+(waiting|done|failed|stuck)\s+\/.+\.md/);
  });
});

describe('TauriSource request shaping', () => {
  it('sends the four commands with snake_case bodies', async () => {
    const calls: [string, boolean, unknown][] = [];
    const api = buildApi(async (name, get, req) => {
      calls.push([name, get, req]);
      if (name === 'fabric_list') return { records: [{ path: '/p/.fabric-space/tasks/a.md', mtime: 1 }], registry: [], events: null };
      if (name === 'fabric_read') return { text: 'T', mtime: 1 };
      if (name === 'fabric_write') return { mtime: 2 };
      return { stamp: 'abc' };
    });
    const s = new TauriSource(api, ['/p'], '/h');
    expect(await s.listRecords()).toEqual([{ path: '/p/.fabric-space/tasks/a.md', text: 'T' }]);
    await s.writeRecord('/p/.fabric-space/tasks/a.md', 'X', 'U');
    expect(await s.stamp()).toBe('abc');
    expect(await s.readEvents()).toEqual([]);
    expect(calls.filter(([, g]) => g)).toEqual([]);
    expect(calls[0]).toEqual(['fabric_list', false, { root: '/p', home: '/h' }]);
    expect(calls[1]).toEqual(['fabric_read', false, { path: '/p/.fabric-space/tasks/a.md' }]);
    expect(calls[2]).toEqual(['fabric_write', false, { path: '/p/.fabric-space/tasks/a.md', text: 'X', expected_updated_at: 'U' }]);
    expect(calls[3]).toEqual(['fabric_stamp', false, { roots: ['/p'], home: '/h' }]);
  });

  it('lists registry, events and heartbeat without a root', async () => {
    const calls: [string, unknown][] = [];
    const api = buildApi(async (name, _get, req) => {
      calls.push([name, req]);
      if (name === 'fabric_list') return { records: [], registry: [], events: { path: '/h/.fabric-space/events.log', mtime: 1 }, backend: { path: '/h/.fabric-space/registry/_backend.yaml', mtime: 1 } };
      return { text: req && (req as { path: string }).path.endsWith('.yaml') ? 'heartbeat_at: 2026-10-24T18:00:00Z\n' : 'a\n\nb\n', mtime: 1 };
    });
    const s = new TauriSource(api, ['/p'], '/h');
    expect(await s.listRegistry()).toEqual([]);
    expect(await s.readEvents()).toEqual(['a', 'b']);
    expect(await s.readBackendHeartbeat()).toBe('2026-10-24T18:00:00Z');
    expect(s.eventsKey()).toBe('/h/.fabric-space/events.log');
    for (const [name, req] of calls.filter(([n]) => n === 'fabric_list')) {
      expect(name).toBe('fabric_list');
      expect(req).toEqual({ home: '/h' });
    }
  });

  it('reports no heartbeat when the file is absent or unreadable', async () => {
    const none = new TauriSource(buildApi(async () => ({ records: [], registry: [], events: null, backend: null })), ['/p'], '/h');
    expect(await none.readBackendHeartbeat()).toBeNull();
    const bad = new TauriSource(buildApi(async (name) => (name === 'fabric_list' ? { records: [], registry: [], events: null, backend: { path: '/h/.fabric-space/registry/_backend.yaml', mtime: 1 } } : { text: 'heartbeat_at: soon\n', mtime: 1 })), ['/p'], '/h');
    expect(await bad.readBackendHeartbeat()).toBeNull();
    expect(await new FixtureSource().readBackendHeartbeat()).toBeNull();
  });
});

describe('agent commands request shaping', () => {
  it('sends the two agent commands as empty posts', async () => {
    const calls: [string, boolean, unknown][] = [];
    const api = buildApi(async (name, get, req) => {
      calls.push([name, get, req]);
      return { running: false, reason: 'Setup is not complete' };
    });
    expect(await api.fabricAgentStatus()).toEqual({ running: false, reason: 'Setup is not complete' });
    await api.fabricAgentStart();
    expect(calls).toEqual([
      ['fabric_agent_status', false, undefined],
      ['fabric_agent_start', false, undefined],
    ]);
  });
});
