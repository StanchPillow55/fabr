/**
 * Task 14.1 — frontend-to-backend happy-path E2E (vitest integration form).
 *
 * WHY THIS IS A VITEST INTEGRATION TEST AND NOT A GUI E2E:
 * The app (`app/`) ships vitest only (`npm test` -> `vitest run`). There is no
 * browser/Tauri GUI E2E harness here (no Playwright, WebdriverIO, or
 * tauri-driver), and standing one up was explicitly deferred — see task 14.1's
 * note: "this needs the app harness and may require user action to run locally".
 * So this test drives the frontend fabric CLIENT surface (`source.ts`,
 * `requests.ts`, `answer.ts`, `approval.ts`, `backend.ts`) through the full
 * human-in-the-loop lifecycle against the in-memory `FixtureSource`, exactly as
 * the real Svelte UI calls these helpers (delegate button -> `submitRequest`,
 * answer panel -> `applyAnswer`/`writeRecord`, approve button -> `submitApproval`,
 * connected indicator -> `backendState`). That proves the client-side half of
 * the in-process `call` surface flow — delegate -> spec/plan -> approve ->
 * verified result — plus the heartbeat/backend-connected reporting that the
 * worker thread feeds. A true end-to-end run through a live Tauri window and the
 * real worker thread still requires the app GUI harness and is user-run.
 *
 * Lifecycle stages / requirements exercised:
 *   - delegate a node Todo             -> Req 4.1       (01-reminder, submitRequest 'delegate')
 *   - question gate (pause + answer)   -> Req 5.1, 5.3  (02-open-question, applyAnswer round-trip)
 *   - approval gate (no auto-approve)  -> Req 5.2, 5.4  (04-awaiting-approval, submitApproval)
 *   - verified result                  -> Req 5.1/5.4   (07-succeeded, verdict pass)
 *   - backend-connected heartbeat      -> Req 9.1       (backendState over the heartbeat the worker writes)
 */
import { describe, expect, it } from 'vitest';
import { applyAnswer } from './answer';
import { backendState } from './backend';
import { needsHumanApproval, submitApproval } from './approval';
import { parseRecord } from './parse';
import { submitRequest } from './requests';
import { serializeRecord } from './serialize';
import { FixtureSource, updatedAtOf } from './source';

const now = new Date('2026-10-24T19:00:00.500Z');

function fresh() {
  return new FixtureSource();
}

async function find(src: FixtureSource, fragment: string) {
  const match = (await src.listRecords()).find((r) => r.path.includes(fragment));
  if (!match) throw new Error(`no fixture record matching "${fragment}"`);
  return match;
}

describe('happy-path E2E: delegate -> question -> approve -> verified result', () => {
  it('step 1 (Req 4.1): the delegate button records a delegate request on a reminder node Todo', async () => {
    const src = fresh();
    const todo = await find(src, '01-reminder');
    expect(parseRecord(todo.text).state).toBe('reminder');
    expect(parseRecord(todo.text).requests.delegate).toBe(false);

    const { record } = await submitRequest(src, todo.path, todo.text, 'delegate');

    // The delegate flag is the exact handoff the worker thread picks up to turn
    // the node Todo into a driven record.
    expect(record.requests.delegate).toBe(true);
    expect(parseRecord(await src.readRecord(todo.path)).requests.delegate).toBe(true);
  });

  it('step 2 (Req 5.1, 5.3): the worker parks at a question and only resumes once the UI records an answer', async () => {
    const src = fresh();
    const open = await find(src, '02-open-question');
    const before = parseRecord(open.text);

    // The pipeline has produced a spec step and is parked waiting on the human:
    // an unanswered open question, with no answer yet (Req 5.1 — stays parked).
    expect(before.step).toBe('define');
    expect(before.question?.kind).toBe('open');
    expect(before.question?.answer).toBeNull();

    // The UI answer panel folds the human reply back into the record (Req 5.3).
    const answered = applyAnswer(before, { value: ' Prusament ' }, 'gui', now);
    await src.writeRecord(open.path, serializeRecord(answered), updatedAtOf(open.text));

    const persisted = parseRecord(await src.readRecord(open.path));
    expect(persisted.question?.answer).toMatchObject({ value: 'Prusament', by: 'gui' });
    // Answering never advances state client-side — the worker does that next cycle.
    expect(persisted.state).toBe(before.state);
  });

  it('step 3 (Req 5.2, 5.4): the approval gate needs a human and does not auto-approve', async () => {
    const src = fresh();
    const awaiting = await find(src, '04-awaiting');
    const before = parseRecord(awaiting.text);

    // Parked at the approval gate: a pending approval on an irreversible plan
    // (Req 5.2 — the UI must not proceed on its own).
    expect(before.approval?.status).toBe('pending');
    expect(needsHumanApproval(before)).toBe(true);

    // The approve button grants the requested fields and flips status to approved
    // (Req 5.4 — the resume path), writing through the same `call`/source surface.
    const { record } = await submitApproval(src, awaiting.path, awaiting.text, {
      status: 'approved',
      granted: [{ field: 'name.first', scope: 'task' }],
    });

    expect(record.approval?.status).toBe('approved');
    expect(record.grants.granted).toEqual([{ field: 'name.first', scope: 'task' }]);
    // The on-disk record reflects the approval the worker reads on the next cycle.
    expect(parseRecord(await src.readRecord(awaiting.path)).approval?.status).toBe('approved');
  });

  it('step 4 (Req 5.1/5.4): the UI reads the verified result the worker produced', async () => {
    const src = fresh();
    const done = await find(src, '07-succeeded');
    const record = parseRecord(done.text);

    // After scoped execution the record is succeeded with a passing verdict —
    // the result the UI surfaces to the user.
    expect(record.state).toBe('succeeded');
    expect(record.verdict?.result).toBe('pass');
    expect(record.verdict?.items.length).toBeGreaterThan(0);
    expect(record.verdict?.items.every((i) => i.pass)).toBe(true);
  });

  it('step 5 (Req 9.1): the GUI reports a live connection from the worker heartbeat', async () => {
    const src = fresh();

    // FixtureSource has no heartbeat -> the UI shows "not connected".
    expect(await src.readBackendHeartbeat()).toBeNull();
    expect(backendState(await src.readBackendHeartbeat(), now)).toBe('absent');

    // When the worker thread writes a fresh `_backend.yaml`, the same indicator
    // reports connected; an aged heartbeat reads stale.
    const freshBeat = '2026-10-24T18:59:55Z';
    const agedBeat = '2026-10-24T18:59:20Z';
    expect(backendState(freshBeat, now)).toBe('connected');
    expect(backendState(agedBeat, now)).toBe('stale');
  });
});
