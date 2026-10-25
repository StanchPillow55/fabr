---
schema: 2
id: 2026-10-24-order-replacement-nozzle
source: null
title: "order a spare 0.4 mm nozzle"
due: 2026-11-05
state: defined
step: approve
waiting: true
created_at: 2026-10-24T20:30:00Z
updated_at: 2026-10-24T20:37:30Z
requests: {delegate: false, cancel: false, redelegate: false}
question: null
answers: []
grants:
  requested: [address.line_1, address.zip, name.first]
  granted: []
plan_meta:
  reversibility: irreversible
  tools: [browser]
  assumptions:
    - rule_id: ship-to-home
      text: "Ship to the home address on file, standard shipping"
assertions:
  - id: a1
    text: "The shipping form is filled from granted fields only"
  - id: a2
    text: "The cart holds one 0.4 mm nozzle for the Prusa MK4 and nothing else"
  - id: a3
    text: "The run stops on the payment page and no payment field is filled"
approval: {status: pending}
verdict: null
cost_usd: 0.09
links: {redelegated_from: null, redelegated_to: null}
---
## Context
The nozzle has started to under-extrude on the first layer. A spare is cheap, so order one before it fails mid-print.

### Gathered
- Folder knowledge (<SAMPLE>/home/projects/3d-printing): Prusa MK4, 0.4 mm nozzle, usual shop is shop.example-filament.test, orders ship to home, payment is always done by hand.
- Preferences: ship_to home, max_order_usd 80.
- Past task 2026-08-14-order-resin (succeeded): same shop, shipped to home with standard shipping.
## Spec
Order one 0.4 mm nozzle for the Prusa MK4 from shop.example-filament.test. Ship to the home address on file with standard shipping (rule: ship to home). Fill the shipping form from the granted name and address fields. Stop at the payment step and hand it to the human. The preference cap is 80 USD.
## Success
A spare nozzle is in the cart, the shipping form is filled in, and I only have to pay.
## Plan
1. Open shop.example-filament.test and search for a 0.4 mm nozzle for the Prusa MK4.
2. Add exactly one nozzle to the cart.
3. Go to checkout and choose standard shipping.
4. Fill name.first, address.line_1 and address.zip from the granted data bank fields.
5. Stop at the payment page. Payment is handed to the human; the agent never fills it.
## Log
- 2026-10-24T20:30:30Z gather: delegate accepted, folder lock taken
- 2026-10-24T20:30:50Z gather: context bundle written
- 2026-10-24T20:36:40Z define: spec and 3 assertions written, 3 grants requested
- 2026-10-24T20:37:10Z plan: plan written, irreversible, tools: browser
- 2026-10-24T20:37:30Z approve: approval pending, message sent
