import { describe, expect, it } from 'vitest';
import { advancedToText, clamp, fmt, minToMs, msToMin, msToSec, parseNumber, secToMs, validateAdvanced } from './units';

describe('unit conversion', () => {
  it('ms <-> s and min', () => {
    expect(msToSec(2000)).toBe(2);
    expect(secToMs(0.25)).toBe(250);
    expect(msToMin(300000)).toBe(5);
    expect(minToMs(0.5)).toBe(30000);
    expect(secToMs(0.1 + 0.2)).toBe(300);
  });
  it('round-trips through the text shown', () => {
    for (const ms of [250, 500, 1500, 2000, 30000, 123456]) expect(secToMs(Number(fmt(msToSec(ms))))).toBe(ms);
    for (const ms of [30000, 300000, 90000, 600000]) expect(minToMs(Number(fmt(msToMin(ms))))).toBe(ms);
    expect(fmt(2)).toBe('2');
    expect(fmt(0.25)).toBe('0.25');
  });
  it('clamp and parse', () => {
    expect(clamp(0, 1, 12)).toBe(1);
    expect(clamp(99, 1, 12)).toBe(12);
    expect(clamp(5, 1, 12)).toBe(5);
    expect(parseNumber(' 2.5 ')).toBe(2.5);
    expect(parseNumber('')).toBeNull();
    expect(parseNumber('abc')).toBeNull();
  });
});

describe('validateAdvanced', () => {
  const ok = { poll: '2', remote: '30', rescan: '5', depth: '4', maxNodes: '10000' };
  it('converts valid fields to API units', () => {
    const r = validateAdvanced({ ...ok, poll: '0.5', rescan: '10' });
    expect(r.errors).toEqual({});
    expect(r.values).toEqual({ poll_ms: 500, remote_poll_ms: 30000, full_rescan_ms: 600000, search_depth: 4, max_nodes: 10000 });
  });
  it('enforces the API ranges', () => {
    const r = validateAdvanced({ poll: '0.1', remote: '0.5', rescan: '0', depth: '13', maxNodes: '99' });
    expect(Object.keys(r.errors).sort()).toEqual(['depth', 'maxNodes', 'poll', 'remote', 'rescan']);
    expect(r.values).toEqual({});
    expect(validateAdvanced({ ...ok, poll: '0.25', remote: '1', depth: '12', maxNodes: '100000' }).errors).toEqual({});
    expect(validateAdvanced({ ...ok, depth: '1.5' }).errors.depth).toBeTruthy();
    expect(validateAdvanced({ ...ok, poll: '' }).errors.poll).toBeTruthy();
  });
  it('displays settings in seconds / minutes', () => {
    expect(advancedToText({ poll_ms: 2000, remote_poll_ms: 30000, full_rescan_ms: 300000, search_depth: 4, max_nodes: 10000 }))
      .toEqual({ poll: '2', remote: '30', rescan: '5', depth: '4', maxNodes: '10000' });
    expect(advancedToText({ poll_ms: 250, remote_poll_ms: 1000 }).poll).toBe('0.25');
  });
});
