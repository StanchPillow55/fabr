---
schema: 4
id: 2026-10-25-rotate-the-expiring-stripe-key
title: rotate the expiring Stripe key
due: 2026-10-26
pri: 3
state: in_progress
step: execute
waiting: false
paused: true
suspended: false
suspend_until: null
source: null
created_at: 2026-10-25T04:00:00Z
updated_at: 2026-10-25T08:30:00Z
requests:
  delegate: false
  cancel: false
  redelegate: false
  pause: false
  resume: false
question: null
answers: []
grants:
  requested: []
  granted: []
plan_meta:
  reversibility: irreversible
  tools:
  - browser
  - github
  assumptions:
  - rule_id: stripe-secret-repo
    text: The key lives only in the STRIPE_SECRET_KEY repository secret
  checks: []
assertions:
- id: a1
  text: The STRIPE_SECRET_KEY secret was updated after the new key was created
- id: a2
  text: A test charge on staging succeeded with the new key
- id: a3
  text: No key value appears in any log or record
approval:
  status: approved
  by: gui
  at: 2026-10-25T07:00:00Z
  note: null
  digest: 97705bfa2d3848e2b93d48e4a7b6075f4ce15e569b21e6075b1763e3604ec8ba
verdict: null
cost_usd: 0.21
links:
  redelegated_from: null
  redelegated_to: null
---
## Context
The live Stripe secret key expires tomorrow. Rotate it before the deploy window.

### Gathered
- Rules (home): Never fill payment details; stop and hand the last step to me. Ask before anything over $100.
- Folder knowledge (work): Stripe keys are rotated in the dashboard and stored as the STRIPE_SECRET_KEY repository secret.
## Spec
Create a replacement restricted key in the Stripe dashboard, store it as the STRIPE_SECRET_KEY repository secret, redeploy staging, run one test charge, then revoke the old key. Never print the key value in a log.
## Success
Staging runs on the new key, a test charge succeeds, and the old key is revoked.
## Plan
1. Create a restricted key named ci-2026-10 in the Stripe dashboard.
2. Store it as the STRIPE_SECRET_KEY repository secret.
3. Redeploy staging and run one test charge.
4. Revoke the old key.
## Log
- 2026-10-25T04:00:00Z gather: delegate accepted, folder lock taken
- 2026-10-25T04:01:00Z gather: context bundle written
- 2026-10-25T04:06:00Z define: spec and 3 assertions written, 0 grants requested
- 2026-10-25T04:14:00Z plan: plan written, irreversible, tools: browser, github
- 2026-10-25T04:15:00Z approve: approval pending, message sent
- 2026-10-25T07:00:00Z approve: approved by gui
- 2026-10-25T07:02:00Z execute: started, registry entry created
- 2026-10-25T07:08:00Z execute: tool browser: opened the Stripe dashboard, API keys page
- 2026-10-25T07:16:00Z execute: tool browser: created restricted key ci-2026-10
- 2026-10-25T08:26:00Z execute: tool github: updated the STRIPE_SECRET_KEY secret, ok
- 2026-10-25T08:30:00Z execute: paused by user
