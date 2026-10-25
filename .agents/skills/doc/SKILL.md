---
name: doc
description: >
  Design and philosophy of fabric, the folder-based system that turns a dumped reminder into a
  task an agent defines, plans, runs and checks. Holds the owner's founding input, the design
  principles, the design review with every concern and its resolution, the component map with
  all 25 blocks and their status, the task record and its five states, the nine-step delegate
  flow, the on-disk layout, the first-run setup, the four feature waves with a plan each, the frontend
  track's ownership split, file contract, screens and waves F0 to F5, the decisions log, and the
  interaction-surface spec for the agent backend under dev2/. Read it for anything about architecture, task lifecycle, Telegram, the
  personal data bank, grants, fill at the sink, learned preferences, waves, setup, or why a
  thing is the way it is. It does not describe the code.
---

# doc

This is the index of the project's agent knowledge. Open only the file whose row matches the
task. Each row names one file under `references/`, and every file there has a row. The same
material is published as an interactive map at https://claude.ai/artifact/WqqKKtBxHBizKY849YsSb8
(private to the owner); the files here are the source of record.

| File | Read it when |
|---|---|
| `references/USER_INPUT.md` | you need the owner's own words on what fabric is and must do, from the founding conversation of 2026-10-24 |
| `references/PHILOSOPHY.md` | deciding how to shape anything new, so it fits the twelve principles |
| `references/DESIGN_REVIEW.md` | a concern comes up that may already have been raised and settled, or you want the strengths the design rests on |
| `references/ARCHITECTURE.md` | you need the big picture: the five layers, the status legend, how the blocks connect |
| `references/COMPONENTS_CHANNELS_CORE_AGENTS.md` | working on setup, the GUI, Telegram, the task store, the index, the orchestrator, the registry, the notifier, or any of the six agents |
| `references/COMPONENTS_KNOWLEDGE_PRIVATE.md` | working on machine settings, rules and preferences, learned preferences, past executions, the data bank, fill at the sink, the audit log, the tool set, or connectors |
| `references/TASK_RECORD.md` | touching the task record, its fields, its five states, or the waiting flag |
| `references/DELEGATE_FLOW.md` | touching what happens after delegate is pressed: the nine steps, their gates, what each reads and writes |
| `references/ON_DISK.md` | deciding where a file lives, how the folder chain resolves, who may write which file |
| `references/SETUP.md` | working on the first-run wizard: the six screens, their checks, the two files they write |
| `references/FRONTEND_PLAN.md` | you work on the frontend track: what is ours versus dev 2's, the file contract between the tracks, the fixture strategy, the screens, and frontend waves F0 to F5 |
| `references/WAVES.md` | deciding whether a feature belongs now or later, and the rule that protects the foundation |
| `references/WAVE_1.md` | building the foundation: the ordered plan, each item's definition of done, the exit test |
| `references/WAVE_2.md` | building tell-it-once: learned preferences and the three-option prompt |
| `references/WAVE_3.md` | building trust and control: procedure proposals, checkpoints, cost caps |
| `references/WAVE_4.md` | building reach: speech and photo input, connectors, virtual cards |
| `references/DECISIONS.md` | changing anything a past decision covers |
| `dev2/agent_surface_spec.md` | anything that crosses between the agent backend and the frontend: record fields and writers, states and transitions, request flags, the question slot, approval, the registry, events, audit, policy, vault, settings, secrets, setup checks, Telegram message kinds, and the ten F0 fixtures. It wins over TASK_RECORD.md where they differ |
