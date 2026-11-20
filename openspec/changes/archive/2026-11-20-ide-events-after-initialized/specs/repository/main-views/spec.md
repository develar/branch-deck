## MODIFIED Requirements

### Requirement: In the IDE: A switch to the view of another tab shows that tab

In the IDE, a switch to a view that another tab shows SHALL ask `brd mcp` for that tab, with an event for its page:
`reveal-branch` reveals a branch in the tab of the branches, `focus-review-link` focuses the field of a link of
Reviews, also in a tab that the IDE builds for the switch. A switch to the own view of the page shows it in the page.
A reveal that Activity or a review asks of the main view shows the tab of the branches.

#### Scenario: Reviews of the palette of the main view
- **WHEN** the user picks Reviews in the palette of the main view
- **THEN** the IDE shows the tab of Reviews, and the main view keeps the branches
  [@test] tests/e2e/air/air-page.spec.ts (`Branches, Reviews and Landing are tabs of the tool window: the main view has no switch of the views, a view of its palette shows its tab, which reads Reviews while it shows, and a branch of Reviews shows in the tab of the branches`)
  [@test] src/stores/main-view.test.ts (`asks brd for the tab of a view of another tab, with the event for its page, and shows its own view itself`)

#### Scenario: A branch of Reviews
- **WHEN** the user clicks an own pull request of a branch in the tab of Reviews
- **THEN** the IDE shows the tab of the branches, whose page reveals the branch as Go to of the palette does
  [@test] tests/e2e/air/air-page.spec.ts (`Branches, Reviews and Landing are tabs of the tool window: the main view has no switch of the views, a view of its palette shows its tab, which reads Reviews while it shows, and a branch of Reviews shows in the tab of the branches`)
  [@test] src/stores/main-view.test.ts (`reveals a branch in the tab of the branches, whose page reveals it, and asks no reveal of this page`)

#### Scenario: Add Pull Request by Link… to a tab never shown
- **WHEN** the user picks "Add Pull Request by Link…" in the palette before the tab of Reviews ever showed
- **THEN** the IDE shows the tab of Reviews, and its field of a link has the focus
  [@test] tests/e2e/air/air-page.spec.ts (`Add Pull Request by Link… focuses the field of a Reviews tab never shown`)

#### Scenario: A branch of Activity
- **WHEN** the user clicks a branch in an entry of the tab of Activity
- **THEN** the IDE shows the tab of the branches, whose page opens the row of the branch
  [@test] tests/e2e/air/air-page.spec.ts (`a branch that the tab of Activity reveals shows in the tab of the branches: the main view comes to the front, and the IDE shows its tab with the row open`)
