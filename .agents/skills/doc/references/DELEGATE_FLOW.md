# The delegate flow

What happens after the user presses "do with agent". The order is code. The model works inside a
step and never decides which step comes next. A gate is a step where the flow can stop and wait
for the user. Two steps are gates: define and approve.

| # | Step | Gate | Actor | Reads | Writes |
|---|---|---|---|---|---|
| 1 | Delegate pressed | no | GUI or Telegram | the record | state task, registry entry |
| 2 | Gather context | no | context gatherer, no model | machine settings, global and project agent files, learned records, past records | context bundle into the record |
| 3 | Define | yes | definer | the bundle | spec, success assertions, grants requested, answers, pending question |
| 4 | Plan | no | planner | spec, assertions, bundle | plan, reversibility, tools, assumptions |
| 5 | Approve | yes | the user, via Telegram or GUI | the defined task | state in progress, grants granted |
| 6 | Execute | no | executor | plan, grants granted | log, audit entries |
| 7 | Evaluate | no | evaluator, fresh context | assertions, log and outputs | verdict, state succeeded or failed |
| 8 | Learn | no | learner | answers, verdict | learned records, bank fields with confirmation, proposals later |
| 9 | Notify | no | notifier | the record | Telegram message, GUI badge |

## Each step in full

1. **Delegate pressed.** The orchestrator claims the record, moves it to task, and takes the
   per-folder lock. A second delegate in the same folder queues and the board says so.
2. **Gather context.** Walk the folder chain from the task's folder up to the root and merge
   knowledge and preferences by precedence: nearer wins, project over global. Load learned
   records for the project and globally. Grep finished records in the folder for titles, tags
   and answers. Write one bundle into the record.
3. **Define.** Fill the spec. Rewrite the success definition as checkable assertions. For each
   gap, one of three things happens. If a prior exists, ask the three-option question. If the
   gap is personal data, add the field to grants requested. Otherwise ask the open question. All questions go in one message,
   within the budget. The step waits on the flag until the answer lands in the slot.
4. **Plan.** Write the ordered steps. Classify the task as reversible or irreversible. List the
   tools that are needed, the data fields to fill, and every always-rule relied on.
5. **Approve.** One message: plan, assertions, grants. One tap. Auto mode skips the tap unless
   an ask-level field, payment, or an irreversible action is involved. First answer from either
   channel wins. A refusal returns the record to defined with the user's note in the log.
6. **Execute.** Run the plan with the tool set inside the folder. Fill placeholders are resolved
   by the browser and HTTP tools against granted fields and logged. Payment is never filled; the
   executor stops and hands the last step to the human. The executor heartbeats to the registry.
   A mid-run question sets the flag and goes out through the notifier.
7. **Evaluate.** With no memory of the run, check each assertion against the evidence. Return a
   verdict per assertion and set succeeded or failed. After an irreversible step the verdict
   can only report.
8. **Learn.** Classify each captured answer. A preference becomes a learned record at project
   scope. A data-like answer goes to the bank after the user confirms. An environment fact
   becomes a staged proposal, from wave 3 on. After a failed run, agent conclusions are skipped.
9. **Notify.** Tell the user the outcome, what was assumed from always-rules, and where to look
   in the GUI for anything long.

## Restart and resume

Every step writes its artifact to the record before the next starts. On restart, the
orchestrator finds records in non-terminal states and resumes each from the step after the last
finished one. A record in task or in progress with the flag set simply keeps waiting.
