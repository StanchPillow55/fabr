import { describe, expect, it } from 'vitest';
import { backendState } from './backend';

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
