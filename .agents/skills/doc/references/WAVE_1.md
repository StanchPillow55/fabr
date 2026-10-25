# Wave 1: Foundation

Goal: one task goes from reminder to succeeded through Telegram, with a grant, a fill and an
audit entry. A restart mid-run resumes. Everything here is a foundation block from
`ARCHITECTURE.md`, plus the learner as a capture-only placeholder.

## Contents

1. Exit test
2. Build order
3. Definition of done per item
4. Open with the owner

## 1. Exit test

On a fresh machine:

1. Finish the six setup screens.
2. Hand-write a reminder in the GUI.
3. Press delegate.
4. Answer one batched question on Telegram.
5. Approve plan, assertions and grants in one tap.
6. The executor fills address fields on a web form through fill at the sink.
7. The evaluator passes every assertion.
8. The notice arrives on Telegram and the GUI.
9. Run it again and kill the process during execute. It resumes from the last finished step.
10. The audit log shows one line per filled field with task, field, sink and time.

## 2. Build order

Each line depends only on the lines above it. Build and test in this order.

1. Settings and secrets files, and the setup wizard.
2. The task record: format, five states, waiting flag, atomic writes, validation.
3. The task index: scan, watch, due today, running, failed this week.
4. The GUI: tree, board, delegate button, answer box.
5. The Telegram bot: send, receive, inline buttons, single user id.
6. The notifier: four event kinds to both channels.
7. Knowledge files and the context gatherer: chain walk, precedence, grep.
8. The orchestrator skeleton: nine step slots, resume, per-folder lock, and the registry with
   register, heartbeat and cancel.
9. The definer: schema for "defined enough", question budget, batching, assertions, grant
   instead of ask, the three-option prompt shape.
10. The planner: plan, reversibility, tools, grants, assumptions.
11. The data bank: field tree, policy, grant request message, allow-for-this-task.
12. The tool set with fill at the sink in the browser and HTTP tools, payment refusal, audit log.
13. The executor: tool loop, folder scope, ask-user mid-run, heartbeat.
14. The evaluator: fresh context, per-assertion verdict with evidence.
15. The learner, capture only: answers on the record, project learned file written, data-like
    answers routed to the bank.
16. GUI additions: running agents list, rules and preferences editors, data bank policy table,
    audit log view.
17. The exit test, including the kill and resume.

## 3. Definition of done per item

- Setup: all six screens pass their checks on a clean machine. Quitting at step 3 and reopening
  lands on step 3; the secrets file has mode 600; no secret appears in any log.
- Task record: a record round-trips through read and write unchanged. An invalid transition is
  refused with a reason; a crash between temp write and rename leaves the old file intact.
- Index: deleting the index and restarting rebuilds it with the same rows.
- GUI: a reminder created by hand appears in the tree and on the board. Delegate moves it to
  task; a pending question can be answered from the GUI.
- Telegram: a message from any other user id is ignored. A question with buttons comes back as
  an answer in the record slot; a notice arrives within seconds.
- Notifier: done, failed, stuck and waiting each produce one message per channel.
- Gatherer: a preference set in a parent folder reaches a task in a child folder. A nearer value
  overrides a farther one; a past record's title is found by grep.
- Orchestrator and registry: a second delegate in the same folder queues. Killing the process
  at any step resumes at the next step; the board shows the running task with its step.
- Definer: a task with a complete seed asks nothing. A task with two gaps asks both in one
  message; a gap that is a bank field becomes a grant request, not a question; the success
  definition comes out as assertions.
- Planner: an order or a send is classified irreversible. The approval message lists tools,
  fields and assumptions.
- Data bank: a field marked always-allow is granted without a tap. An always-ask field appears
  in the approval message; a grant is scoped to the task and gone afterwards.
- Fill and audit: the executor's transcript never contains a bank value. The web form receives
  the value; a payment field stops the run with a hand-off notice; the audit log has one line
  per fill.
- Executor: writes land only inside the task's folder. A mid-run question sets the flag and
  reaches Telegram.
- Evaluator: a deliberately wrong result fails the matching assertion and the record says why.
- Learner: every answer given during define is on the record with its question. A preference
  answer appears in the project learned file; an address-like answer is offered to the bank.

## 4. Open with the owner

- The exact record file format shared with the input project.
- The demo web form used in the exit test.
- Whether the first pour includes the GUI preferences editors or only the files.
