# Proposal

## Why

The follow-up of `rename-wording`: a declaration renamed where no index proves the rename still showed as two units,
"totalOf — new · sumOf — removed", beside calls that now say "only renamed sumOf → totalOf, not proven". A function
whose code is the same but for its name is one declaration renamed, and the review should say so, as truthfully as it
says the calls.

## What Changes

- `ReviewUnit::renamed_from` (`RenamedFrom`: the old name and `wait`): the declaration of the old name and the one of
  the new name are one unit where the marks of the file declare the rename and the index has not proven it
  (`DeclaredRenames`), both of one kind and key but the last name, the same code but for renamed names
  (`same_but_names`); the keys of the old side read as renamed (`renamed_key`), so members pair under the new owner.
- The plan reads the renames again with the answer of the index, while the tests are asked, and tells its story again
  where the pairs changed; else only the wait changes.
- Heading "totalOf — renamed from sumOf, not proven" ("… checking"), the badge "Renamed, not proven".
- `brd review-plan`: `renamed_from`, `"schema": 38`.
