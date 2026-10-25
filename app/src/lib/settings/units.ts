import type { Settings } from '../api/types';

export type AdvField = 'poll' | 'remote' | 'rescan' | 'depth' | 'maxNodes';
export type AdvancedText = Record<AdvField, string>;

export const msToSec = (ms: number): number => ms / 1000;
export const secToMs = (s: number): number => Math.round(s * 1000);
export const msToMin = (ms: number): number => ms / 60000;
export const minToMs = (m: number): number => Math.round(m * 60000);

export const clamp = (n: number, lo: number, hi: number): number => Math.min(hi, Math.max(lo, n));

export const fmt = (n: number): string => String(Number(n.toFixed(3)));

export function parseNumber(text: string): number | null {
  const t = text.trim();
  if (!t) return null;
  const n = Number(t);
  return Number.isFinite(n) ? n : null;
}

export const DEFAULTS = { poll_ms: 2000, remote_poll_ms: 30000, full_rescan_ms: 300000, search_depth: 4, max_nodes: 10000 };

export function advancedToText(s: Settings): AdvancedText {
  return {
    poll: fmt(msToSec(s.poll_ms ?? DEFAULTS.poll_ms)),
    remote: fmt(msToSec(s.remote_poll_ms ?? DEFAULTS.remote_poll_ms)),
    rescan: fmt(msToMin(s.full_rescan_ms ?? DEFAULTS.full_rescan_ms)),
    depth: String(s.search_depth ?? DEFAULTS.search_depth),
    maxNodes: String(s.max_nodes ?? DEFAULTS.max_nodes),
  };
}

export interface AdvancedCheck {
  values: Partial<Pick<Settings, 'poll_ms' | 'remote_poll_ms' | 'full_rescan_ms' | 'search_depth' | 'max_nodes'>>;
  errors: Partial<Record<AdvField, string>>;
}

export function validateAdvanced(t: AdvancedText): AdvancedCheck {
  const values: AdvancedCheck['values'] = {};
  const errors: AdvancedCheck['errors'] = {};
  const seconds = (f: 'poll' | 'remote', key: 'poll_ms' | 'remote_poll_ms', minMs: number, msg: string) => {
    const n = parseNumber(t[f]);
    const ms = n === null ? NaN : secToMs(n);
    if (!(ms >= minMs)) errors[f] = msg;
    else values[key] = ms;
  };
  seconds('poll', 'poll_ms', 250, 'Must be a number of seconds, at least 0.25.');
  seconds('remote', 'remote_poll_ms', 1000, 'Must be a number of seconds, at least 1.');
  const r = parseNumber(t.rescan);
  const rms = r === null ? NaN : minToMs(r);
  if (!(rms >= 1000)) errors.rescan = 'Must be a number of minutes, at least 1 second (0.02).';
  else values.full_rescan_ms = rms;
  const whole = (f: 'depth' | 'maxNodes', key: 'search_depth' | 'max_nodes', lo: number, hi: number) => {
    const n = parseNumber(t[f]);
    if (n === null || !Number.isInteger(n) || n < lo || n > hi) errors[f] = `Must be a whole number from ${lo} to ${hi}.`;
    else values[key] = n;
  };
  whole('depth', 'search_depth', 1, 12);
  whole('maxNodes', 'max_nodes', 100, 100000);
  return { values, errors };
}
