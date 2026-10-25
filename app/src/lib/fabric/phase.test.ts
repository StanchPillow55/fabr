import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { PHASES, isActive, isOpen, phaseOf, type Phase } from './phase';
import type { FrontMatter, TaskRecord } from './types';

const base = (over: Partial<TaskRecord>): TaskRecord => ({
  state: 'in_progress', suspended: false, suspend_until: null, paused: false, waiting: false, question: null, approval: null, ...over,
}) as TaskRecord;

describe('phaseOf', () => {
  it('follows fixtures/phase-table.json', () => {
    const rows = JSON.parse(readFileSync(new URL('../../../../fixtures/phase-table.json', import.meta.url), 'utf8')) as { name: string; front_matter: FrontMatter; now: string; phase: Phase }[];
    expect(rows.length).toBeGreaterThan(0);
    for (const row of rows) {
      const record = { ...row.front_matter, sections: {} } as unknown as TaskRecord;
      expect([row.name, phaseOf(record, row.now)]).toEqual([row.name, row.phase]);
    }
  });

  it('puts finished states before suspension', () => {
    expect(phaseOf(base({ state: 'succeeded', suspended: true }), '2026-10-25T08:00')).toBe('done');
    expect(phaseOf(base({ state: 'failed', paused: true }), '2026-10-25T08:00')).toBe('failed');
  });

  it('suspension ends at its until time and beats paused and needs you', () => {
    const r = base({ suspended: true, paused: true, suspend_until: '2026-10-25T09:00' });
    expect(phaseOf(r, '2026-10-25T08:00')).toBe('suspended');
    expect(phaseOf(r, '2026-10-25T09:00')).toBe('paused');
    expect(phaseOf(base({ state: 'reminder', suspended: true }), '2026-10-25T08:00')).toBe('suspended');
  });

  it('asks for you on a pending approval or an open question', () => {
    expect(phaseOf(base({ state: 'defined', approval: null }), '2026-10-25T08:00')).toBe('needs_you');
    expect(phaseOf(base({ state: 'defined', approval: { status: 'approved' } }), '2026-10-25T08:00')).toBe('working');
    expect(phaseOf(base({ waiting: true, question: { answer: null } as TaskRecord['question'] }), '2026-10-25T08:00')).toBe('needs_you');
  });

  it('puts needs_harness after paused and before needs you', () => {
    const at = '2026-10-25T08:00';
    expect(phaseOf(base({ needs: 'harness: write_file', state: 'defined', approval: null } as Partial<TaskRecord>), at)).toBe('needs_harness');
    expect(phaseOf(base({ needs: 'harness: write_file', paused: true } as Partial<TaskRecord>), at)).toBe('paused');
    expect(phaseOf(base({ needs: 'harness: write_file', suspended: true } as Partial<TaskRecord>), at)).toBe('suspended');
    expect(phaseOf(base({ needs: null, state: 'defined', approval: null } as Partial<TaskRecord>), at)).toBe('needs_you');
  });

  it('classifies the groups', () => {
    expect(PHASES.filter(isOpen)).toEqual(['todo', 'working', 'needs_harness', 'needs_you', 'paused', 'suspended']);
    expect(PHASES.filter(isActive)).toEqual(['working', 'needs_harness', 'needs_you', 'paused']);
  });
});
