# Frontend subagent schedule

The launch order for the agents that build and verify the frontend track. Each step has a Sonnet
implementer and a separate Sonnet verifier. The definitions of done are in `FRONTEND_EXECUTION.md`;
this file only says who runs when. Status is kept in the owner's session, not here.

## 1. F1, agentic execution

Each step has a Sonnet implementer (A) and a separate Sonnet verifier (V) that checks the
step's definition of done, runs the app's check and test commands, and confirms that nothing
outside the fabric module changed except wiring. Two lanes run in parallel.

| Agent | Lane | Builds | Starts after |
|---|---|---|---|
| A1 | screens | types, parser, fixture source, read-only record screen, fixture picker, parser tests | now |
| A2 | screens | board by state, tree with counts, both opening the record screen | A1 reports |
| A3 | data | Rust raw-file commands (list, read, write with updated_at check, stamp), registry and events reads, Tauri source, HTTP route, mock entries | A1 reports |
| A4 | screens | question panel, three kinds, writes through the source | V2 passes |
| A5 | screens | approval panel with scope selectors, approve and refuse | A4 reports |
| A6 | screens | request actions: delegate, cancel, re-delegate, queued state | A5 reports |
| A7 | data | agents screen over the registry, dead marking, cancel | A3 reports |
| A8 | data | notices badge and list from the events log | A3 reports |
| A9 | both | live watching through the Tauri source and stamp polling; also: registry and events reads must not walk the home folder for records, and the stamp should include a content hash or sub-millisecond guard | A3, A6, A7, A8 verified |
| V10 | both | integration: all eight definitions of done, check and test green, isolation review | A9 verified |

A verifier runs while the next implementer starts. A failed verification spawns a fix-up agent
before the lane moves on. Budget: 9 implementers, 10 verifiers, 3 fix-ups.

Human points: the owner reviews the visual direction after A2. Dev 2 checkpoints at A3, their
reader against our file commands, and at A9, the first real record rendering unchanged.

## 2. F2 and the M1 ship

Same pairing as F1: an implementer (B) and a verifier (VB) per step. The Rust work goes first
because every screen's check calls it.

| Agent | Builds | Starts after |
|---|---|---|
| B1 | Rust: settings and secrets commands (read, write, mode 600, setup progress); the HTTP client crate; the five checks from spec section 8 as core functions, exposed both as app commands for the GUI and as `fabr check <sub>` and `fabr telegram link` CLI subcommands with the spec's JSON on stdout; the Telegram link listener with its Confirm button; tests against a local fake HTTP server for the Anthropic and Telegram endpoints | V10 passes |
| B2 | Setup wizard: shell with one screen at a time, back, resume from the completed-steps list; the six screens with default folder `~/`; a SetupSource interface with a stub in mock mode and the B1 commands otherwise; the key masked after entry | B1 reports |
| B3 | Wire `default_folder` from settings into the fabric view's roots, replacing the home-folder walk; gate the app on setup so a fresh machine opens on screen 1; the desktop defaults to the home `.fabric-space` as machine root | B2 reports |
| B4 | Ship: a desktop build, a check that a fresh config opens on setup, a check that the fixture records show when `default_folder` points at a fixtures copy, no developer wording on any screen | VB3 passes |
| VM1 | M1 verifier: the whole M1 definition from section 9 on a clean config, with a real key and bot where the owner provides them, otherwise against the fake server | B4 reports |

Budget: 4 implementers, 5 verifiers, 2 fix-ups. The checks are implemented in our core as the
placeholder for dev 2's command; the CLI subcommands keep the spec's shape so swapping theirs in
later changes no screen.
