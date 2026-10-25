# Wave 3: Trust and control

Goal: the agent can learn about the environment and act on riskier tasks without the user
losing the wheel.

## Exit test

1. A run discovers that a site needs login before search. The learner stages a proposal.
2. The user accepts it in the GUI. It lands in the project's knowledge file.
3. The next run on that site logs in first without asking.
4. An irreversible task stops at the checkpoint before its irreversible step and waits for a
   tap, even in auto mode.
5. A run that fails evaluation is re-planned once and succeeds on the second plan.

## Items

1. Procedure proposals: staged in the project's proposals file after a passed evaluation,
   accepted or rejected in the GUI, accepted ones appended to project knowledge. Never auto-
   accepted in this wave.
2. A learned rule is promoted to global after confirmation in two different projects, and the
   user is told.
3. Staleness review: always-rules carry a last-used date; the GUI nudges after a long gap.
4. A checkpoint before an irreversible step: the executor pauses, the user taps, the step runs.
5. Re-plan on a failed evaluation: one new plan, approved again, then a second execute and
   evaluate. The record links the attempts.
6. Registry: cost caps per task and per day, pause and resume, several agents per folder with
   file-level conflict detection.
7. Notifier: digest mode and quiet hours.
8. Long-form review in the GUI. A plan or an output longer than a screen is linked from
   Telegram and read at the desk.

## Definition of done

- No proposal reaches a knowledge file without a tap in the GUI.
- A proposal from a failed run is never staged.
- An irreversible step never runs while the record's waiting flag is set.
- Pausing a task and restarting the process leaves it paused.

## Depends on

Wave 2 for learned records and their GUI. Wave 1 for reversibility classes and the registry.
