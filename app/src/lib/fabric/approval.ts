import { SCOPES, isoNow } from './answer';
import { parseRecord } from './parse';
import { serializeRecord } from './serialize';
import { updatedAtOf, type FabricSource } from './source';
import type { BoundAction, Scope, TaskRecord } from './types';
import { ConflictError } from './write';

export interface ApprovalDecision {
  status: 'approved' | 'refused';
  note?: string | null;
  granted?: { field: string; scope?: Scope }[];
}

/** The persisted `bound_action.effect` labels that are irreversible external
 * effects (send/post/pay). Mirrors the Rust `Effect::{ExternalEffect,Payment}`
 * labels, which gate the second (final) confirmation. */
export const FINAL_CONFIRM_EFFECTS: readonly string[] = ['external_effect', 'payment'];

export function effectNeedsFinalConfirm(effect: string): boolean {
  return FINAL_CONFIRM_EFFECTS.includes(effect);
}

/** The record is parked on the final-confirmation gate: the backend bound an
 * irreversible external effect and recorded a still-pending `final_confirm`. No
 * effect has run; the user must confirm or cancel the exact artifact. */
export function awaitingFinalConfirm(record: TaskRecord): boolean {
  return record.final_confirm?.status === 'pending';
}

/** The bound actions whose effect is the irreversible external one being gated.
 * These carry the exact artifact (params/destination) to render. */
export function finalConfirmActions(record: TaskRecord): BoundAction[] {
  return (record.plan_meta?.bound_actions ?? []).filter((a) => effectNeedsFinalConfirm(a.effect));
}

/** A short verb for the gate heading, derived from the gated effect: "pay" for
 * payment, otherwise "send" (post/send). Falls back to "run" with none. */
export function finalConfirmVerb(record: TaskRecord): 'pay' | 'send' | 'run' {
  const effects = finalConfirmActions(record).map((a) => a.effect);
  if (effects.includes('payment')) return 'pay';
  if (effects.includes('external_effect')) return 'send';
  return 'run';
}

export interface FinalConfirmDecision {
  status: 'approved' | 'refused';
  note?: string | null;
}

export function needsHumanApproval(record: TaskRecord, alwaysAsk: string[] = []): boolean {
  if (record.plan_meta?.reversibility === 'irreversible') return true;
  return record.grants.requested.some((f) => f === 'payment' || f.startsWith('payment.') || alwaysAsk.includes(f));
}

export function applyApproval(record: TaskRecord, decision: ApprovalDecision, by: 'gui' | 'telegram', now: Date = new Date()): TaskRecord {
  if (record.approval === null) throw new Error('this record has no approval to decide');
  if (record.approval.status !== 'pending') throw new Error('this approval is already decided');
  const at = isoNow(now);
  const note = decision.note?.trim() || null;
  const approval = { status: decision.status, by, at, note, digest: record.approval.digest ?? null };
  if (decision.status === 'refused') return { ...record, updated_at: at, approval };
  const seen = new Set<string>();
  const granted = (decision.granted ?? []).map((g) => {
    const scope = g.scope ?? 'task';
    if (!record.grants.requested.includes(g.field)) throw new Error(`field "${g.field}" was not requested`);
    if (!SCOPES.includes(scope)) throw new Error(`scope "${String(scope)}" is not task, project or always`);
    if (seen.has(g.field)) throw new Error(`field "${g.field}" is listed twice`);
    seen.add(g.field);
    return { field: g.field, scope };
  });
  return { ...record, updated_at: at, approval, grants: { ...record.grants, granted } };
}

export async function submitApproval(source: FabricSource, path: string, readText: string, decision: ApprovalDecision, by: 'gui' | 'telegram' = 'gui'): Promise<{ text: string; record: TaskRecord }> {
  const applied = applyApproval(parseRecord(readText), decision, by);
  const expected = updatedAtOf(readText);
  try {
    await source.writeRecord(path, serializeRecord(applied), expected);
  } catch (e) {
    const disk = await source.readRecord(path).catch(() => null);
    if (disk !== null && updatedAtOf(disk) !== expected) throw new ConflictError(path);
    throw e;
  }
  const text = await source.readRecord(path);
  return { text, record: parseRecord(text) };
}

/** Decide the second, final confirmation gate. The existing `digest` (the
 * invocation id binding the gate to the exact artifact) is preserved so the
 * backend only replays this one send/commit/pay, never a different one. */
export function applyFinalConfirm(record: TaskRecord, decision: FinalConfirmDecision, by: 'gui' | 'telegram', now: Date = new Date()): TaskRecord {
  if (record.final_confirm === null) throw new Error('this record has no final confirmation to decide');
  if (record.final_confirm.status !== 'pending') throw new Error('this final confirmation is already decided');
  const at = isoNow(now);
  const note = decision.note?.trim() || null;
  const final_confirm = { status: decision.status, by, at, note, digest: record.final_confirm.digest ?? null };
  return { ...record, updated_at: at, final_confirm };
}

export async function submitFinalConfirm(source: FabricSource, path: string, readText: string, decision: FinalConfirmDecision, by: 'gui' | 'telegram' = 'gui'): Promise<{ text: string; record: TaskRecord }> {
  const applied = applyFinalConfirm(parseRecord(readText), decision, by);
  const expected = updatedAtOf(readText);
  try {
    await source.writeRecord(path, serializeRecord(applied), expected);
  } catch (e) {
    const disk = await source.readRecord(path).catch(() => null);
    if (disk !== null && updatedAtOf(disk) !== expected) throw new ConflictError(path);
    throw e;
  }
  const text = await source.readRecord(path);
  return { text, record: parseRecord(text) };
}
