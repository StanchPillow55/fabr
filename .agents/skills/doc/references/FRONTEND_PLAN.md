# Frontend plan

Two people build fabric. Dev 2 builds the agent backend: the orchestrator, the agents, the data
bank and the Telegram transport, following the spec in this folder. We build the frontend: the
local GUI and the first-run setup. This file carves our part out of the spec, fixes the contract
between the two tracks, and lays out our waves. Product waves stay in `WAVES.md`; this file says
which pieces of each are ours.

## Contents

1. Ownership
2. The contract between the tracks
3. Strategy: build against fixture records
4. Screens
5. Frontend waves
6. Open with dev 2

## 1. Ownership

| Block | Owner | Note |
|---|---|---|
| Local GUI, all screens | us | the only block that is wholly ours |
| First-run setup | us: every screen, including API key entry, the model selector and the Telegram bot onboarding | dev 2 exposes only the checks: folder writable, API key works, model answers, bot token valid, and the Telegram link listener |
| Task record store | shared | dev 2 writes the reader, writer and validator once; both tracks use the same format |
| Task index | us | a view-side cache; we own the queries that the GUI needs |
| Agent registry and agent status UI | dev 2 writes the registry files; we own the agent status screen | the Agents screen is a view over the registry and the events log |
| Notifier | dev 2 | the GUI badge is our rendering of its events |
| Telegram bot | dev 2, transport | we write the message templates and button layouts as a spec, because they are the phone half of the same conversation that the GUI shows |
| Orchestrator, gatherer, six agents | dev 2 | we never call them; we watch the record change |
| Data bank, fill, audit log | dev 2 | we own the policy table and the audit view in the GUI |
| Knowledge files | shared format | we own the editors; dev 2 owns the loader |
| Machine settings and secrets | setup writes, backend reads | format fixed in `ON_DISK.md` |

## 2. The contract between the tracks

Everything crosses the boundary as a file, never as a function call. This is principle 4 of
`PHILOSOPHY.md` applied to two developers.

- The task record is the whole API. We render it and write into three places only: the answer
  slot, the approval fields, and request flags. We never write the state field.
- Request flags. The GUI asks for things by setting a flag on the record that the orchestrator
  picks up and clears: delegate requested, cancel requested, re-delegate requested. The state
  changes when the orchestrator acts, not when we click.
- The answer slot. A pending question is a slot on the record with the question text, the kind
  (open, three-option, grant request) and the options. Telegram and the GUI both write the
  answer into the same slot. First answer wins; the loser sees the answer that landed.
- Approval fields. The approval screen writes the granted fields with their scope and the
  approve or refuse verdict with an optional note. The orchestrator reads them and advances.
- The registry is a folder with one file per running agent. Each file holds task id, folder,
  step, started at, heartbeat at, cost so far, last log lines and pending question. We list it
  and we set the cancel flag. Dev 2 writes the rest.
- Settings and secrets. Setup writes them; the backend reads them. The backend provides the
  checks setup needs; setup never talks to Anthropic or Telegram on its own.
- Change notification. The GUI watches the folders. No sockets, no push. A record that changes
  on disk re-renders.

## 3. Strategy: build against fixture records

Because the contract is files, the frontend needs no backend to be built. We keep a fixtures
folder with one task record per state and per flag. The set:

- a bare reminder
- a task waiting on an open question
- a task waiting on a three-option question
- a defined task awaiting approval with two grant requests
- a task running, with a registry entry
- a task waiting mid-run
- a succeeded task with a full log and verdict
- a failed task with one failed assertion
- a re-delegated pair that link to each other
- one registry folder with two live agents and one dead one

Every screen is built and reviewed against those fixtures first. The day dev 2's orchestrator
writes real records, the screens already fit. If they do not, the format was wrong, and we both
find out at the file, not in a call.

The fixtures are also the format contract in executable form. When either track changes the
record, the fixtures change in the same commit.

## 4. Screens

| Screen | Shows | User can |
|---|---|---|
| Tree | folders of the default folder, each with its task count by state | open a folder, open a task, add a reminder by hand |
| Board | all tasks grouped by the five states, waiting ones marked | filter by folder, open a task |
| Task record | the record as it is on disk: title, due, context, spec, success assertions, plan, log, verdict; unfilled sections shown as empty slots, so progress is visible | press delegate, answer a pending question, cancel, re-delegate a failed task, edit title, due and context on a reminder |
| Question | the pending question in its kind: open text, the three options once, always, no with a text field, or a grant request with the field tree | answer; the answer lands in the slot |
| Approval | plan, success assertions, grants requested with scope choice per field, reversibility, tools, assumptions from always-rules | approve in one tap, refuse with a note, change a grant's scope before approving |
| Agents | every registry entry: task, folder, step, elapsed, cost, last log lines, pending question; dead ones marked by heartbeat age | open the task, cancel |
| Setup | six screens, one at a time, with the check result per screen | go back, retry a failed check |
| Rules and preferences | global and per-folder: a textarea for the prose file, typed fields for the structured file | edit and save; the two files never mix |
| Data bank policy | the field tree with always-allow or always-ask per field or subtree | change a policy; add, rename or remove a field |
| Audit | the audit log newest first: task, field, sink, time | filter by task or field |
| Learned preferences (wave 2) | learned records with scope, count, source tasks, last confirmed | promote, demote, delete |

Design rules for all screens: the record is shown as it is, not reinterpreted. Progress is
which sections are filled. Anything the user can change writes to a file, and the screen
re-renders from that file, so what you see is what is on disk.

## 5. Frontend waves

Our waves are smaller than the product waves and front-loaded. The agentic execution screens
are the first thing that dev 2's backend will need a face for.

| Wave | Name | Scope | Exit test |
|---|---|---|---|
| F0 | Fixtures | the fixture records and registry above, agreed with dev 2 | dev 2 reads every fixture with their reader without error |
| F1 | Agentic execution | Tree, Board, Task record, Question, Approval, Agents, GUI badge for notices | every fixture renders; answering a question and approving write the right fields; a change to a fixture on disk re-renders within a second |
| F2 | Setup | the six screens with a default folder of `~/`; checks stubbed until dev 2's are ready | quitting at screen 3 and reopening lands on screen 3; a stubbed failed check keeps the user on the screen with the reason |
| F3 | Settings and privacy | Rules and preferences, Data bank policy, Audit | editing the prose file does not touch the structured file and the reverse; a policy change is visible in the file |
| F4 | Tell it once, GUI half | Learned preferences screen; project grant scope and revocation in Approval | promote, demote, delete round-trip through the learned file |
| F5 | Trust and control, GUI half | proposal acceptance, long-form review view, pause and resume, cost caps shown on Agents | a proposal accepted in the GUI appears in the project knowledge file |

F1 and F2 together are our half of product wave 1. F3 finishes it. F4 is product wave 2. F5 is
product wave 3. Product wave 4 has no frontend work beyond a connector grant with a filter in
the Approval screen.

## 6. Open with dev 2

The five items that were open here are now proposed in `../dev2/agent_surface_spec.md`. They are
the record format and fixtures, the request flags, the registry format and heartbeat, the setup
checks, and the Telegram message kinds. What stays open is dev 2's review of that file and the
message wording, which we draft.
