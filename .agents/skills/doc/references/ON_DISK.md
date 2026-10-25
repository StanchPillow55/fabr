# On disk

The filetree is the database. Two roots: the machine root under the home folder, and a
`.fabric-space/` folder in any project folder. Paths below use the product name fabric; the
code still says fabr until the rename lands.

## The machine root

```
~/.fabric-space/                # machine level: what exists on this box
  settings.toml                 # default_folder, model, telegram_user_id, auto-mode default, setup progress
  secrets.toml                  # mode 600: API key, bot token; written by setup, never loaded into an agent context
  procedures/                   # how things are done on this machine
  .agents/                      # global agent layer, dot_agents shape
    rules.md                    # hand-written prose, GUI textarea
    preferences.toml            # hand-written structured: budgets, allowed tools, ask or auto
    learned.yaml                # agent-written records; GUI lists, promotes, deletes
  data-bank/
    vault.age                   # encrypted fields: name.first, address.zip, payment.* and so on
    policy.toml                 # per field or subtree: always-allow or always-ask
    audit.log                   # every fill: task, field, sink, time
  registry/                     # live agents, heartbeat files, runtime only
  index.sqlite                  # derived cache over all task records; safe to delete
```

## A project folder

```
~/any/project/folder/
  .fabric-space/
    tasks/
      2026-10-24-order-filament.md   # front matter: state, due, grants, pending question
                                     # sections: context, spec, success, plan, log
    .agents/
      knowledge.md                   # per-folder knowledge, dot_agents shape
      preferences.toml               # project overrides of the global file
      learned.yaml                   # project-scope learned preferences
      proposals.yaml                 # staged procedural learnings, accepted in the GUI (wave 3)
```

## Resolution order

Walk from the task's folder up to the root and collect every `.fabric-space/.agents`. The
nearer folder wins over the farther one. Project wins over global. Machine settings are a
different kind of thing: they say what exists here, not how to behave, so they never conflict
with preferences.

## Two writers, two files

People edit `rules.md` and `preferences.toml`. The agent writes only `learned.yaml` and
`proposals.yaml`. No file has both writers, so nothing gets clobbered.

## Files are truth

The task record in the folder is the only source of truth. Telegram, the GUI, the registry and
the index all read and write that file. The sqlite index is a cache and can be rebuilt from the
tree at any time. The registry holds runtime entries only and is empty after a clean stop.

## Setup writes two files

The wizard fills `settings.toml` and `secrets.toml` one step at a time and records which steps
passed. The default folder is where the tree opens and where the first `.fabric-space` goes.

## Contract with the input project

The organizer writes a task record with state reminder and at least a title. Everything else is
optional. Fabric owns every state after that. The exact file format is still to be agreed with
the parallel project; until then, this file and `TASK_RECORD.md` describe the intended shape.
