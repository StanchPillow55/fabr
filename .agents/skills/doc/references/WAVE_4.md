# Wave 4: Reach

Goal: more ways in, more of the user's world reachable, all behind the same grant model. This
wave builds the blocks that wave 1 named and did not build.

## Exit test

1. A voice note sent to the bot becomes a reminder in the right folder.
2. A task reads "emails from X" through a granted connector. The agent's transcript contains
   those emails and nothing else from the mailbox.
3. A purchase completes with a virtual card, and the card number never appears in any
   transcript, screenshot or log.

## Items

1. Speech and photo input, handed to the organizer like text. This belongs to the parallel
   project; fabric only has to accept the reminder.
2. Personal context connectors: email, calendar, messages. Each is a read-only tool behind its
   own grant class. The grant states the filter, such as sender or date range, so the agent
   never sees outside it.
3. Virtual cards for payment, issued per task, so the payment refusal from wave 1 can be lifted
   for small amounts the user pre-approves.
4. Screenshot and read-back redaction in the browser tool, so a filled value does not come back
   into the transcript through the page.
5. Several machines sharing one global layer: the machine root stays local; the global agents
   folder and the data bank policy sync.
6. Embeddings over past records, only if the full-text search from wave 2 stops being enough.

## Definition of done

- A connector grant without a filter is refused by the policy.
- A redacted screenshot shows the form with the filled fields masked.
- The audit log records connector reads the same way it records fills.

## Depends on

Wave 1 for the grant model and the tool set slots. Wave 3 for checkpoints, since payment is
irreversible.
