# Proposal

## Why

`brd review-plan` says since schema 39 which `removed` unit and which `added` unit are one declaration moved whole to
another file (`moved_to`, `moved_from`), but the review recipe of the skill does not name them: an agent that follows
it reads both units whole and, by "never skip a removal", reports the removal of code that only moved.

## What Changes

- The recipe of the skill says that a `removed` unit's `moved_to` and an `added` one's `moved_from` name the other
  place of the same code: one move between files, read once at its new place, its removal no loss.
- Its test checks that both names are printed by `brd review-plan`, by its schema, and named by the recipe.

Capabilities: `cli/agent-cli`.
