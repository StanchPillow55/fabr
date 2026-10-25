---
schema: 1
id: 2026-10-24-order-filament
title: "order 2 kg of black PLA"
due: 2026-10-28
state: task
step: define
waiting: true
created_at: 2026-10-24T18:02:11Z
updated_at: 2026-10-24T18:05:30Z
requests: {delegate: false, cancel: false, redelegate: false}
question:
  id: q2
  kind: choice
  text: "Ship to your home address again?"
  options: [once, always, "no"]
  prior:
    text: "Ship to home address, standard shipping"
    source_task: /home/me/3d/.fabric-space/tasks/2026-08-14-order-resin.md
    last_confirmed: 2026-08-14T10:20:00Z
  fields: null
  asked_at: 2026-10-24T18:05:30Z
  answer: null
answers:
  - id: q1
    kind: open
    text: "Which shop, and is a 1 kg spool acceptable if 2 kg is out?"
    options: null
    prior: null
    fields: null
    asked_at: 2026-10-24T18:03:40Z
    answer:
      value: "shop.example-filament.test, 1 kg spools are fine"
      replacement: null
      granted: null
      by: gui
      at: 2026-10-24T18:05:02Z
grants:
  requested: []
  granted: []
plan_meta: null
assertions: []
approval: null
verdict: null
cost_usd: 0.05
links: {redelegated_from: null, redelegated_to: null}
---
## Context
Same brand as last time if it is in stock.

### Gathered
- Folder knowledge (/home/me/3d): Prusa MK4, 1.75 mm PLA, usual shop is shop.example-filament.test, orders ship to home, payment is always done by hand.
- Preferences: ship_to home, max_order_usd 80.
- Past task 2026-08-14-order-resin (succeeded): grey resin from the same shop, shipped to home with standard shipping.
## Spec
## Success
The black PLA is in the cart, the shipping form is filled in, and I only have to pay.
## Plan
## Log
- 2026-10-24T18:03:00Z gather: delegate accepted, folder lock taken
- 2026-10-24T18:03:20Z gather: context bundle written
- 2026-10-24T18:03:40Z define: asked q1 (open)
- 2026-10-24T18:05:02Z define: q1 answered by gui
- 2026-10-24T18:05:30Z define: asked q2 (choice)
