import type { Answer, Question, Scope, TaskRecord } from './types';

export const SCOPES: readonly Scope[] = ['task', 'project', 'always'];

export interface AnswerInput {
  value?: string | null;
  replacement?: string | null;
  granted?: { field: string; scope: Scope }[] | null;
}

export function isoNow(now: Date = new Date()): string {
  return now.toISOString().replace(/\.\d{3}Z$/, 'Z');
}

function build(q: Question, input: AnswerInput): Pick<Answer, 'value' | 'replacement' | 'granted'> {
  const value = input.value?.trim() ?? '';
  if (q.kind === 'open') {
    if (!value) throw new Error('an open question needs a non-empty answer');
    return { value, replacement: null, granted: null };
  }
  if (q.kind === 'choice') {
    if (!['once', 'always', 'no'].includes(value)) throw new Error('a choice answer must be once, always or no');
    if (value !== 'no') return { value, replacement: null, granted: null };
    const replacement = input.replacement?.trim() ?? '';
    if (!replacement) throw new Error('choosing no needs a non-empty replacement');
    return { value, replacement, granted: null };
  }
  const granted = input.granted ?? [];
  if (!granted.length) throw new Error('a grant answer needs at least one field');
  const seen = new Set<string>();
  for (const g of granted) {
    if (!q.fields?.includes(g.field)) throw new Error(`field "${g.field}" was not requested`);
    if (!SCOPES.includes(g.scope)) throw new Error(`scope "${String(g.scope)}" is not task, project or always`);
    if (seen.has(g.field)) throw new Error(`field "${g.field}" is listed twice`);
    seen.add(g.field);
  }
  return { value: null, replacement: null, granted: granted.map((g) => ({ field: g.field, scope: g.scope })) };
}

export function applyAnswer(record: TaskRecord, input: AnswerInput, by: 'gui' | 'telegram', now: Date = new Date()): TaskRecord {
  const q = record.question;
  if (q === null) throw new Error('this record has no pending question');
  if (q.answer !== null) throw new Error('this question is already answered');
  const at = isoNow(now);
  return { ...record, updated_at: at, question: { ...q, answer: { ...build(q, input), by, at } } };
}

export function describeAnswer(q: Question): string {
  const a = q.answer;
  if (!a) return '';
  if (a.granted) return a.granted.map((g) => `${g.field} (${g.scope})`).join(', ');
  if (a.value === 'no' && a.replacement) return `no, instead: ${a.replacement}`;
  return a.value ?? '';
}
