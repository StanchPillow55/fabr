# First-run setup

A wizard that runs on first launch and again whenever a step is missing. It shows one step,
waits until that step's check passes, then shows the next. Progress is saved in the settings
file, so quitting halfway resumes at the open step. Secrets are written once, masked afterwards,
and never echoed back. Each screen has a back button.

## The six screens

| # | Screen | What the user does | Check that must pass | Written |
|---|---|---|---|---|
| 1 | Default folder | picks where their projects live; `~/projects` is suggested | the folder exists and is writable | `default_folder` in settings; `~/.fabric-space/` is created |
| 2 | Anthropic API key | pastes the key | one tiny request succeeds | the key in secrets, mode 600, shown masked from then on |
| 3 | Model | picks Sonnet (suggested) or Haiku; only these two are offered | a one-line reply from the chosen model | `model` in settings |
| 4 | Telegram bot | opens BotFather in Telegram, sends `/newbot`, answers its two prompts, pastes the token | the bot API returns the bot's name | the token in secrets |
| 5 | Telegram link | opens the new bot and sends it any message | the user taps Confirm on the bot's "connected" reply | `telegram_user_id` in settings; the bot is locked to it |
| 6 | Done | reads a summary of the five values | none | nothing; a button opens the tree on the default folder |

## Model ids

The two models offered, with the exact ids that the API expects. Do not add a date suffix.

| Choice | Id |
|---|---|
| Sonnet (suggested) | `claude-sonnet-5-5` |
| Haiku | `claude-haiku-5-5` |

## Why one step at a time

The owner asked for it, and it matches how the checks work. Each step has one thing to verify.
A failed check keeps the user on the screen with the reason. Showing all five inputs at once
would make the Telegram part confusing. Step 5 cannot start until step 4's token is verified
and the bot is live.

## Later

Re-run any single step from the GUI settings page. More models when they are available.
Several machines sharing one global layer is wave 4.
