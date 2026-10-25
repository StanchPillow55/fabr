# Architecture

Fabric takes a line dumped into a folder and turns it into a task. An agent defines, plans, runs and checks the task, and the user approves it. This file is the big picture: the layers, the status legend, the 26 blocks with what is built today, and what the app looks like.

## Status legend

| Status | Meaning |
|---|---|
| built | in code today |
| stub | the slot, name and step exist; the body is a placeholder |
| planned | designed here, no code |
| parallel | owned by the input project that runs alongside; only a file contract is shared |
| named | drawn on the map so nobody fills the hole by accident; never scheduled |

## Layers

| Layer | Holds |
|---|---|
| Channels | how the user touches it |
| Core | the record store, the sequencer, the registry, the notifier |
| Agents | model workers with one job each |
| Knowledge | files the agents read, merged by precedence: nearer folder wins, project over global |
| Private data and tools | where values and side effects live |

Everything above the knowledge layer reads and writes the task record in the folder. The context gatherer is not a model: it walks the folder chain and merges knowledge. The executor is the only agent with side effects. The data bank hands values to fill, never to an agent, and every fill lands in the audit log. The orchestrator is the only sequencer and the only writer of `state`, except the done-by-hand exception in `CONTRACT.md`.

## The 26 blocks

| Layer | Block | Status | Where it lives, or what is missing |
|---|---|---|---|
| Channels | First-run setup | built | `fabr-core/src/setup.rs`, `app/src/lib/setup/`; six screens |
| Channels | Input capture | parallel | the app has an add-task line and sheet; speech and photo are wave 4 |
| Channels | Local GUI | built | `app/`, a Svelte 5 and Tauri 2 desktop app |
| Channels | Telegram bot | planned | setup checks and the link command exist; no send or receive; wording in `TELEGRAM.md` |
| Core | Task record store | built | `fabr-core/src/taskrec.rs`, per-folder flock |
| Core | Task index | planned | no sqlite; the app polls a stamp over the tree |
| Core | Orchestrator | built | `fabr-agent/src/lifecycle.rs`, hosted in the desktop app |
| Core | Agent registry | built | `fabr-agent/src/board.rs`, heartbeat in `runner/` |
| Core | Notifier | stub | writes `events.log`; fan-out to Telegram is planned |
| Agents | Organizer | parallel | not in this repo |
| Agents | Context gatherer | stub | lists the node's own files; the folder-chain walk and merge are planned |
| Agents | Definer | built | model call in `lifecycle.rs` and `model.rs` |
| Agents | Planner | built | same; writes `plan_meta` and the digest |
| Agents | Executor | stub | runs `read_file`, `write_file`, `list_dir` plan lines; the tool loop is planned |
| Agents | Evaluator | stub | runs allowlisted checks; a model evaluator is planned |
| Agents | Learner | stub | logs "deferred" |
| Knowledge | Machine settings | built | `settings.toml`, `secrets.toml` |
| Knowledge | Global rules and preferences | planned | no files, no editor |
| Knowledge | Project knowledge and preferences | planned | same |
| Knowledge | Learned preferences | planned | wave 2 |
| Knowledge | Past executions | planned | wave 2 search over finished records |
| Private | Personal data bank | planned | no vault, policy or crypto |
| Private | Fill at the sink | planned | no browser or HTTP tool |
| Private | Audit log | planned | comes with fill |
| Private | Tool set | stub | three file tools; browser, HTTP and ask-user are planned |
| Private | Personal context connectors | named | never built in wave 1 |

## The app

- The tree is the only main view. A folder node shows task counts and a waiting or running mark, and clicking into it shows its tasks, panels and logs. There is no Board or Tabs view.
- A node card has a gear that opens node settings. The side panel holds the same settings body.
- Tasks are created from the node panel: an "Add task" line opens a sheet with title, due, priority, context, spec and success. A reminder is also what the UI labels a todo.
- A stage bar for each node and each task, and an Agents sheet opened from the toolbar badge, show progress. Both are fed by the record's `step` and the registry entry.
- Remote ssh nodes list their tasks read-only through the snapshot.
- The app hosts the agent runner in-process. It starts when setup is complete and stops on exit. `fabr agent` runs the same runner as a debug entry point.
- The frontend parses records in one place, `app/src/lib/fabric/parse.ts`, and tests against `fixtures/`.
