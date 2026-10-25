# Components: knowledge, private data and tools

Each block has its status, what it is, what it owns, what wave 1 builds, what comes later, and
which blocks it talks to. The channels, core and agents are in
`COMPONENTS_CHANNELS_CORE_AGENTS.md`.

## 1. Knowledge

### Machine settings (foundation)
What exists on this box: the tool inventory, the Telegram user id, the auto-mode default, the
procedures. Orthogonal to preferences, so it never conflicts with them. Lives in
`~/.fabric-space/`.
- Owns: the settings file; the secrets file (mode 600, never loaded into an agent context);
  the procedures folder.
- Wave 1: the settings and secrets files, their loader, and the setup wizard that fills them.
- Later: several machines sharing one global layer (wave 4).
- Talks to: Context gatherer, Orchestrator, First-run setup.

### Global rules and preferences (foundation)
Prose rules in a markdown file, structured preferences in a TOML file. People edit both from
the GUI. The agent never writes here. Lives in `~/.fabric-space/.agents/`.
- Owns: `rules.md`, `preferences.toml`.
- Wave 1: the two files and a GUI editor with a textarea and typed fields.
- Later: templates.
- Talks to: Context gatherer, Local GUI.

### Project knowledge and preferences (foundation)
Per-folder knowledge and preference overrides, in the same two file shapes as global. Nearer
folder wins. This is where the dot_agents conventions plug in. Lives in
`<dir>/.fabric-space/.agents/`.
- Owns: `knowledge.md`, `preferences.toml`.
- Wave 1: the same loader as global, with the chain walk.
- Later: procedure proposals landing here after acceptance (wave 3).
- Talks to: Context gatherer, Local GUI, Learner.

### Learned preferences (placeholder)
Structured records, never prose: question, answer, applies-when, scope, confirmation count,
source tasks, last confirmed. Only always-rules are applied silently. Everything else is
re-asked with the three options until promoted. A separate file from the hand-written ones.
- Owns: `learned.yaml` at project and global scope.
- Wave 1: the record format, the project-scope file, read by the gatherer, listed in the GUI.
- Later: promote, demote, the conflict rule (specific scope wins), staleness nudges (waves 2
  and 3).
- Talks to: Context gatherer, Learner, Local GUI.

### Past executions (foundation)
Not a new store. Finished task records are the memory. The gatherer greps them for titles, tags
and answers.
- Owns: nothing new.
- Wave 1: grep.
- Later: index-backed search (wave 2).
- Talks to: Task record store, Context gatherer.

## 2. Private data and tools

### Personal data bank (foundation)
Hierarchical fields like `name.first` and `address.zip`. A policy marks each field or subtree
always-allow or always-ask. A task gets a grant list. The agent only ever holds field names,
never values. The crypto is a boring file; the policy and the grants are the product.
- Owns: the encrypted vault, the policy file, grants per task.
- Wave 1: the field tree, the policy, a grant request as one message with one-tap approve, the
  allow-for-this-task scope.
- Later: the allow-for-this-project scope, revocation (wave 2), real key management.
- Talks to: Fill at the sink, Definer, Local GUI, Audit log.

### Fill at the sink (foundation)
The agent writes `{{address.zip}}`. The browser driver or the HTTP client substitutes the value
at the moment of use and only if a grant exists. Payment fields are never filled: the executor
stops and hands the last step to the human. Read-back leaks are best effort and logged.
- Owns: the substitution inside trusted tools.
- Wave 1: substitution in the browser driver and the HTTP client, the grant check, the payment
  refusal.
- Later: virtual cards, screenshot and read-back redaction (wave 4).
- Talks to: Personal data bank, Tool set, Audit log.

### Audit log (foundation)
Append-only: every fill with task, field, sink and time. Visible in the GUI. The demo moment
for the privacy story.
- Owns: the audit file.
- Wave 1: append and list.
- Later: export.
- Talks to: Fill at the sink, Local GUI.

### Tool set (foundation)
The executor's hands. Five tools: the filesystem inside the folder, a shell, a browser driver,
an HTTP client, and an ask-user tool. The ask-user tool routes through the orchestrator to
Telegram. The browser and
HTTP tools carry the fill substitution.
- Owns: the tool definitions.
- Wave 1: the five tools above.
- Later: more connectors.
- Talks to: Executor, Fill at the sink, Personal context connectors.

### Personal context connectors (named, not built)
Tasks that need "emails from X" or "what did I do on date Y". Named in the architecture,
intentionally not implemented. They would be read-only tools with their own grant class.
- Owns: nothing yet.
- Wave 1: a slot in the tool set and a grant class name.
- Later: email, calendar, messages, each as a read-only tool behind a grant (wave 4).
- Talks to: Tool set.
