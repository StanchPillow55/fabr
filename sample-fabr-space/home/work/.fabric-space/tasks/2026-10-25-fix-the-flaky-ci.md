---
schema: 4
id: 2026-10-25-fix-the-flaky-ci
title: fix the flaky CI
due: 2026-10-27
pri: 2
state: in_progress
step: execute
waiting: false
paused: false
suspended: false
suspend_until: null
source: null
created_at: 2026-10-25T05:40:00Z
updated_at: 2026-10-25T08:57:00Z
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
  reversibility: reversible
  tools:
  - shell
  - github
  assumptions:
  - rule_id: ci-github-actions
    text: CI is GitHub Actions, workflow ci.yml
  checks: []
assertions:
- id: a1
  text: tests/test_checkout.py passes 50 repeated runs
- id: a2
  text: No workflow file under .github/workflows is changed
- id: a3
  text: A draft pull request is open and not merged
approval:
  status: approved
  by: gui
  at: 2026-10-25T06:10:00Z
  note: null
  digest: d749d48b9c35876fc960392ae1203b6f15a5c216251edde935c726131fe9f677
verdict: null
cost_usd: 0.4
links:
  redelegated_from: null
  redelegated_to: null
---
## Context
The ci.yml run fails about one time in five on test_checkout_retry. Find out why and make it stable.

### Gathered
- Rules (home): Never fill payment details; stop and hand the last step to me. Ask before anything over $100.
- Folder knowledge (work): CI is GitHub Actions; the main workflow is .github/workflows/ci.yml. Releases are vX.Y.Z tags on main; a tag push runs release.yml.
## Spec
Find the cause of the intermittent failure in test_checkout_retry, fix it in the test or the code under test, and open a draft pull request. Do not merge and do not touch the release workflow.
## Success
test_checkout_retry passes 50 runs in a row and a draft pull request is open.
## Plan
1. List the last 30 ci.yml runs and read the failing logs.
2. Reproduce the failure locally by repeating the test.
3. Fix the race between the test and the mock payment server.
4. Repeat the test 50 times, then open a draft pull request.
## Log
- 2026-10-25T05:40:00Z gather: delegate accepted, folder lock taken
- 2026-10-25T05:41:00Z gather: context bundle written
- 2026-10-25T05:44:00Z define: spec and 3 assertions written, 0 grants requested
- 2026-10-25T05:50:00Z plan: plan written, reversible, tools: shell, github
- 2026-10-25T05:51:00Z approve: approval pending, message sent
- 2026-10-25T06:10:00Z approve: approved by gui
- 2026-10-25T06:15:00Z execute: started, registry entry created
- 2026-10-25T06:20:00Z execute: tool shell: list the last 30 ci.yml runs
- 2026-10-25T06:28:00Z execute: tool shell: read the failed log of run 11482
- 2026-10-25T06:40:00Z execute: tool files: read tests/test_checkout.py
- 2026-10-25T06:52:00Z execute: tool shell: pytest tests/test_checkout.py -k retry --count 20, 3 failed
- 2026-10-25T07:05:00Z execute: tool files: edited tests/test_checkout.py to wait for the mock server
- 2026-10-25T07:18:00Z execute: tool shell: pytest tests/test_checkout.py --count 50, 50 passed
- 2026-10-25T08:54:00Z execute: tool shell: pushed branch fix/flaky-checkout-test
- 2026-10-25T08:57:00Z execute: tool github: opening a draft pull request
