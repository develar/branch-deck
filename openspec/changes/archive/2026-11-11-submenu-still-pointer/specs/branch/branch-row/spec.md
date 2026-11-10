## ADDED Requirements

### Requirement: Hover and click: A submenu whose trigger moved from under a still pointer stays open

Content that moves the trigger of an open submenu from under a still pointer MUST close nothing: the submenu stays open
until the pointer moves; a move away from it then closes it, a move into it keeps it (`hoverClosesOnMove` of
`src/lib/pointer-moves.ts`, in the submenu roots of `src/components/ui`).

#### Scenario: A sync puts Fix Conflict… above Copy, the pointer resting on Copy
- **WHEN** "Copy ›" of a branch is open with the pointer resting on Copy, and a sync finds a conflict in the branch, so the menu leads with "Fix Conflict…" and Copy goes down from under the pointer
- **THEN** "Copy ›" stays open, and the next move away from it closes it
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should keep its open submenu when a sync moves its item from under a still pointer, until the pointer moves away`)

#### Scenario: The move into the submenu
- **WHEN** the pointer then moves into "Copy ›"
- **THEN** it stays open, and its item under the pointer is highlighted
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should keep its open submenu when a sync moves its item from under a still pointer, and a move into it`)
