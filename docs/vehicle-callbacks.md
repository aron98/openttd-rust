# Vehicle callback boundary

`ottd simulate-callbacks INPUT.json` runs one explicitly selected callback on a
JSON state. It does not move vehicles or advance a saved world. The request and
response use schema 1:

```json
{"schema_version":1,"callback":{"kind":"vehicle","operation":{"kind":"economy_year"},"state":{}}}
```

The state placeholder is the complete `VehicleCallbacks` structure. Native
examples are in `reference/callbacks.json`; each case has `before`, `operation`,
and `after`. Wrap `before` as `callback.state` to invoke the CLI. Calendar aging
uses `{"kind":"calendar_day","date_fract":0}` and visits pool IDs congruent to
that fraction modulo 74. The fraction is the original calendar callback phase,
not a post-tick clock snapshot.

Supported original OpenTTD 15.3 bodies:

- `RunVehicleCalendarDayProc` and the four vehicle calendar-day handlers, with
  advice disabled: subtype gates, age saturation, reliability-decay anniversaries.
- Economy `YEAR` / `VEHICLE` callback with human companies and income advice
  disabled: primary-vehicle profit rollover and complete group profit rebuilding.

The boundary requires every company's all/default group bucket for all four
vehicle types, even empty types. Ordinary group IDs are globally unique. Vehicle
IDs are strictly ascending and below native pool end `0xFF000`. Profit uses eight
fractional bits; each display profit is shifted before summation, with native
saturating `Money` additions in pool order. Eligible statistics use age strictly
greater than 730 days. Vehicle count caches and random state remain unchanged.

News/advice, AI queues, daily operating callbacks, breakdowns, servicing, orders,
NewGRF callbacks, running costs, cargo, and motion are not implemented by this
boundary. Enabling either vehicle warning setting is rejected. Unknown JSON
fields and operations are rejected rather than ignored.

`bash scripts/setup-snapshot-reference.sh` builds the pinned instrumented C++
reference. `bash scripts/check-callbacks.sh` runs a fresh dedicated probe process,
compares the original callback state through the Rust API and CLI, checks repeated
output and rejected requests, and verifies that the native corpus reproduces.
The oracle resets its fixture pools using native pool cleanup, creates native
objects, retains the original registered yearly timer, and invokes that original
callback via `TimerManager`; it never copies callback bodies into oracle logic.
