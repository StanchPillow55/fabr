# Feature waves

Four waves, poured in order. Wave 1 is the concrete. The rule that protects it: no later wave
changes a file format or adds a state. Each later wave only fills a slot that wave 1 already
left. When a feature cannot be built without changing a format, the format was wrong in wave 1.
That is a decision for the owner, not an edit made in passing.

| Wave | Name | Goal | Exit test | Plan |
|---|---|---|---|---|
| 1 | Foundation | one task goes from reminder to succeeded through Telegram, with a grant, a fill and an audit entry; a restart mid-run resumes | on a fresh machine, finish setup, then run one task end to end and survive a kill during execute | `WAVE_1.md` |
| 2 | Tell it once | a decision made in one task is offered back in the next, and the user can say once, always, or no | the same kind of task three times: asked, then always, then assumed and listed | `WAVE_2.md` |
| 3 | Trust and control | the agent can learn about the environment and act on riskier tasks without the user losing the wheel | a run proposes a procedure, the user accepts it in the GUI, the next run uses it; an irreversible task waits at a checkpoint | `WAVE_3.md` |
| 4 | Reach | more ways in, more of the user's world reachable, all behind the same grant model | a voice note becomes a reminder; a task reads granted emails and nothing else | `WAVE_4.md` |

## Where a feature goes

Ask in order. The first yes places it.

1. Does it change a file format or a state? Then it is wave 1, or it needs the owner.
2. Is it needed for one task to run end to end? Wave 1.
3. Does it make a second run better than the first? Wave 2.
4. Does it let the agent act with less supervision, or learn about the environment? Wave 3.
5. Does it add an input channel or a connector? Wave 4.

## Placeholders in wave 1

Two blocks are built as slots in wave 1 and filled later. The learner only captures answers in
wave 1. Learned preferences get the record format and the project-scope file in wave 1. Both
exist so the record and the files already carry their fields when wave 2 starts.
