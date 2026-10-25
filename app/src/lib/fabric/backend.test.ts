import { describe, expect, it } from 'vitest';
import { backendState } from './backend';
import { FabricStore } from './tasks.svelte';

const now = new Date('2026-10-24T18:00:00Z');

describe('backendState', () => {
  it('is absent without a heartbeat or with an unreadable one', () => {
    expect(backendState(null, now)).toBe('absent');
    expect(backendState('yesterday', now)).toBe('absent');
  });
  it('is connected within three intervals, stale after', () => {
    expect(backendState('2026-10-24T17:59:55Z', now)).toBe('connected');
    expect(backendState('2026-10-24T17:59:30Z', now)).toBe('connected');
    expect(backendState('2026-10-24T17:59:29Z', now)).toBe('stale');
  });
  it('honours the interval and a heartbeat slightly in the future', () => {
    expect(backendState('2026-10-24T17:59:00Z', now, 30)).toBe('connected');
    expect(backendState('2026-10-24T17:58:29Z', now, 30)).toBe('stale');
    expect(backendState('2026-10-24T18:00:02Z', now)).toBe('connected');
  });
});

// Lock the store-level presence semantics (3.2): a fresh _backend.yaml heartbeat reads as
// connected, an absent one as not connected. The store derives `backend`/`backendLabel` from
// the heartbeat it last read and its `now` clock.
describe('FabricStore backend presence semantics (3.2)', () => {
  it('shows not connected when the heartbeat is absent', () => {
    const f = new FabricStore(null);
    f.now = now;
    f.heartbeat = null;
    expect(f.backend).toBe('absent');
    expect(f.backendLabel).toBe('Agent backend not connected');
  });
  it('shows connected when a fresh heartbeat is present', () => {
    const f = new FabricStore(null);
    f.now = now;
    f.heartbeat = '2026-10-24T17:59:55Z';
    expect(f.backend).toBe('connected');
    expect(f.backendLabel).toBe('Agent backend connected');
  });
  it('shows not responding when the heartbeat is stale', () => {
    const f = new FabricStore(null);
    f.now = now;
    f.heartbeat = '2026-10-24T17:59:29Z';
    expect(f.backend).toBe('stale');
    expect(f.backendLabel).toBe('Agent backend not responding');
  });
});
