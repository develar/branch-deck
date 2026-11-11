## MODIFIED Requirements

### Requirement: The whole stack in the app: An open menu of the island follows the pull requests

An open "⋯" of the island MUST follow what the list reads in the places it opened with (`StackIslandMenu`, its
selector memoized on the stores it reads; an open menu doesn't move,
[Branch Row](../branch-row/spec.md#requirement-branch-menu-an-open-menu-doesnt-move)): the pull requests that load after
the sync change its items in their places while it is open. "Open Pull Request #12" takes the place of "Create Pull
Request of the Stack…", and "Request Review…", there from the first view on GitHub, is enabled in its place.

#### Scenario: The pull requests load after the sync
- **GIVEN** the "⋯" of the island is open
- **WHEN** the pull requests load after the sync
- **THEN** its items change in their places while it is open: "Open Pull Request #12" where "Create Pull Request of the Stack…" was, "Request Review…" enabled where it was disabled
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should keep the places of the pull request in the island's ⋯ while it is open: they load after the sync`)
