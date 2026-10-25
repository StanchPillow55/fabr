# The task record

One file per task, in the folder that the task belongs to. It is the only shared state in fabric.
Every component reads and writes this file and nothing else. The state machine is the record:
a state is entered by writing one more part of the same file.

## Fields

Front matter, structured:

| Field | Written by | Meaning |
|---|---|---|
| state | the orchestrator only | reminder, task, defined, in_progress, succeeded, failed |
| title | the user or the organizer | the one line that names the task |
| due | the user or the organizer | optional date |
| waiting | the orchestrator | a flag, set while a question is out to the user, in any non-terminal state |
| pending_question | the definer or the executor | the question slot. Telegram and the GUI both answer into it, and the first answer wins |
| grants_requested | the definer | data bank fields that the task needs, as field paths |
| grants_granted | the user, through approval | the subset allowed, with scope: task, project or always |
| reversibility | the planner | reversible or irreversible |
| tools | the planner | the tools that the plan needs |
| assumptions | the planner | the always-rules the plan leaned on |
| answers | the definer and the executor | every user answer with its question and the moment it was asked |
| links | the orchestrator | the earlier record this one re-delegates, if any |

Sections, prose:

| Section | Written by | Meaning |
|---|---|---|
| Context | the user, the organizer, then the gatherer | what the user wrote, then the merged bundle from the folder chain, learned records and past tasks |
| Spec | the definer | the task stated specifically enough to do |
| Success | the definer | the success definition rewritten as a few assertions that the evaluator can check |
| Plan | the planner | the ordered steps |
| Log | the executor and the evaluator | what happened, then the verdict per assertion with evidence |

The user owns title, due and the first paragraph of context. The pipeline owns the rest. A field
that the pipeline owns is optional on a reminder and required by the state that needs it.

## The five states

### Reminder
A title, maybe a due date. Written by the organizer from a dump, or by hand in the GUI. Light by
design: most reminders never become agent tasks. Exit: the user presses delegate, or starts
filling the spec by hand.

### Task
Claimed by the orchestrator when delegate is pressed. It takes the per-folder lock, gathers
context and starts the definer. The record gains the context bundle and, as they arrive, the
captured answers. Waiting on the user is the flag, not a state. Questions go out batched and
within a budget. Exit: the definer satisfies the schema: spec present, success definition
rewritten as assertions, grants listed.

### Defined task
The definer's work, then the planner's: plan, reversibility class, tools, grants requested and
the always-rules used. This is the only place a human has to look at the whole thing, so the
one approval message must be good. Exit: approval. The user taps approve on one message that
carries plan, assertions and grants. Auto mode skips the tap unless an ask-level field, payment,
or an irreversible action is involved.

### Task in progress
The executor runs under the registry, then the evaluator runs with a fresh context. The record
gains the log, the grants granted, the fills performed (also in the audit log) and the verdict
per assertion. Sub-steps: running, waiting on user, evaluating. A mid-run question sets the
flag. Exit: the evaluator returns a verdict.

### Succeeded or failed
Terminal. The evaluator sets the state; the learner and the notifier run after. The record
gains the verdict, the learned records written and the notice sent. A failed task can be
re-delegated, which creates a new record that links back to this one. Learn runs on both
outcomes, but after a failure only from user answers, never from what the agent concluded.

## Rules that keep the record honest

- Only the orchestrator changes the state field.
- A step writes its artifact before the state advances, so a restart resumes from the last
  finished step.
- Writes are atomic: write a temp file, then rename.
- The record is plain text, so it can be read, diffed and fixed by hand.
