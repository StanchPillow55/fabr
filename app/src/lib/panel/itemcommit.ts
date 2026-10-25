import type { Item } from '../api/types';
import { commit } from '../ui/actions';

export const commitInput = (fn: (el: HTMLInputElement) => void) => (el: HTMLInputElement) => commit(el, () => fn(el)).destroy;

export function titleDecision(old: string, typed: string): { kind: 'none' | 'restore' | 'save'; title: string } {
  const v = typed.trim();
  if (!v) return { kind: 'restore', title: old };
  if (v === old) return { kind: 'none', title: old };
  return { kind: 'save', title: v };
}

export const patchItem = (i: number, p: Partial<Item>) => (all: Item[]): Item[] | null =>
  all[i] ? all.map((x, j) => (j === i ? { ...x, ...p } : x)) : null;
export const removeItem = (i: number) => (all: Item[]): Item[] | null => (all[i] ? all.filter((_, j) => j !== i) : null);
export const appendItem = (row: Item) => (all: Item[]): Item[] => [...all, row];
