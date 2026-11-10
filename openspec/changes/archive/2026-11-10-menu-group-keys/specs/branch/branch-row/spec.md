## ADDED Requirements

### Requirement: Branch menu: An open submenu stays open when the menu changes above it

A change of the menu while it shows (a sync that puts a group above another) MUST keep each group that stays, with
its open submenu and the item under the pointer: a group is known by its item that opens a submenu, else by its first
item, never by its place.

#### Scenario: A sync puts Fix Conflict… above an open Copy
- **WHEN** "Copy ›" of a branch is open with the pointer on its items, and a sync finds a conflict in the branch
- **THEN** the menu leads with "Fix Conflict…", "Copy ›" stays open, and Copy Branch Name copies the name
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should keep its open submenu under the pointer when a sync puts a group above it`)
