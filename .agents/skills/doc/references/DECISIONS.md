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
