# Issue tracker: Local Markdown

Issues and specs for this project live as Markdown files in `.scratch/`.

## Conventions

- One feature per directory: `.scratch/<feature-slug>/`.
- Spec: `.scratch/<feature-slug>/spec.md`.
- Decision tickets: `.scratch/<feature-slug>/decision/<NN>-<slug>.md`, numbered from `01`, produced by `/wayfinder`.
- Implementation tickets: `.scratch/<feature-slug>/implementation/<NN>-<slug>.md`, numbered from `01`, produced by `/to-tickets`.
- Record triage state in a `Status:` line near the top of each ticket. Use the appropriate ticket vocabulary from `triage-labels.md`.
- Append comments and conversation history under a `## Comments` heading.

## Publishing to the issue tracker

Create the file under `.scratch/<feature-slug>/`, creating directories as needed. Put decision tickets in `decision/` and implementation tickets in `implementation/`.

## Fetching a ticket

Read the referenced file. If given only a number, resolve it within the relevant feature and ticket type; numbers are not globally unique.

## Wayfinding operations

- Map: `.scratch/<effort>/map.md`, containing Notes, Decisions-so-far, and Fog.
- Decision ticket: `.scratch/<effort>/decision/<NN>-<slug>.md`, with the question in the body and a `Type:` line (`research`, `prototype`, `grilling`, or `task`).
- Blocking: record `Blocked by: NN, NN` near the top. A ticket is unblocked when all listed decision tickets are resolved.
- Frontier: select open, unblocked, unclaimed decision tickets in number order.
- Claim: set the decision status to claimed and save before starting work.
- Resolve: record discussion results, invoke `/lighthouse` to produce the lighthouse document in the effort's `lighthouse/` directory, set the decision status to resolved, and append a gist plus link to Decisions-so-far in `map.md`.
