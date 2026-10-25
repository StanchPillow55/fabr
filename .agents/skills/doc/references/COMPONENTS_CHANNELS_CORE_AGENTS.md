# Components: channels, core, agents

Each block has its status, what it is, what it owns, what wave 1 builds, what comes later, and
which blocks it talks to. The knowledge and private layers are in
`COMPONENTS_KNOWLEDGE_PRIVATE.md`.

## Contents

1. Channels
2. Core
3. Agents

## 1. Channels

### First-run setup (foundation)
A wizard that runs on first launch and again whenever a step is missing. It shows one step,
waits until that step's check passes, then shows the next. Progress is saved in machine
settings, so quitting halfway resumes at the open step. Secrets are written once and never
echoed back. The six screens are in `SETUP.md`.
- Owns: setup progress in the settings file; the secrets file, mode 600.
- Wave 1: the six screens, each with its own check, a back button, and resume.
- Later: re-run any single step from the GUI settings page; more models when available.
- Talks to: Local GUI, Machine settings, Telegram bot.

### Input capture (parallel project)
The dump. A line of text today. Speech and picture inputs come later and change nothing below.
- Owns: nothing durable; it hands text to the organizer.
- Wave 1: a text box in the GUI and a plain Telegram message.
- Later: speech and photo input (wave 4).
- Talks to: Organizer.

### Local GUI (foundation)
A local web app over the filetree. The folder tree with its tasks, a board by state, the running
agents view, editors for rules and preferences, the data bank policy and the audit log.
- Owns: nothing; it is a view over files.
- Wave 1: tree, board, delegate button, pending-question answer box, running agents list,
  preferences editor (textarea plus structured fields), data bank policy table, audit log.
- Later: learned preference management (promote, demote, delete), proposal acceptance,
  long-form review of plans and outputs.
- Talks to: Task record store, Orchestrator, Agent registry, Global rules and preferences,
  Project knowledge and preferences, Learned preferences, Personal data bank, Audit log.

### Telegram bot (foundation)
The agent talks to the user here. Short decisions only: batched questions, one-tap approvals,
grant requests, done and failed notices. Long things link to the GUI. Locked to one user id.
- Owns: the mapping from message id to task id. The token lives in the secrets file.
- Wave 1: send a question, receive an answer, inline buttons for approve, once, always and no,
  and notices.
- Later: richer previews, voice messages.
- Talks to: Notifier, Orchestrator, Task record store.

## 2. Core

### Task record store (foundation)
The spine. A task is a file under the folder it belongs to. Front matter holds state, due date,
grants and the pending question. Sections hold context, spec, plan, success definition and the
log. Every component reads and writes this file and nothing else. Details in `TASK_RECORD.md`.
- Owns: the state (reminder, task, defined task, in progress, succeeded or failed); context,
  spec, plan, success definition; the pending-question slot; grants requested and granted;
  captured user answers; the log of stage artifacts.
- Wave 1: the schema, a reader and a writer, atomic writes, state transitions with validation.
- Later: attachments, versions.
- Talks to: Task index, Orchestrator, Local GUI, Telegram bot, Past executions.

### Task index (foundation)
Answers cross-folder questions: what is due today, what is running, what failed this week.
Rebuilt from the tree at any time.
- Owns: derived rows only.
- Wave 1: scan on start, watch for changes, a handful of queries.
- Later: full-text search over past tasks (wave 2).
- Talks to: Task record store, Local GUI.

### Orchestrator (foundation)
Runs the fixed sequence: gather context, define, plan, approve, execute, evaluate, learn,
notify. Each step writes its artifact to the record before the next starts, so a restart
resumes from the last finished step. The model never chooses the next step. Steps in
`DELEGATE_FLOW.md`.
- Owns: the step order; the gates for approval, grants and irreversible actions; restart and
  resume; the question budget.
- Wave 1: all step slots, auto mode with its three exceptions (ask-level fields, payment,
  irreversible actions).
- Later: parallel tasks per folder, retries with a changed plan.
- Talks to: Task record store, Agent registry, Notifier, Context gatherer, Definer, Planner,
  Executor, Evaluator, Learner.

### Agent registry (foundation)
The live table behind the running agents view. One entry per active executor: task id, folder,
step, elapsed time, cost, last log lines, pending question. A heartbeat marks a dead agent. One
running task per folder for now.
- Owns: runtime entries, not durable.
- Wave 1: register, heartbeat, cancel, a per-folder lock.
- Later: cost caps, pause and resume, several agents per folder (wave 3).
- Talks to: Orchestrator, Local GUI, Executor.

### Notifier (foundation)
Turns a task event into a message: done, failed, stuck, waiting for you. It names the
always-rules that the plan relied on, so assumptions are visible.
- Owns: nothing.
- Wave 1: four event kinds to Telegram and the GUI.
- Later: digest mode, quiet hours (wave 3).
- Talks to: Telegram bot, Local GUI, Orchestrator.

## 3. Agents

### Organizer (parallel project)
Owned by the parallel project. Reads a dump, decides which folder each item belongs to, writes a
task record with state reminder and a title. The contract is the file format, nothing more.
- Owns: the reminder state only.
- Wave 1: agree on the record format and the folder for new records.
- Later: belongs to the other project.
- Talks to: Input capture, Task record store.

### Context gatherer (foundation)
Not a model. Walks the folder chain and collects knowledge and preferences. Loads learned
records for the project and globally. Greps finished tasks in the folder. Hands one bundle to
the definer and the planner.
- Owns: the resolution order.
- Wave 1: chain walk, merge by precedence, grep over past tasks.
- Later: search across folders; embeddings only if grep stops being enough (wave 4).
- Talks to: Machine settings, Global rules and preferences, Project knowledge and preferences,
  Learned preferences, Past executions, Definer, Planner.

### Definer (foundation)
Turns the seed into a defined task. Fills the spec and rewrites the success definition into a
few assertions that the evaluator can check. Asks the user only what the bundle cannot answer,
batched in one message, within a question budget. When a prior exists it asks the three-option
question. When the gap is personal data it requests a grant instead of asking.
- Owns: spec, success assertions, grants requested, captured answers.
- Wave 1: the schema that says when a task is defined enough, the budget, batching, and the
  three-option prompt shape.
- Later: per task-type checklists.
- Talks to: Context gatherer, Task record store, Personal data bank, Telegram bot, Orchestrator.

### Planner (foundation)
Writes the plan. Classifies the task as reversible or not. Lists the tools it will need, the
data fields it will fill, and every learned always-rule that it leaned on. All of that is what the
user approves.
- Owns: plan, reversibility class, tools and grants needed.
- Wave 1: the plan with the four lists above.
- Later: cost estimate, alternatives.
- Talks to: Context gatherer, Task record store, Orchestrator.

### Executor (foundation)
The only agent with side effects. Runs the plan with the tool set, scoped to the folder. Uses
fill placeholders and never sees bank values. Can ask the user through the orchestrator
mid-run. Registered and heartbeating.
- Owns: the log.
- Wave 1: the tool loop, folder scope, the ask-user tool, fill placeholders.
- Later: sandboxing beyond folder scope; checkpoints before irreversible steps (wave 3).
- Talks to: Tool set, Agent registry, Orchestrator, Task record store.

### Evaluator (foundation)
Starts with no memory of the run. Reads the success assertions and the evidence and returns a
verdict per assertion. Sets succeeded or failed. After an irreversible step it can only report.
- Owns: the verdict.
- Wave 1: a per-assertion verdict with evidence.
- Later: re-plan on failure (wave 3).
- Talks to: Task record store, Orchestrator.

### Learner (placeholder)
Two moments. During define, every user answer is captured with its question and context. After
evaluation, each capture is classified. A preference goes to learned records. A data-like
answer goes to the bank. An environment fact becomes a staged procedure proposal. Only user
answers are learned from failed runs, never agent conclusions.
- Owns: the classification; the scope proposal, project or global.
- Wave 1: capture answers; write preference records to the project learned file; route
  data-like answers to the bank.
- Later: procedure proposals, promotion to global after confirmation in two projects,
  staleness review of always-rules (waves 2 and 3).
- Talks to: Task record store, Learned preferences, Project knowledge and preferences,
  Personal data bank, Orchestrator.
