# Philosophy

Twelve principles shape every part of fabric. When a new idea does not fit one of them, change
the idea or raise the principle with the owner. Do not bend the principle without saying so.

## 1. The filetree is the spine

A folder is a project. Tasks, knowledge and preferences all hang off the path. A task in
`a/b/c` inherits from `a/`, `a/b/` and `a/b/c/`, nearer wins. This is the same move that
coding agents make with per-folder instruction files, and it is proven. Nothing in fabric has
an identity that is not a path.

## 2. The task record is the state machine

One file per task holds title, due date, context, spec, plan, success definition, state and
log. The states are entered by filling one more part of that file. A reminder, which the UI also calls a todo, is the same record in its first state. There is no separate
workflow model. The GUI shows progress by showing which fields are filled.

## 3. The outer loop is code, the model works inside a step

The order define, plan, approve, execute, evaluate, learn, notify is fixed in code. The model
never chooses the next step. Every gate is a line of code, not a prompt instruction. Each step
writes its artifact to the record before the next starts. A restart resumes from the last
finished step, and every run can be replayed and inspected.

## 4. Files are truth, everything else is a view or a cache

The record in the folder is the only source of truth. Telegram, the GUI, the registry and the
index all read and write that file. The index can be deleted and rebuilt at any time. No state
that the system depends on lives only in memory or only in a database.

## 5. The agent fills, it never reads

Privacy here is about the model's context window, not about storage. The agent holds field
names like `address.zip` and writes placeholders. A trusted tool substitutes the value at the
moment of use, and only when a grant exists. The crypto is a boring file. The policy, the
grants and the audit log are the product. Payment is never filled by the agent.

## 6. Tell it once

Every answer that the user gives is captured with its question. The next time the same kind
of decision comes up, the agent offers the prior back. The user has three answers: once,
always, or no and here is the new one. Only an always-rule is applied without asking, and every always-rule a
plan leans on is listed in the approval message, so assumptions are never silent.

## 7. The user keeps the wheel

One approval message carries the plan, the success assertions and the data grants, and one tap
approves all three. Auto mode skips that tap except for three cases: an ask-level data field,
payment, or an irreversible action. An evaluator can only report after an irreversible step,
so the gate sits before it.

## 8. One data model, thin adapters

The record and its state machine are the core. Adapters sit over it: the Telegram bot, the GUI,
the agent runtime, the data bank, the learner and the registry. An adapter only reads and
writes the record, so each one can be built, demoed and replaced on its own.

## 9. Pour the foundation once

Wave 1 defines every file format and every state, and later features fill slots it left. A
placeholder is a slot with a name, a file and an empty body. A format change needs the owner
and a new schema number. Schema 4 is the third since the first pour.

## 10. Name what is not built

A feature that is presented but not implemented is drawn on the map and marked. Personal
context connectors are the first example. Saying "named, not built" is better than leaving a
hole that someone fills by accident.

## 11. Short decisions on the phone, long reading at the desk

Telegram carries batched questions, one-tap approvals, grant requests and notices. Anything
longer than a screen links to the GUI. The bot answers one user id and no one else.

## 12. Two writers never share a file

People edit prose rules and structured preferences from the GUI. The agent writes only its own
learned records and proposals. No file has both writers, so nothing is clobbered and the user
keeps trusting what they wrote. In the task record the orchestrator alone writes `state`. The
one owner-approved exception is the GUI moving a reminder to succeeded by hand.

## What the design rests on

- One key for everything: the path gives tasks, knowledge and preferences their inheritance.
- A deterministic outer loop, so every stage leaves an artifact you can inspect and replay.
- A separate evaluator with a fresh context, so the executor never grades its own work.
- Fill, never read: leaks happen in the model's context window, not in storage.
- Telegram fits a loop that mostly waits on the user: asynchronous, on the phone, cheap to set up.
