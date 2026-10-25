# Telegram message templates

Drafts for the five message kinds in `agent_surface_spec.md`, section 9. The backend owns the
transport; this file owns the text and the button rows. Placeholders in {braces} name a record
field, an events.log value or the GUI link. Nothing else is used.

Every message starts with the title line `{title}` and ends with `{gui_link}`, the deep link to
the task in the GUI. A bank value never appears in any message. Messages from a user id other than
`telegram_user_id` are ignored.

Values: `{title}`, `{question.text}`, `{question.options}`, `{question.prior.text}`,
`{question.fields}`, `{plan}` (the `## Plan` section), `{assertions}` (each `text`),
`{grants.requested}`, `{plan_meta.reversibility}`, `{plan_meta.tools}`, `{plan_meta.assumptions}`
(each `text`), `{event.text}` (the free text of the events.log line).

## 1. Question, open

Sent when `question.kind` is open.
```
{title}
Question: {question.text}
Reply to this message with your answer.
{gui_link}
```

Buttons: none. Spec: "none; the next text reply is the answer". The reply goes to `answer.value`.

## 2. Question, choice

Sent when `question.kind` is choice.
```
{title}
{question.text}
Last time: {question.prior.text}
{gui_link}
```

Buttons, one row: `Once` `Always` `No, something else`.

| Button | Writes |
|---|---|
| Once | `answer.value: once` |
| Always | `answer.value: always` |
| No, something else | `answer.value: "no"`, then the follow-up below fills `answer.replacement` |

Spec: "`answer.value`; `No` prompts for `replacement`".

## 3. Grant request

Sent when `question.kind` is grant.
```
{title}
{question.text}
Fields: {question.fields}
{gui_link}
```

Buttons, one row: `Allow for this task` `Review in GUI`.

| Button | Writes |
|---|---|
| Allow for this task | `answer.granted`: one `{field, scope: task}` per entry of `question.fields` |
| Review in GUI | nothing; opens `{gui_link}` |

Spec: "`answer.granted` with scope task".

## 4. Approval

Sent when `approval.status` is pending.
```
{title}
Plan:
{plan}
Checks: {assertions}
Needs: {grants.requested}
Tools: {plan_meta.tools}
Assumed: {plan_meta.assumptions}
Reversible: {plan_meta.reversibility}
{gui_link}
```

When `plan_meta.reversibility` is irreversible, add the line `This cannot be undone.` after
`Reversible:`. If the text passes Telegram's 4096 characters, drop `{plan}`; the GUI shows the full plan.

Buttons, one row: `Approve` `Refuse` `Open in GUI`.

| Button | Writes |
|---|---|
| Approve | `approval.status: approved`, `by: telegram`, `at` |
| Refuse | `approval.status: refused`, `by: telegram`, `at`, and `approval.note` from the follow-up |
| Open in GUI | nothing; opens `{gui_link}` |

Spec: "`approval.*`; `Refuse` prompts for `note`".

## 5. Notice

Sent once for each events.log line. All four kinds have one button, `Open in GUI`, which writes
nothing (spec: "nothing").

| Event kind | Sent when | Text |
|---|---|---|
| waiting | the agent waits on the user | `{title}` / `Waiting on you: {event.text}` / `{gui_link}` |
| done | `verdict.result` is pass | `{title}` / `Done: {event.text}` / `{gui_link}` |
| failed | `verdict.result` is fail, or cancel | `{title}` / `Failed: {event.text}` / `{gui_link}` |
| stuck | a registry entry is dead | `{title}` / `Stuck: {event.text}` / `{gui_link}` |

## Follow-up prompts

Each follow-up is one plain text message sent after the button tap. The next text reply from
`telegram_user_id` is the answer. No button row.

- After "No, something else": `What should I do instead of: {question.prior.text}?` The reply
  goes to `answer.replacement`; `answer.value` is "no".
- After "Refuse": `Why refuse? Your note goes into the log.` The reply goes to `approval.note`.

Until the reply arrives, `waiting` stays true. The first answer from either channel wins; a late
reply is discarded and the user gets `Already answered in the GUI.`
