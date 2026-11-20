## MODIFIED Requirements

### Requirement: Page: The page in the IDE is the page of the desktop app

The Air build (`vite build --mode air`, entry `src/air-main.ts`, into `dist-air/`) MUST run the same page as the desktop
app. It awaits `bd_bootstrap`, installs the globals of the shim, and then runs the usual main on the page of its view
(`main` is `/`, another view `/<view>`). Its chunks and its worker stay files of their own, and `index.html` is the page
of every view.

#### Scenario: The main window
- **WHEN** the IDE opens the tool window of Branch Deck on a repository
- **THEN** the page shows the branches of the repository as the desktop app does
  [@test] tests/e2e/air/air-page.spec.ts (`the tool window shows the branches of the repository after a sync of the page, each patch of the mirror a change of its rows`)

## ADDED Requirements

### Requirement: Page: The page sends initialized once it listens

The page MUST send `ui/notifications/initialized` only once the first render of its page ran its effects, after the
listens of its events. The host delivers an event to a view only after its page sent `initialized`, and holds it
until then, so brd tells an event at once, also to the page of a tab that the IDE builds for it.

#### Scenario: A Reviews tab never shown
- **WHEN** "Add Pull Request by Link…" of the palette asks the tab of Reviews, which the IDE builds for it
- **THEN** the event `focus-review-link` reaches the page once it listens, and the field of a link takes the focus
  [@test] tests/e2e/air/air-page.spec.ts (`Add Pull Request by Link… focuses the field of a Reviews tab never shown`)
  [@test] src/lib/page-start.test.ts (`runs each waiter once the page started, and a later one at once`)

### Requirement: Windows: The focus of the main window shows the tab of the main view

`bd_invoke` of `plugin:window|set_focus` of the instance of the main view MUST ask `openView {view: "main"}`, as the
focus brings the main window of the desktop app to the front. A focus of another view does nothing: the IDE shows its
place.

#### Scenario: A reveal that Activity asks
- **WHEN** the tab of Activity asks the main view to reveal a branch
- **THEN** the main view takes the focus of its window, and brd asks `openView {view: "main"}`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] tests/e2e/air/air-page.spec.ts (`a branch that the tab of Activity reveals shows in the tab of the branches: the main view comes to the front, and the IDE shows its tab with the row open`)
