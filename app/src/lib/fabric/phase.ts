import { clockMinute } from '../model';
import type { TaskRecord } from './types';

export const PHASES = ['todo', 'working', 'needs_harness', 'needs_you', 'paused', 'suspended', 'done', 'failed'] as const;
export type Phase = (typeof PHASES)[number];

export const PHASE_LABEL: Record<Phase, string> = {
  todo: 'todo', working: 'working', needs_harness: 'needs harness', needs_you: 'needs you', paused: 'paused', suspended: 'suspended', done: 'done', failed: 'failed',
};

export const isOpen = (p: Phase): boolean => p !== 'done' && p !== 'failed';
export const canShelve = (p: Phase): boolean => p !== 'done';
export const isActive = (p: Phase): boolean => p === 'working' || p === 'needs_harness' || p === 'needs_you' || p === 'paused';

type PhaseInput = Pick<TaskRecord, 'state' | 'suspended' | 'suspend_until' | 'paused' | 'waiting' | 'needs' | 'question' | 'approval'>;

const minute = (now: Date | string): string => (now instanceof Date ? clockMinute(now) : now.length > 16 ? clockMinute(new Date(now)) : now);

export function phaseOf(r: PhaseInput, now: Date | string): Phase {
  if (r.state === 'succeeded') return 'done';
  if (r.suspended && (!r.suspend_until || r.suspend_until > minute(now))) return 'suspended';
  if (r.state === 'failed') return 'failed';
  if (r.state === 'reminder') return 'todo';
  if (r.paused) return 'paused';
  if (r.needs != null && r.needs !== '') return 'needs_harness';
  if ((r.question && !r.question.answer && r.waiting) || (r.state === 'defined' && (!r.approval || r.approval.status === 'pending'))) return 'needs_you';
  return 'working';
}
