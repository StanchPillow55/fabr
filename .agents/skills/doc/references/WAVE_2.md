# Wave 2: Tell it once

Goal: a decision made in one task is offered back in the next, and the user can say once,
always, or no. This wave fills the learner and learned preferences slots that wave 1 left.

## Exit test

1. Run a task that asks one open question and answer it.
2. Run the same kind of task again. The definer asks the three-option question for that choice,
   naming the earlier task.
3. Tap always.
4. Run it a third time. Nothing is asked; the approval message lists the choice under
   assumptions.
5. In the GUI, demote the rule. The fourth run asks again.

## Items

1. Learned records are read by the gatherer and shown to the definer as priors, with their
   applies-when condition and confirmation count.
2. The three-option prompt is wired end to end. Once increments the count. Always promotes to a
   rule at project scope. No asks whether to replace the record or add a new one for this case.
3. Always-rules are applied silently, and every one a plan leans on is listed in the approval
   message and in the notice.
4. The GUI manages learned records: list, promote, demote, delete, with source tasks and last
   confirmed date visible.
5. The conflict rule: when project and global records disagree, the nearer scope wins.
6. Grant scope allow-for-this-project, and revocation of any grant from the GUI.
7. Full-text search over past records from the index, replacing grep in the gatherer.
8. Per task-type checklists for the definer, so "defined enough" is sharper for common tasks.

## Definition of done

- A prior is offered only when the applies-when condition matches; a different kind of task is
  not asked about it.
- A prior is never applied without a tap unless it is an always-rule.
- The learned file never contains a value that belongs in the bank.
- The hand-written preference files are byte-identical before and after this wave's runs.

## Depends on

Wave 1 complete: the record's answers field, the project learned file, the gatherer and the
definer's prompt shape.
