# Domain Docs

## Layout

This is a single-context project:

- `GLOSSARY.md` at the project root: domain vocabulary.
- `docs/adr/`: architectural decision records.

These are locations for real domain documentation, not files to scaffold empty.

## Before exploring

Read the root `GLOSSARY.md` and ADRs in `docs/adr/` relevant to the area being explored.

If a root `GLOSSARY-MAP.md` exists in the future, follow its pointers to the glossaries relevant to the topic, and also check `src/<context>/docs/adr/` for context-specific decisions.

If these documents are absent, proceed silently. `/domain-modeling` creates them lazily when terms or decisions are resolved.

## Use the glossary's vocabulary

Use the defined domain terms in issue titles, refactor proposals, hypotheses, and test names, rather than synonyms the glossary avoids.

If a needed concept is missing, reconsider whether the project uses it; note genuine vocabulary gaps for `/domain-modeling`.

## Flag ADR conflicts

Explicitly surface proposals that contradict an existing ADR, naming the ADR and explaining why its decision is worth reopening.
