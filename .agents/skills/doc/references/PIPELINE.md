# The pipeline

What happens after the user presses "delegate". The order is code (`crates/fabr-agent/src/lifecycle.rs`, `advance`). The model works inside a step and never chooses the next one. Two steps are gates where the flow waits for the user: define (a question) and approve.

Delegate itself is not a step. It sets `requests.delegate`, and the orchestrator moves the record from reminder to task and sets `step: gather`. The record's `step` field has eight values, listed below.

| Step | Gate | Actor | Reads | Writes | Built |
|---|---|---|---|---|---|
| gather | no | orchestrator | the node folder | `### Gathered` bundle in Context | stub: lists the node's own files, no folder-chain walk |
| define | yes | definer (model) | Context with answers | spec, assertions, `grants.requested`, question | built |
| plan | no | planner (model) | spec, assertions | plan, `plan_meta` (reversibility, tools, checks), approval digest | built |
| approve | yes | the user, in the GUI | the defined task | `approval`, `grants.granted` | built; Telegram approval planned |
| execute | no | executor | plan | log; writes inside the node | built for `read_file`, `write_file`, `list_dir` lines; a model tool loop is planned |
| evaluate | no | orchestrator | assertions, `plan_meta.checks` | verdict, state succeeded or failed | built as allowlisted checks; a model evaluator is planned |
| learn | no | learner | answers, verdict | nothing | stub: logs "deferred" |
| notify | no | orchestrator | the record | `events.log` line, step cleared | events built; Telegram send planned |

## Rules the code enforces

- A second approved task in a node waits (`other_running`) and shows registry `activity: queued`. One in-progress task per node.
- Approval is checked before execute: status approved, a digest equal to the recomputed one, and `by` not `auto` when the plan is irreversible. A mismatch resets approval to pending.
- A refusal sends the record back through plan, with the user's note in the log.
- A stage that errors logs `error N`. The third consecutive error on the same stage moves the record to failed.
- Cancel applies only to in_progress; in other states the request is cleared and logged as ignored. A cancelled task is failed with `verdict` null and can be redelegated.
- A paused or currently suspended record is skipped by the loop.

## Restart and resume

Every step writes its artifact to the record before the next one starts. On restart the runner finds records that are in task, defined or in progress with a `step` set, and continues from that step. A record parked on a question or an approval keeps waiting.
