import type { Source, SourceError, TestResult } from '../api/types';
import { slugify } from '../addnodes';

export type SourceStatus = 'ok' | 'error' | 'untested';

export function sourceStatus(id: string, test: TestResult | null, errors: readonly SourceError[], loaded: readonly string[]): SourceStatus {
  if (test) return test.ok ? 'ok' : 'error';
  if (errors.some((e) => e.source === id)) return 'error';
  return loaded.includes(id) ? 'ok' : 'untested';
}

export const parsePrune = (text: string): string[] | null => {
  const v = text.split(/[\s,]+/).filter(Boolean);
  return v.length ? v : null;
};

export function parseDepth(text: string): { value: number | null; error?: string } {
  const t = text.trim();
  if (!t) return { value: null };
  const n = Number(t);
  if (!Number.isInteger(n) || n < 1 || n > 12) return { value: null, error: 'Must be a whole number from 1 to 12, or blank.' };
  return { value: n };
}

export function uniqueId(base: string, taken: Iterable<string>): string {
  const used = new Set(taken);
  const b = slugify(base) || 'source';
  if (!used.has(b)) return b;
  let n = 2;
  while (used.has(`${b}-${n}`)) n++;
  return `${b}-${n}`;
}

export function suggestId(src: Pick<Source, 'name' | 'host' | 'root'>, taken: Iterable<string>): string {
  const seg = src.root.trim().replace(/[\\/]+$/, '').split(/[\\/]/).pop() ?? '';
  return uniqueId(src.name.trim() || src.host?.trim() || seg || 'source', taken);
}

export function buildSource(src: Source, pruneText: string, depthText: string): Source {
  const out: Source = {
    id: src.id,
    name: src.name.trim() || src.id,
    host: src.host?.trim() || null,
    root: src.root.trim(),
    prune: parsePrune(pruneText),
  };
  const d = parseDepth(depthText);
  if (d.value !== null) out.search_depth = d.value;
  return out;
}

export const rowError = (label: string, message: string): string => `“${label}”: ${message}`;

export interface DraftErrors { id?: string; root?: string; depth?: string }

export function validateDraft(src: Source, depthText: string, otherIds: readonly string[]): DraftErrors {
  const e: DraftErrors = {};
  if (!/^[a-z0-9_-]+$/.test(src.id)) e.id = 'The id may only use lowercase letters, digits, _ and -.';
  else if (otherIds.includes(src.id)) e.id = `Another source already uses the id “${src.id}”.`;
  if (!src.root.trim()) e.root = 'A root folder is required.';
  const d = parseDepth(depthText);
  if (d.error) e.depth = d.error;
  return e;
}

export const hasErrors = (e: DraftErrors): boolean => Object.keys(e).length > 0;
export const firstError = (e: DraftErrors): string | null => e.id ?? e.root ?? e.depth ?? null;

export const isUntouched = (src: Source, pruneText: string, depthText: string): boolean =>
  !src.name.trim() && !src.host?.trim() && !src.root.trim() && !pruneText.trim() && !depthText.trim();
