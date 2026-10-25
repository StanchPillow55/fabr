# Architecture

Fabric takes a line that was dumped into a folder and turns it into a task. An agent defines,
plans, runs and checks the task, and the user approves on Telegram. This file is the big picture. The blocks
themselves are in `COMPONENTS_CHANNELS_CORE_AGENTS.md` and `COMPONENTS_KNOWLEDGE_PRIVATE.md`.

## Status legend

Every block carries one of four statuses. The status says when it gets built, not how important
it is.

| Status | Meaning |
|---|---|
| foundation | built in wave 1, the first pour |
| placeholder | wave 1 creates the file, the slot and the name. The body comes later |
| parallel project | owned by the input project that runs alongside; only a file contract is shared |
| named, not built | drawn on the map so nobody fills the hole by accident; no wave assigned |

## Five layers

| Layer | Holds | Blocks |
|---|---|---|
| Channels | how the user touches it | First-run setup, Input capture, Local GUI, Telegram bot |
| Core | the concrete | Task record store, Task index, Orchestrator, Agent registry, Notifier |
| Agents | model workers, one job each | Organizer, Context gatherer, Definer, Planner, Executor, Evaluator, Learner |
| Knowledge | files that the agents read | Machine settings, Global rules and preferences, Project knowledge and preferences, Learned preferences, Past executions |
| Private data and tools | where values and side effects live | Personal data bank, Fill at the sink, Audit log, Tool set, Personal context connectors |

Counts: 25 blocks. 20 are foundation. 2 are placeholders: Learner and Learned preferences. 2
belong to the parallel project: Input capture and Organizer. 1 is named and not built: Personal
context connectors.

## How the layers connect

Everything above the knowledge layer reads and writes the task record in the folder. Everything
in the knowledge layer is read by the context gatherer and merged by precedence. The private
layer holds the only values that the agent never sees.

- The user reaches the system through the GUI and Telegram. Both write to the record and both
  can answer a pending question. First answer wins.
- The orchestrator owns the step order and calls the six agents in turn. It is the only thing
  that advances the record's state.
- The context gatherer is not a model. It walks the folder chain, merges knowledge and
  preferences, loads learned records and greps past records. Then it hands one bundle to the
  definer and the planner.
- The executor is the only agent with side effects. It reaches the world through the tool set,
  and the browser and HTTP tools carry the fill substitution.
- The data bank hands values to fill, never to an agent. Every fill lands in the audit log.
- The notifier fans events out to Telegram and the GUI.

## Edges

An edge means two blocks read or write each other's output. The full list lives on each block
under "talks to". The densest nodes are the task record store (read by five blocks) and the
orchestrator (talks to nine). That is by design: the record is the shared state and the
orchestrator is the only sequencer.
