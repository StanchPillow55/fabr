# Design review

The strengths that the design rests on, then every concern raised in the founding conversation with
how it was settled. A concern marked "open" still needs the owner. Wave numbers point at
`WAVES.md`. Decision dates point at `DECISIONS.md`.

## Contents

1. Strengths
2. Concerns about the core
3. Concerns about the learning layer
4. Concerns about monitoring and scale

## 1. Strengths

- One key for everything. Tasks, knowledge and preferences resolve by path, with inheritance up
  the folder chain for free.
- The record is the state machine. Context, spec, plan and success definition map one to one
  onto the pipeline stages, so no separate workflow model is needed.
- A deterministic outer loop. Each stage leaves an artifact you can inspect and replay, and the
  approval point is a gate in code.
- A separate evaluator. A fresh context checking the result against the success definition
  stops the executor grading its own work.
- Fill, never read. Most privacy stories are about storage. This one is about the model's
  context window, which is where leaks happen.
- Telegram fits a loop that is mostly waiting on the user: asynchronous, on the phone, already
  does notifications, cheap to set up.

## 2. Concerns about the core

- Reminder and task are two different weights. Settled: the four heavy fields are optional on
  the seed and owned by the pipeline; the user owns title, due and context (2026-10-24).
- Define can ask forever. Settled: a question budget, all questions in one message, and a
  schema that says when a task is defined enough. Define never asks for data the bank holds;
  it requests a grant instead. Per task-type checklists are wave 2 or later.
- The success definition is the weakest field and the evaluator depends on it. Settled: define
  rewrites it as a few checkable assertions and the user approves them with the plan.
- Fill has to live at the sink. Settled: the browser driver and the HTTP client substitute
  placeholders at the moment of use. Read-back leaks (page text, screenshots, confirmation
  emails) are best effort and logged. Payment is never filled; the executor stops and hands the
  last step to the human. Virtual cards and redaction are wave 4.
- Do not build a password manager. Settled: an encrypted file is enough for the demo. The policy
  layer, grants and the audit log are the product.
- Grant scopes need three levels. Settled: always allow, always ask, allow for this task in
  wave 1; allow for this project and revocation in wave 2. A request is one message with one tap.
- Auto mode plus always-allow lets an agent buy things unasked. Settled: auto mode never covers ask-level fields,
  payment, or irreversible actions. Reversibility is classified by the planner.
- Precedence between layers is undefined. Settled: machine settings say what exists and never
  conflict with preferences; project overrides global; nearer folder wins.
- Prose rules do not GUI well. Settled: a hybrid of a prose file edited in a textarea and a
  structured file edited through typed fields.
- Telegram is wrong for long review. Settled: short decisions on Telegram, long things link to
  the GUI, bot locked to one user id. Long-form review in the GUI is wave 3.
- States need to be explicit. Settled: five states plus a waiting flag that may be set in any
  non-terminal state. The notifier reports done, failed, stuck and waiting.
- Define the contract with the input project now. Settled as a format: state reminder plus a
  title, everything else optional. Open: the exact file format still has to be agreed with the
  parallel project.
- Where tasks live. Settled: files in the folder are truth; a sqlite index is a rebuildable cache.

## 3. Concerns about the learning layer

- Three kinds of learning must not share a store. Settled: user preferences go to learned
  records, personal data goes to the bank, environment facts become procedure proposals. The
  learner classifies every captured answer into one of the three.
- Learned and hand-written preferences need separate files. Settled: learned records are
  structured and agent-written; rules and preferences are hand-written. See principle 12.
- Matching a past decision to a new situation is the real problem. Settled: the agent proposes
  the match and the user confirms it through the three-option prompt. Only always-rules apply
  silently. Wave 2.
- "Always" needs a scope and a way back. Settled: always means this project. Promotion to
  global happens after confirmation in two projects and is announced. Every always-rule that a plan
  uses is listed in the approval message. Wave 2 for the prompt, wave 3 for promotion.
- Learned rules go stale. Settled: ordinary preferences self-correct through re-asking.
  Always-rules carry a last-used date and get a review nudge. Wave 3.
- Contradictions need a rule. Settled: the more specific scope wins. A "no" answer asks whether
  to replace the old record or add a new one for this case. Wave 2.
- Do not learn from failures. Settled: user answers are captured at any time; agent conclusions
  about the environment are kept only after the evaluator passes.
- Procedural learning is self-modifying instructions. Settled: proposals are staged and the
  user accepts them in the GUI. Auto-accept with a cap is a later option. Wave 3.
- The learn step has two moments. Settled: capture during define, classify after evaluate.
- Past executions need an index. Settled: grep over finished records in wave 1, index-backed
  full-text search in wave 2, embeddings in wave 4 only if grep fails.
- Every question becoming a three-option prompt is noise. Settled: the three options appear
  only when a prior exists; otherwise the open question. Both kinds are batched.

## 4. Concerns about monitoring and scale

- A registry of running agents is its own component. Settled: register, heartbeat, cancel, one
  task per folder in wave 1; cost caps, pause and resume, several per folder in wave 3.
- Telegram and the GUI must both answer the same question. Settled: the pending question is a
  slot on the record, both channels write to it, first answer wins.
- Two agents in one folder will collide. Settled: one running task per folder, the rest queue,
  the board says so.
- Crash recovery. Settled: every stage writes its artifact before advancing; restart resumes
  from the last finished stage.
- The application explodes. Settled by principle 8: one data model with thin adapters.

## Still open with the owner

- The exact record file format shared with the input project.
- The demo task for the wave 1 exit test. The plan assumes filling address fields on a web form.
- The repo rename from fabr to fabric, parked.
