# Frontend execution plan

The ordered plan for our track. `FRONTEND_PLAN.md` says what is ours and why. This file says
what we do first, what done means for each step, and where we need dev 2. Steps inside a wave
depend only on the steps above them. Everything is built against the fixtures from
`../dev2/agent_surface_spec.md`, section 10, until real records exist.

## Contents

1. Now: F0
2. F1, agentic execution
3. F2, setup
4. F3, settings and privacy
5. Checkpoints with dev 2
6. How we split the work
7. Open with the owner
8. Milestone M1: the shippable shell

## 1. Now: F0

1. Dev 2 reads the surface spec and either signs it or marks the lines they want changed. Nothing
   below starts on a disputed field. Done 2026-10-24: version 1 agreed as written.
2. Write the ten fixtures as real files under `fixtures/`, one `.fabric-space` tree each, exactly
   as the spec describes them. Done: every record has every field in section 2 of the spec. The
   two question fixtures and the approval fixture have realistic text that a reviewer can judge.
3. Draft the Telegram message templates: the text and button rows for the five kinds in section 9
   of the spec. Every template carries the task title and the GUI link. Done: dev 2 has the file,
   and no template needs a field that the record does not have.
4. Pick the demo task for the wave 1 exit test and write it as fixture 4. The approval screen is
   then designed around the real thing.

Steps 2 to 4 were done on 2026-10-24: the fixtures are under `fixtures/`, the templates are in
`../dev2/telegram_templates.md`, and the demo task is the filament order. F0 closes when dev 2's
reader loads all ten fixtures without error.

Two gaps in spec version 1 came out of writing the fixtures and are proposed for version 1.1:

- In YAML, a bare `no` is the boolean false. The choice options and the answer value must be
  written as the string `"no"`, and the reader must treat it as a string.
- The approval row in section 9 says the Telegram Approve button writes `approval.*` but not what
  it writes to `grants.granted`. Proposed: every requested field at scope `task`; anything else
  goes through "Open in GUI".
- Writes replace the whole record file by a temp file and rename. A reader should reopen the file
  on each read rather than hold a handle, and should expect the mtime to jump.
- The GUI shows a stale-write conflict and offers a reload instead of retrying once silently, so a
  user's answer is never re-applied onto a record that changed under them.
- A backend-presence signal: a heartbeat file at `~/.fabric-space/registry/_backend.yaml` with
  `heartbeat_at`, so the GUI can show connected or not connected.
- The events log is space-separated, so a task path with a space mis-parses. Proposed: fields
  separated by a tab, or a rule that task paths contain no spaces.

## 2. F1, agentic execution

Build order and definition of done.

1. Task record screen, read-only. It shows the title and the state ribbon with the five states
   and the waiting mark. It shows every section with its filled text or an empty slot, then the
   log and the verdict per assertion. Done: all ten fixtures render. A reviewer can tell from the
   ribbon and the empty slots which step the task has reached, without reading the log.
2. Board and Tree. The board groups every record under the root by state and marks waiting ones.
   The tree shows folders with counts by state. Done: fixtures 1 to 9 appear in the right column,
   and opening any row lands on its record screen.
3. Question panel on the record screen. Three kinds. Open text. The three choices, with the prior
   shown and a text field behind "no". The grant request as a field tree with a scope per field.
   Done: answering writes exactly the `answer` fields that the spec allows. A second answer is
   refused. The panel disappears when the slot is cleared on disk.
4. Approval panel. Plan, assertions, requested grants with a scope selector, reversibility, tools,
   assumptions. Approve in one tap, refuse with a note. Done: the write touches only `approval`
   and `grants.granted`; an irreversible fixture shows its warning.
5. Request actions. Delegate on a reminder, cancel while in progress, re-delegate on a failure.
   Done: each sets its flag and nothing else, and the screen shows "queued" until the flag is
   cleared on disk.
6. Agents screen over the registry. Task, folder, step, elapsed, cost, last log lines, pending
   question, with dead agents marked by heartbeat age. Done: fixture 10 shows two live and one
   dead, and cancel from here sets the flag on the right record.
7. Notices. A badge and a list fed by the events log, with a read position kept per viewer.
   Done: appending a line to the log by hand raises the badge within a second.
8. File watching across all of the above. Done: editing any fixture by hand re-renders the open
   screen within a second with no reload.

F1 is done when the eight points above pass against the fixtures, and again against the first
record that dev 2's orchestrator writes for real.

Status 2026-10-25: all eight points verified against the fixtures and in serve mode, plus a
fix-up for real-path folder grouping, wording and the notices read position. The check against
dev 2's first real record is still open.

## 3. F2, setup

1. The wizard shell: one screen at a time, a back button, resume from the completed steps list in
   the settings file. Done: quitting on screen 3 and reopening lands on screen 3.
2. The six screens with a stub check command that returns success or a chosen failure. Default
   folder `~/`. Done: a stubbed failure keeps the user on the screen and shows the reason. The
   Telegram link screen shows the waiting state and then the confirmed user id.
3. Writing the two files: settings with the completed steps, secrets with mode 600 and the key
   masked on every screen after entry. Done: the files match section 7 of the spec byte for byte
   on the fields setup owns.
4. Swap the stub for dev 2's check command when it exists. Done: a real key and a real bot pass
   all five checks on a clean machine.

## 4. F3, settings and privacy

1. Rules and preferences editors, global and per folder. The prose file gets a textarea and the
   structured file gets typed fields. Done: saving one file leaves the other byte-identical.
2. Data bank policy table over the field tree, with always-allow, always-ask and never per field
   or subtree, and the vault editor for values, mode 600. Done: a policy change is visible in the
   policy file; no value ever appears outside the vault.
3. Audit view, newest first, filter by task and field. Done: a line appended by hand appears.

F1 to F3 together are our half of product wave 1. F4 and F5 follow `FRONTEND_PLAN.md` and are
not scheduled until wave 1's exit test passes.

## 5. Checkpoints with dev 2

| When | Checkpoint | Both sides bring |
|---|---|---|
| end of F0 | spec signed; fixtures load in their reader | the fixtures, their reader's output |
| F1 step 1 | the first real record from the orchestrator renders unchanged in our record screen | one real record, our screen |
| F1 step 5 | a flag we set is picked up and cleared by their orchestrator | a running backend, our GUI |
| F2 step 4 | their check command replaces our stub | the command, our wizard |
| after F3 | the wave 1 exit test, together, on a fresh machine | everything |

## 6. How we split the work

The owner decides the stack, the visual direction and what ships. The agent in the session
drafts fixtures, templates, screen contents and copy, and reviews each screen against the
fixtures and the spec. Any change to a shared file goes through the spec first, with a new
version number, never through a quick edit on one side.

## 7. Open with the owner
8. Milestone M1: the shippable shell

- Spec version 1.1 with the two gaps above, to agree with dev 2.

- The visual direction. The stack was decided on 2026-10-24: the existing app, isolated module.
- The demo task for fixture 4 and the exit test.
- Whether F3 ships before or after dev 2's first end-to-end run.

## 8. Milestone M1: the shippable shell

The owner's first stopping point, named on 2026-10-24: the UI is wired, the API key and Telegram
setup work for real, and no agent backend is integrated. The build ships with placeholders where
the backend would be, so the setup can be tested end to end on a fresh machine.

M1 is reached when all three blocks below are verified.

| Block | What has to be true |
|---|---|
| F1 complete | every F1 step verified, V10 passed: screens, writes, live watching |
| F2 with real checks | the six setup screens; the five checks from the spec's section 8 implemented in our own Rust core as the placeholder for dev 2's command, same subcommands and JSON shapes, so swapping in theirs later changes nothing in the screens |
| Ship | a backend-presence placeholder so the GUI says "agent backend not connected" rather than showing flags that never clear; a desktop build that opens on setup on a fresh machine |

What a user can do at M1: install, enter a real API key, pick a model, create and link a bot, see
the fixture records on the board and tree, answer a question, approve a plan, and watch the
request flags stay queued because nothing picks them up. F3 is not part of M1.

Decisions M1 needed, both approved by the owner on 2026-10-24: one HTTP client crate for the Rust core, because the real
checks call Anthropic and Telegram; and a backend-presence signal for spec version 1.1, proposed
as a heartbeat file at `~/.fabric-space/registry/_backend.yaml`. Until dev 2 agrees, the
placeholder treats a request flag uncleared for a minute as "no backend".
