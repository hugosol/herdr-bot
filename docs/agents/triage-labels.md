# Triage Labels

This project uses two distinct ticket systems with separate status vocabularies.
For local Markdown tickets, record the applicable value in the `Status:` line.

## Decision tickets (wayfinder)

Planning artifacts in `.scratch/<feature>/decision/`. Each asks what to decide; its lighthouse document records the decision.

| Status | Meaning |
|--------|---------|
| `open` | Not yet claimed by an agent |
| `claimed` | Agent is actively working on this decision |
| `resolved` | Decision made and recorded; no implementation code has been written as part of this ticket |

Decision tickets are planning work. A resolved decision means the decision is locked, not that implementation exists.

## Implementation tickets (to-tickets)

Tracer-bullet vertical slices in `.scratch/<feature>/implementation/`, delivering working, testable behaviour.

| Status | Meaning |
|--------|---------|
| `ready-for-agent` | Fully specified, ready for an AFK agent to implement |
| `ready-for-human` | Requires human implementation |
| `in-progress` | Agent is actively implementing |
| `closed` | Code implemented, tested, and merged |

## Shared (both ticket types)

| Status | Meaning |
|--------|---------|
| `needs-triage` | Maintainer needs to evaluate this item |
| `needs-info` | Waiting on reporter for more information |
| `wontfix` | Will not be actioned |

When a skill names a status role, use the corresponding string from the appropriate table. Keep decision and implementation status vocabularies separate.
