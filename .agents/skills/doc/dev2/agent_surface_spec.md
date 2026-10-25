# Agent backend: interaction surface

This is the contract between the agent backend (dev 2) and the frontend (us). It defines only
what crosses the boundary: files, their fields, who writes each one, and the protocols on top of
them. It says nothing about how the backend works inside. Where this file and
`references/TASK_RECORD.md` differ, this file wins. Version 1.1, 2026-10-24.

Everything crosses as a file. The one exception is the setup checks, which cross as a local
command with JSON on stdout, because they need the network. The record front matter `schema`
is the integer `2`. A fractional record schema is not used. Task filenames stay
`YYYY-MM-DD-<slug>.md`.

## Contents

1. Paths and formats
2. The task record
3. States and who moves them
4. Protocols: requests, questions, approval, cancel
5. The registry
6. Events, audit, data bank files
7. Settings and secrets
8. Setup checks
9. Telegram messages that the backend sends
10. Fixtures (F0 exit test)
11. Conventions

## 1. Paths and formats

| Path | Format | Written by | Read by |
|---|---|---|---|
| `~/.fabric-space/settings.toml` | TOML | setup (GUI) | backend, GUI |
| `~/.fabric-space/secrets.toml` | TOML, mode 600 | setup (GUI) | backend only |
| `~/.fabric-space/registry/<task_id>--<path_tag>.yaml` | YAML | backend | GUI |
| `~/.fabric-space/events.log` | one line per event | backend | GUI, Telegram transport |
| `~/.fabric-space/data-bank/vault.toml` | TOML, mode 600 | GUI | fill at the sink only |
| `~/.fabric-space/data-bank/policy.toml` | TOML | GUI | backend |
| `~/.fabric-space/data-bank/audit.log` | one line per fill | backend | GUI |
| `~/.fabric-space/.agents/rules.md`, `preferences.toml` | markdown, TOML | GUI | backend |
| `~/.fabric-space/.agents/learned.yaml` | YAML | backend | GUI, backend |
| `<folder>/.fabric-space/tasks/<task_id>.md` | YAML front matter + markdown | both, by field | both |
| `<folder>/.fabric-space/.agents/knowledge.md`, `preferences.toml` | markdown, TOML | GUI | backend |
| `<folder>/.fabric-space/.agents/learned.yaml` | YAML | backend | GUI, backend |

Human-edited files are TOML or markdown. Machine-written files are YAML. Timestamps are ISO 8601
in UTC with a `Z` suffix. A `task_id` is `YYYY-MM-DD-<slug>`, slug from the title, lowercase,
hyphens, at most 40 characters, with `-2`, `-3` appended on collision inside the folder. The
record file is `<task_id>.md`. A task is identified across the system by its full path.
`path_tag`, used only in the registry filename, is defined in section 5. Events log lines still
use the full task path.

## 2. The task record

YAML front matter between `---` lines, then five markdown sections with these exact headings in
this order: `## Context`, `## Spec`, `## Success`, `## Plan`, `## Log`. Missing sections are
present and empty. The writer column says who may write the field. Nobody else does.

| Field | Type | Writer | Meaning |
|---|---|---|---|
| `schema` | int | creator | always `2` |
| `id` | string | creator | the task id |
| `source` | map or null | backend only | `{node_path, todo_id}`, or null when the record was not created from a todo. The GUI may display it and must not write it. `node_path` is the workspace-relative node path; an empty string is the workspace root. `todo_id` is the hex id on the `.fabr/.todo` line |
| `title` | string | user, organizer | one line |
| `due` | date or null | user, organizer | optional |
| `state` | enum | backend only | `reminder`, `task`, `defined`, `in_progress`, `succeeded`, `failed` |
| `step` | enum or null | backend | `gather`, `define`, `plan`, `approve`, `execute`, `evaluate`, `learn`, `notify`; null outside the pipeline |
| `waiting` | bool | backend | true while `question` has no answer or `approval.status` is `pending` |
| `created_at`, `updated_at` | timestamp | every writer updates `updated_at` | |
| `requests` | map | GUI and Telegram set true; backend clears to false | keys `delegate`, `cancel`, `redelegate` |
| `question` | map or null | backend creates; GUI or Telegram fills `answer` | see section 4 |
| `answers` | list | backend | every answered question, copied from the slot |
| `grants` | map | backend writes `requested`; GUI or Telegram write `granted` | `requested`: list of field paths; `granted`: list of `{field, scope}` with scope `task`, `project` or `always` |
| `plan_meta` | map or null | backend | `reversibility`: `reversible` or `irreversible`; `tools`: list; `assumptions`: list of `{rule_id, text}`; `checks`: list of `{cmd, args}`. `checks` is not a top-level field. The executor may run only that list, with no shell |
| `assertions` | list | backend | `{id, text}`; the checkable form of the success definition |
| `approval` | map or null | backend creates `{status: pending, digest}` and rewrites `digest`; GUI or Telegram set `status`, `by`, `at`, `note` | `status`: `pending`, `approved`, `refused`; `digest`: lowercase sha256 hex, absent while `approval` is null; `by`: `gui`, `telegram`, `auto`; `at`; `note` |
| `verdict` | map or null | backend | `result`: `pass` or `fail`; `at`; `items`: list of `{id, pass, evidence}` |
| `cost_usd` | number | backend | total model spend for this record |
| `links` | map | backend | `redelegated_from`, `redelegated_to`: task paths or null |

Sections:

- `## Context`: the user's or organizer's text. The gatherer appends a `### Gathered` subsection
  with the merged bundle and never edits above it.
- `## Spec`: written by the definer.
- `## Success`: the user's success definition as given. The checkable form is `assertions`.
- `## Plan`: written by the planner, numbered steps.
- `## Log`: append-only lines `- <timestamp> <step>: <message>`, written by the backend. The GUI
  never writes here.

`plan_meta.checks` is a list of `{cmd, args}` inside `plan_meta`. The approval digest covers it
with the rest of `plan_meta`. The executor may run only that list, and it does not use a shell.

A minimal record as the organizer or the GUI creates it:

```yaml
---
schema: 2
id: 2026-10-24-order-filament
source: null
title: order 2 kg of black PLA
due: 2026-10-28
state: reminder
step: null
waiting: false
created_at: 2026-10-24T18:02:11Z
updated_at: 2026-10-24T18:02:11Z
requests: {delegate: false, cancel: false, redelegate: false}
question: null
answers: []
grants: {requested: [], granted: []}
plan_meta: null
assertions: []
approval: null
verdict: null
cost_usd: 0
links: {redelegated_from: null, redelegated_to: null}
---
## Context
Same brand as last time if it is in stock.
## Spec
## Success
## Plan
## Log
```

## 3. States and who moves them

Only the backend writes `state`. Every transition is logged.

| From | To | Trigger | Precondition |
|---|---|---|---|
| reminder | task | `requests.delegate` true | folder lock free; otherwise the request stays true and the GUI shows "queued" |
| task | defined | definer done | `## Spec` non-empty, `assertions` non-empty, `grants.requested` set |
| defined | in_progress | `approval.status` approved | `approval.status` is `approved` and a recomputed digest equals `approval.digest`. If `plan_meta.reversibility` is irreversible, or any requested field is always-ask, or any field is under `payment`, `by` must not be `auto` |
| defined | defined | `approval.status` refused | note appended to log; `approval` reset to pending on the next plan |
| in_progress | succeeded | `verdict.result` pass | |
| in_progress | failed | `verdict.result` fail, or `requests.cancel` true | cancel logs `cancelled by user`, `verdict` stays null |
| failed | task | `requests.redelegate` true | a new record is created with `links.redelegated_from`; the old one gets `redelegated_to` |

`step` advances inside `task` and `in_progress`. After a crash the backend resumes at the step
after the last one logged.

**Todo projection.** This is not a record field. The backend writes the originating `.fabr/.todo`
line. States `task`, `defined`, and `in_progress` display as `wip`. `succeeded` displays as
`done` only after `verdict.result` is `pass`. `failed` leaves the line at `wip`. The model never
opens `.todo`.

If `source.todo_id` is missing from that node's `.todo`, or the registry `task_path` does not
exist, log and skip: do not delete the record, do not invent a new one, and do not mark
`succeeded`. A missing `task_path` after a move is that case. Moving the node directory moves
`.fabric-space` with it, so the link travels. The registry's absolute path is then stale until
the backend is pointed at the new folder. V1 reports that and does not search the disk. At most
one record in the node folder with that `todo_id` whose state is not `succeeded` or `failed`. A
second delegate returns the existing path. The state table still records redelegate as a new
record. V1 does not create a redelegated pair.

## 4. Protocols: requests, questions, approval, cancel

**Requests.** A frontend sets one key under `requests` to true. The backend acts, then sets it
back to false in the same write as the state change. If the request does not apply to the
current state, the backend sets it false and logs `ignored request <key> in state <state>`.

**Questions.** The backend writes the slot and sets `waiting: true`:

```yaml
question:
  id: q3
  kind: open            # open | choice | grant
  text: Which shop, and is a 1 kg spool acceptable if 2 kg is out?
  options: null         # for choice: [once, always, no]
  prior: null           # for choice: {text, source_task, last_confirmed}
  fields: null          # for grant: [address.line_1, address.zip]
  asked_at: 2026-10-24T18:03:40Z
  answer: null
```

A frontend fills `answer` only if it is null when read:

```yaml
  answer:
    value: "Prusament shop, 1 kg is fine"   # choice: once | always | no
    replacement: null                       # choice with value no: the new choice
    granted: null                           # grant: [{field, scope}]
    by: gui                                 # gui | telegram
    at: 2026-10-24T18:05:02Z
```

The backend copies the slot into `answers`, sets `question: null` and `waiting: false`. A second
writer that finds `answer` already set discards its own answer and shows the one that landed.

**Approval.** The backend writes `approval: {status: pending, digest: <hex>}` in the same update
as the bodies the digest covers, and sets `waiting: true`. A frontend writes `status`, `by`,
`at`, optional `note`, and may edit `grants.granted` before approving, including a field's
scope. A frontend does not write `digest`. In auto mode the backend writes `status: approved,
by: auto` itself and leaves `digest` unchanged. If the precondition in section 3 forbids that,
the approval waits like any other.

Any backend rewrite of `## Spec`, `## Success`, `## Plan`, `assertions`, or `plan_meta` resets
`approval` to `{status: pending, digest: <new>}` and clears `by`, `at`, and `note`.

**Approval digest.** `digest` is lowercase sha256 hex. It is absent while `approval` is null.
Normalize each of the Spec, Success, and Plan markdown section bodies. The body is the text
after that heading line and before the next heading line. Split on `\n`. Trim trailing spaces
and tabs on each line. Do not trim leading whitespace. If the body is empty or only whitespace
(spaces, tabs, and newlines), the normalized value is a single newline `\n`. Otherwise drop
trailing empty lines, join the remaining lines with `\n`, and end with exactly one `\n`: one
trailing newline, and no extra trailing blank line.

Build one JSON object. Insert the keys in alphabetical order: `assertions`, `plan`,
`plan_meta`, `spec`, `success`.

- `spec`, `success`, and `plan` are those normalized strings.
- `assertions` is the list as stored. Each object has keys in alphabetical order (`id`, then
  `text`). List order is unchanged.
- `plan_meta` is JSON null when null. Otherwise it is an object with keys sorted
  alphabetically at every level. Array order is unchanged. Absent keys are omitted. A check
  entry's keys are `args`, then `cmd`.

The digest is the lowercase hex sha256 of the UTF-8 JSON bytes, with no extra whitespace.
Separators are a comma and a colon, with no space after either. Use standard JSON escapes for
quotes, backslashes, and control characters. Do not escape `/`. Encode non-ASCII characters as
UTF-8, not as `\u` escapes. The preimage for empty sections, empty assertions, and null
`plan_meta` is exactly these bytes:

```
{"assertions":[],"plan":"\n","plan_meta":null,"spec":"\n","success":"\n"}
```

sha256 of those bytes: `b910c8602e80533538acd11d3a45cc84990e6430ff12d07f52c0a355edfb9d5a`.

**Cancel.** A frontend sets `requests.cancel: true`. The backend stops the executor within one
heartbeat interval, removes the registry file, sets `state: failed`, logs the reason, clears the
flag.

**Concurrent writes.** The writer holds an exclusive flock on `<folder>/.fabric-space/tasks.lock`
for the whole read, `updated_at` check, temp file, and rename. It reads the record, checks
`updated_at`, writes a temp file in the same folder, and renames it over the record. The lock
is what keeps a GUI write and a backend write from overlapping. If `updated_at` changed between
read and write, the writer re-reads and retries once, then stops and logs.

## 5. The registry

The GUI owns the agent status UI. The backend writes the registry and the events log and shows
nothing itself. One file per running task at
`~/.fabric-space/registry/<task_id>--<path_tag>.yaml`, created when execute starts and removed
when the task leaves `in_progress`.

`path_tag` is the first 8 lowercase hex characters of sha256 of the UTF-8 absolute task-record
path string. That string is the `task_path` value, with no trailing newline and no trailing
slash. YAML fields `task_id` and `task_path` stay. The GUI lists the registry directory and
reads `task_id` from the YAML. The filename stem is not the task id. Events log lines still use
the full task path.

```yaml
task_path: /home/me/3d/.fabric-space/tasks/2026-10-24-order-filament.md
task_id: 2026-10-24-order-filament
folder: /home/me/3d
step: execute
started_at: 2026-10-24T18:10:00Z
heartbeat_at: 2026-10-24T18:12:30Z
heartbeat_interval_s: 10
cost_usd: 0.42
log_tail:               # last 20 log lines
  - "2026-10-24T18:12:28Z execute: opened shop page"
question: null          # copy of the record's slot, or null
pid: 48213
```

An agent is dead when `heartbeat_at` is older than three intervals. The GUI marks it; the backend
removes stale files on start.

## 6. Events, audit, data bank files

**Events.** `~/.fabric-space/events.log`, append-only, one line per event:

```
2026-10-24T18:15:02Z waiting /home/me/3d/.fabric-space/tasks/2026-10-24-order-filament.md one question
2026-10-24T18:21:40Z done    /home/me/3d/.fabric-space/tasks/2026-10-24-order-filament.md 3 of 3 assertions passed; assumed: ship to home
```

Kinds: `waiting`, `done`, `failed`, `stuck`. `stuck` means a dead registry entry. Each line
names the task by its full path, not by the registry filename. The GUI keeps its own read
position. The Telegram transport sends each line once.

**Audit.** `~/.fabric-space/data-bank/audit.log`, one line per fill: timestamp, task path, field,
sink. Sink is `browser:<host>` or `http:<host>`. No values, ever.

**Policy.** `~/.fabric-space/data-bank/policy.toml`. Every known field appears, with a policy for
the field or its subtree. Missing means always-ask.

```toml
[fields]
"name" = "always-allow"
"address" = "always-allow"
"address.line_2" = "always-ask"
"payment" = "never"          # the backend refuses to fill; the executor hands off
```

**Vault.** `~/.fabric-space/data-bank/vault.toml`, mode 600, the same field paths with values.
Plain TOML in wave 1, written by the GUI; the backend swaps in encryption behind the same paths
later. No agent process loads this file. Only the fill substitution inside the browser and HTTP
tools reads it, and only for fields in `grants.granted` of the running task.

## 7. Settings and secrets

```toml
# settings.toml
default_folder = "/home/me"
model = "claude-sonnet-5-5"        # or claude-haiku-5-5
telegram_user_id = 123456789
auto_mode = false
question_budget = 5                # questions per define step
heartbeat_interval_s = 10

[setup]
completed_steps = ["folder", "api_key", "model", "telegram_bot", "telegram_link"]
completed = true
```

```toml
# secrets.toml, mode 600
anthropic_api_key = "..."
telegram_bot_token = "..."
```

Setup writes both. The backend reads both. It runs when `completed_steps` contains `folder`,
`api_key`, and `model`, and `anthropic_api_key` and `model` are present. `telegram_bot` and
`telegram_link` may be absent. This slice does not block on them, and it does not wait on
`setup.completed`.

## 8. Setup checks

The GUI owns every setup screen. The backend owns no setup UI. The six screens are the folder
picker, the API key entry, the model selector, Telegram bot, Telegram link, and done. This
slice does not block on the Telegram screens. The GUI runs these checks as a local command and
reads one JSON object from stdout. Exit code 0 means that the check passed. The command name
is the backend's choice; the subcommands and outputs are not.

| Subcommand | Does | Output on success |
|---|---|---|
| `check folder <path>` | path exists and is writable; creates `~/.fabric-space` | `{"ok": true}` |
| `check api-key` | one tiny request with the key from secrets | `{"ok": true}` |
| `check model <id>` | one one-line reply from that model | `{"ok": true, "reply": "..."}` |
| `check telegram-token` | bot API `getMe` | `{"ok": true, "bot_name": "..."}` |
| `telegram link` | waits up to 120 s for any message to the bot, replies "connected" with a Confirm button, waits for the tap | `{"ok": true, "user_id": 123456789}` |

On failure: `{"ok": false, "reason": "<one sentence the user can act on>"}` and exit code 1.
`check telegram-token` and `telegram link` stay for the Telegram screens. This slice does not
block backend start on them.

## 9. Telegram messages that the backend sends

The backend owns the transport. We draft the text and the button rows; the kinds and callbacks
below are fixed so both halves of the conversation match.

| Kind | When | Buttons | Callback writes |
|---|---|---|---|
| question, open | `question.kind` open | none; the next text reply is the answer | `answer.value` |
| question, choice | `question.kind` choice | `Once` `Always` `No, something else` | `answer.value`; `No` prompts for `replacement` |
| grant request | `question.kind` grant | `Allow for this task` `Review in GUI` | `answer.granted` with scope task |
| approval | `approval.status` pending | `Approve` `Refuse` `Open in GUI` | `approval.*`; `Refuse` prompts for `note` |
| notice | an events.log line | `Open in GUI` | nothing |

Every message carries the task title and a deep link to the GUI. Messages from any user id other
than `telegram_user_id` are ignored and not logged.

## 10. Fixtures (F0 exit test)

Ten fixtures, one folder each under `fixtures/` at the repo root, each a complete `.fabric-space`
tree. The backend's reader must load every record without error, and the GUI must render every
one. When either side changes the record, the fixtures change in the same commit.

| Fixture | Record state | Notable fields |
|---|---|---|
| `01-reminder` | reminder | only title, due, context |
| `02-open-question` | task, waiting | `question.kind` open, no answer |
| `03-choice-question` | task, waiting | `question.kind` choice with `prior` |
| `04-awaiting-approval` | defined, waiting | plan, 3 assertions, 2 requested grants, irreversible |
| `05-running` | in_progress | registry entry, `step` execute |
| `06-waiting-mid-run` | in_progress, waiting | `question.kind` grant, registry entry |
| `07-succeeded` | succeeded | full log, verdict pass, `cost_usd` |
| `08-failed` | failed | verdict with one failed item and evidence |
| `09-redelegated-pair` | failed and task | `links` both ways |
| `10-registry` | none | two live entries, one dead by heartbeat age |

## 11. Conventions

- A field that is not in section 2 is not written. New fields need a new `schema` number and a
  new version of this file.
- Nothing in any file under `~/.fabric-space` or `.fabric-space/` contains a bank value except
  `vault.toml`.
- The backend never deletes a task record. Cancel and failure leave the file in place.
- The GUI never writes `state`, `step`, `answers`, `assertions`, `verdict`, `plan_meta`,
  `cost_usd`, `links`, `## Log`, `source`, or `approval.digest`. It may still write
  `approval.status`, `approval.by`, `approval.at`, and `approval.note`.
