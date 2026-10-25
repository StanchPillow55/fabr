# Decisions

Each line records one decision that a later change could undo by accident. Write it as a date,
the decision, and the reason, with the newest line last.

2026-10-24 The product is named fabric. The repo and the code still say fabr. The owner will trigger the rename as a plain string replace, so no file here introduces a third spelling.
2026-10-24 The filetree is the spine. Tasks, knowledge and preferences are all keyed by folder path, with inheritance up the chain, because one key keeps the three from drifting apart.
2026-10-24 A task has exactly five states: reminder, task, defined, in progress, succeeded or failed. Waiting on the user is a flag, not a state, so a question can be pending in any non-terminal state without multiplying states.
2026-10-24 The task record in the folder is the only shared state. Telegram, GUI, registry and index are adapters over it, because one data model keeps the exploding feature list buildable one piece at a time.
2026-10-24 The outer loop is code and the model works inside a step. Gates in code can be tested and replayed, and gates in prompts cannot.
2026-10-24 Only the orchestrator changes the state field, so no two components race on a transition.
2026-10-24 The context gatherer is not a model. It does a chain walk, a precedence merge and grep, because retrieval must be deterministic and cheap before it is clever.
2026-10-24 The success definition is rewritten into checkable assertions, and the user approves them with the plan. The evaluator is only as good as what it checks against.
2026-10-24 One approval message carries plan, assertions and grants, and one tap approves all three. The defined state is the only place a human looks at the whole task.
2026-10-24 Auto mode never covers ask-level data fields, payment, or irreversible actions, because an evaluator after an irreversible step can only report.
2026-10-24 The agent fills and never reads. Values are substituted inside the browser and HTTP tools against a grant, because privacy is about the model's context window, not storage.
2026-10-24 Payment is never filled by the agent in wave 1; the executor stops and hands the last step to the human. Virtual cards may lift this in wave 4.
2026-10-24 Grant scopes: allow-for-this-task in wave 1, allow-for-this-project in wave 2, plus always-allow and always-ask in the policy. Per-task grants are the unit that the user reasons about.
2026-10-24 The data bank's crypto is an encrypted file for the demo; the policy, grants and audit log are the product.
2026-10-24 Telegram carries short decisions only and links to the GUI for anything long; the bot answers one user id.
2026-10-24 Precedence: nearer folder over farther, project over global; machine settings describe what exists and never conflict with preferences.
2026-10-24 Hand-written preferences are a prose file plus a structured file; the agent writes only learned records and proposals. No file has two writers.
2026-10-24 The learner captures user answers during define and classifies them after evaluate into preference, data or procedure. Three stores, because a mixed store leaks addresses into preferences.
2026-10-24 Only user answers are learned from a failed run; agent conclusions are kept only after a passed evaluation.
2026-10-24 A prior is offered with three options: once, always, no. Only an always-rule is applied silently, and every always-rule that a plan uses is listed in the approval message.
2026-10-24 Always means this project; promotion to global happens after confirmation in two projects and is announced.
2026-10-24 Procedural learnings are proposals accepted in the GUI, never written straight into knowledge files, because an agent writing its own instructions drifts.
2026-10-24 One running task per folder in wave 1; the rest queue.
2026-10-24 Every step writes its artifact before the state advances, so a restart resumes from the last finished step.
2026-10-24 Personal context connectors are named on the map and not built, so nobody fills the hole by accident.
2026-10-24 No wave after the first changes a file format or adds a state. A feature that needs that goes back to the owner.
2026-10-24 Setup is a wizard that shows one step at a time with a check per step, as the owner asked. It offers only Sonnet (claude-sonnet-5-5, suggested) and Haiku (claude-haiku-5-5).
2026-10-24 The API key and the bot token live in a secrets file with mode 600 at the machine root. They are never loaded into an agent context.
2026-10-24 The input project's contract is a record with state reminder and a title; everything else is optional and owned by the pipeline.
2026-10-24 Agent documentation here covers design and philosophy only; code documentation follows later, as the owner asked on 2026-10-24.
2026-10-24 The default folder that setup suggests is `~/`, not `~/projects`, as the owner asked.
2026-10-24 Two tracks: dev 2 builds the agent backend and the Telegram transport; we build the GUI and setup. Everything crosses the boundary as a file, never as a function call.
2026-10-24 The GUI never writes the state field. It writes answers, approval fields and request flags that the orchestrator picks up and clears, so one writer owns every transition.
2026-10-24 The frontend is built against fixture records, one per state and flag, agreed with dev 2 before either side writes a screen or a step. The fixtures are the format contract in executable form.
2026-10-24 The interaction surface between the agent backend and the frontend is specified in `dev2/agent_surface_spec.md`, which wins over the design files where they differ. New record fields need a new schema number and a new version of that file.
2026-10-24 Human-edited files are TOML or markdown, machine-written files are YAML, and the task record is YAML front matter plus five fixed markdown sections, so each side can tell by format who writes a file.
2026-10-24 The vault is plain TOML with mode 600 in wave 1 and the GUI writes it, as the owner allowed privacy to be "maybe zero for now"; encryption swaps in later behind the same field paths.
2026-10-24 Setup checks cross the boundary as a local command with JSON on stdout, the one exception to files-only, because they need the network.
2026-10-24 Cancel moves a task to failed with the reason logged and the verdict left null, so a cancelled task is re-delegated like any failed one.
2026-10-24 The frontend owns all setup screens (API key entry, model selector, Telegram bot onboarding) and the agent status UI; the backend exposes checks and writes the registry, nothing visual.
2026-10-24 Dev 2 agreed to `dev2/agent_surface_spec.md` version 1 as written. Changes from here go through a new version number, not an edit in place.
2026-10-24 The wave 1 demo task is ordering 2 kg of black PLA from a web shop, which fills address fields on a form and hands payment to the human. It is fixture 4 and the thread through fixtures 2 to 9.
2026-10-24 The task record schema is the integer 2 (`dev2/agent_surface_spec.md` version 1.1). `source` and `approval.digest` are backend-written, registry files are `<task_id>--<path_tag>.yaml`, and this slice starts without Telegram, so those additions are not left as an unrecorded edit of schema 1.
2026-10-24 The F1 screens live in the existing Svelte 5 + Tauri 2 app under `app/`, as an isolated module with its own state type, its own small store and its own file source. The existing todo Store and Status type are not bent. Chosen by the owner over a fresh app because the polling-stamp pipeline, the browser mock path and the ui kit are already there.
2026-10-24 Record parsing lives in the frontend in one place. The Rust side only lists and serves raw file text and includes `.fabric-space` paths in the stamp, so no YAML dependency is added to the Rust core and the parser is testable against the fixtures directly.
2026-10-24 F1 step 1 is built and reviewed in browser mock mode, loading the fixtures from `fixtures/` at build time. Rust commands and live file watching come in F1 steps 5 and 8.
2026-10-24 Milestone M1, the shippable shell: UI wired, real API key and Telegram setup, no agent backend, placeholders where the backend would be. Defined in `FRONTEND_EXECUTION.md` section 9 at the owner's request.
2026-10-24 One HTTP client crate (ureq or equivalent) is added to the Rust core so the setup checks can call Anthropic and Telegram for real. Owner approved. It is the only network client in the core and is used by nothing else.
2026-10-24 Backend presence is signalled by a heartbeat file `~/.fabric-space/registry/_backend.yaml` with `heartbeat_at`, proposed for spec version 1.1. Owner approved as the working assumption until dev 2 confirms; the GUI shows "not connected" when it is absent or stale.
2026-10-25 F1 (agentic execution screens) verified complete: nine steps plus one fix-up, each with an independent verifier, 297 frontend tests and 54 core tests green. The folder grouping rule is the segment before `/.fabric-space/tasks/`; the view is called Tasks in the UI.
2026-10-25 F2 (setup) verified complete: Rust setup core with ureq, six-screen wizard, launch gate, default folder wired into the tasks view, long network ops on their own thread in serve mode. The five checks live in our core as the placeholder for dev 2's command, with `fabr check` and `fabr telegram link` keeping the spec's CLI shape.
2026-10-25 Milestone M1 reached: fresh config opens on setup, real API key and Telegram checks, tasks screens over the default folder, writes with stale refusal, "Agent backend not connected" placeholder, desktop bundles built. A tilde in default_folder is expanded in the core on write, read, list and stamp.
2026-10-25 Verification budget: from M1 on, one light verifier per phase end, not per step, at the owner's request. Implementers self-check with the test gates.
2026-10-25 The tree is the primary and only main view. Tasks never leave their tree node: a folder node shows its task counts and a waiting or running mark, and clicking into the node expands the task's details, panels and logs. The separate Tasks view with Board, Tree and Agents tabs built in F1 is removed. Owner's call after seeing the 0.2.0 build; it also matches principle 1 of PHILOSOPHY.md.
2026-10-25 `sample-fabr-space/` is a frozen synthetic demo dataset: a machine root plus five project folders with tasks in every state, a running and a dead agent, notices and audit lines. It is not the format contract (that is `fixtures/`) and it changes only when the owner asks, never as a side effect of shipping a feature.
2026-10-25 A task is created from the node panel: an "Add task" line beside "add todo"; Enter or blur opens a details sheet with title, due, context, spec and success. "Save" writes a reminder through a new `fabric_create` command. "Generate with AI" writes the record and sets `requests.delegate`, which is the spec's way to start the definer, so no change is needed when dev 2's orchestrator arrives.
2026-10-25 A todo becomes a task through a "Make task" action on the item: the record is created from the todo's title and due with a Context line naming its origin, and the todo line is removed. Two copies of one item would drift, so the todo does not come back. Owner's call.
2026-10-25 Spec version 1.1 and schema 2 from dev 2's backend branch accepted as written. The core parser in taskrec.rs stays beside the frontend parser; the earlier line about no YAML crate in the core is superseded. The workspace keeps one HTTP client, ureq.
2026-10-25 The `fabr agent` runner is built on our side as a thin loop over dev 2's pipeline: heartbeat file, folder scan for request flags and answered slots, events log, registry refresh, stale cleanup. It wraps their code and does not change the pipeline. Owner's call so the GUI comes alive now.
2026-10-25 Every model call is logged with a key fingerprint (first 7 and last 4 characters), model, tokens and cost. The full key and the bot token are redacted from every log line, event, record and error. Owner asked to trace key use in the log.
2026-10-25 The single Tauri desktop app hosts `fabr_agent::runner::Runner` in-process on a background thread, started in the setup hook and stopped on the exit event through a desktop-owned stop flag rather than the runner's process-global signal handler, because a GUI must not install SIGINT or SIGTERM handlers; there is no HTTP server and the Data_API stays the in-process `call` to `fabr_core::ops::dispatch` path. The P0/P1 corrections land in the `fabr-agent` library, not the desktop crate, so every host benefits: escalating bounded backoff with reset-on-success replacing the fixed retry, transient-versus-terminal failure classification, an atomic registry claim replacing check-then-write, a replay guard that routes a non-idempotent interrupted execute to evaluate or needs-human, and readiness split into model-configured versus model-reachable surfaced through the Sink and events without changing the `_backend.yaml` contract.
