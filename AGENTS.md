# NetMap

## Judgment and scope

Use common sense to achieve the user's goal with the least unnecessary machinery.
Rules and plans serve correctness, responsiveness and usability; apply them by
their purpose. Resolve routine choices within the authorization already given.
Ask when an unresolved choice materially changes scope, risk or authorization.
Keep earlier requirements in scope unless the user cancels or replaces them.

Write elegant, ordinary Rust: clear intent, one authority, one implementation.
Prefer established Windows behavior and the simplest correct model. Every new
layer, dependency, state and branch must answer a concrete requirement, because
each becomes permanent maintenance work.

NetMap ports TinyTorrent2's map to native Rust with whole-PC connection facts.
Keep ../TinyTorrent2 untouched as the reference.

## Mandatory reading

[The software specification](docs/plans/rust-map.md) owns product behavior, stage
scope and acceptance criteria. Read its Target before implementing, then the
affected behavior sections. [CONTEXT.md](CONTEXT.md) owns domain vocabulary.
Neither a specification nor a guideline is proof that code or platform checks
already exist.

Mandatory project guidelines live in docs/agents/. Read the following before
changing the corresponding paths:

- **Code and review:** [Code, names and comments](docs/agents/development.md#code-names-and-comments)
  and [Change and verification](docs/agents/development.md#change-and-verification)
  before implementation, refactoring or code review.
- **Modules, ownership and dependencies:** [Architecture](docs/agents/architecture.md#ownership),
  including its project tree and dependencies, before adding or moving code or
  changing an interface.
- **Updates, workers, persistence or shutdown:** [State and update flow](docs/agents/architecture.md#state-and-update-flow)
  and [Background work and shutdown](docs/agents/architecture.md#background-work-and-shutdown)
  before changing event ordering, native work, result delivery or lifetime.
- **Native UI or hosting:** [Native presentation](docs/agents/architecture.md#native-presentation)
  and the specification's affected window/hosting behavior before changing
  controls, layout, input, DPI, accessibility or painting.
- **Settings:** [Settings specification](docs/plans/settings.md) before changing
  its content tree, search, resets, draft/apply behavior or porting the Goz shell.
- **Collection or geolocation:** [Native collection](docs/agents/architecture.md#native-collection)
  and the specification's [connection and location facts](docs/plans/rust-map.md#connection-and-location-facts)
  before changing identity, rates, retention or datasets. For origin changes,
  also read [Origin transitions](docs/agents/architecture.md#origin-transitions).
- **Map port or assets:** [Porting](docs/agents/porting.md) before translating
  source, copying assets or changing map math, navigation, effects or caches.
- **Stage work or acceptance:** the affected stage in [Development and verification](docs/agents/development.md)
  and its shared validation before implementing or accepting a deliverable.

## Documentation authority

Keep each rule at its owner: software behavior in the specifications, domain
definitions in CONTEXT.md, architecture and working rules in docs/agents/.
This file routes agents to those owners. Update affected links when moving
material; separate intended contracts from observed implementation evidence.
