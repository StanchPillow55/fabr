import { isoNow } from './answer';
import { submitEdit } from './edit';
import type { Phase } from './phase';
import type { FabricSource } from './source';
import { STEPS, type Assertion, type Redo, type TaskRecord } from './types';

export type Section = 'context' | 'spec' | 'success' | 'plan';
export const SECTION_KEYS: readonly Section[] = ['context', 'spec', 'success', 'plan'];
export const FROZEN_MESSAGE = '400: task is frozen once execution starts';
export const REDO_PENDING = 'redo requested';
export const REDO_LABEL: Record<Redo, string> = { define: 'Redo define', plan: 'Redo plan' };

export const isEditable = (r: Pick<TaskRecord, 'state'>): boolean => r.state === 'reminder' || r.state === 'task' || r.state === 'defined';

export const provenance = (r: TaskRecord, s: Section): 'you' | 'agent' | null => (r.by[s] === 'human' ? 'you' : r.by[s] === 'agent' ? 'agent' : null);

export function editStart(r: TaskRecord, phase: Phase): 'open' | 'pause' | 'frozen' {
  if (!isEditable(r)) return 'frozen';
  return phase === 'working' && r.state !== 'reminder' ? 'pause' : 'open';
}

function pastDefine(r: TaskRecord): boolean {
  return r.state === 'defined' || (r.state === 'task' && r.step !== null && STEPS.indexOf(r.step) > STEPS.indexOf('define'));
}

function guard(r: TaskRecord): void {
  if (!isEditable(r)) throw new Error(FROZEN_MESSAGE);
}

function touched(r: TaskRecord, section: Section | 'assertions', now: Date): TaskRecord {
  const by = section === 'assertions' ? { ...r.by, success: 'human' as const } : { ...r.by, [section]: 'human' as const };
  const redo = (section === 'spec' || section === 'success' || section === 'assertions') && pastDefine(r) ? 'plan' : r.requests.redo;
  return { ...r, by, requests: { ...r.requests, redo }, updated_at: isoNow(now) };
}

export function applySectionEdit(record: TaskRecord, section: Section, text: string, now: Date = new Date()): TaskRecord {
  guard(record);
  const next = touched(record, section, now);
  return { ...next, sections: { ...next.sections, [section]: text.trim() } };
}

export function assertionsFromLines(text: string, prior: Assertion[]): Assertion[] {
  const lines = text.split('\n').map((l) => l.trim()).filter(Boolean);
  const used = new Set<string>();
  const out: Assertion[] = lines.map((line, i) => {
    const kept = prior.find((a) => a.text === line && !used.has(a.id)) ?? (prior[i] && !used.has(prior[i].id) ? prior[i] : null);
    if (kept) used.add(kept.id);
    return { id: kept?.id ?? '', text: line };
  });
  let n = 1;
  return out.map((a) => {
    if (a.id) return a;
    while (used.has(`a${n}`)) n++;
    used.add(`a${n}`);
    return { id: `a${n}`, text: a.text };
  });
}

export function applyAssertionsEdit(record: TaskRecord, text: string, now: Date = new Date()): TaskRecord {
  guard(record);
  return { ...touched(record, 'assertions', now), assertions: assertionsFromLines(text, record.assertions) };
}

export interface FieldsEdit { title?: string; due?: string | null; pri?: number }

export function applyFieldsEdit(record: TaskRecord, f: FieldsEdit, now: Date = new Date()): TaskRecord {
  guard(record);
  if (f.title !== undefined && !f.title.trim()) throw new Error('a task needs a title');
  if (f.pri !== undefined && !(Number.isInteger(f.pri) && f.pri >= 0 && f.pri <= 3)) throw new Error(`bad priority: ${f.pri}`);
  return { ...record, title: f.title?.trim() ?? record.title, due: f.due === undefined ? record.due : f.due || null, pri: f.pri ?? record.pri, updated_at: isoNow(now) };
}

export function applyRedo(record: TaskRecord, which: Redo, now: Date = new Date()): TaskRecord {
  guard(record);
  if (record.state === 'reminder') throw new Error(`redo ${which} does not apply to a reminder`);
  if (record.requests.redo) throw new Error('redo is already requested');
  return { ...record, requests: { ...record.requests, redo: which }, updated_at: isoNow(now) };
}

export const submitSection = (source: FabricSource, path: string, readText: string, section: Section, text: string) =>
  submitEdit(source, path, readText, (r) => applySectionEdit(r, section, text));

export const submitAssertions = (source: FabricSource, path: string, readText: string, text: string) =>
  submitEdit(source, path, readText, (r) => applyAssertionsEdit(r, text));

export const submitFields = (source: FabricSource, path: string, readText: string, f: FieldsEdit) =>
  submitEdit(source, path, readText, (r) => applyFieldsEdit(r, f));

export const submitRedo = (source: FabricSource, path: string, readText: string, which: Redo) =>
  submitEdit(source, path, readText, (r) => applyRedo(r, which));
