## MODIFIED Requirements

### Requirement: Page: The subscriptions of the page share the one view of its frame

Air holds one view for each frame, so the shim MUST keep at most one `x-air-app/subscribe` of a frame. A subscription of
the repository the frame holds joins its view and asks brd to subscribe again (`bd_invoke` of `subscribe_view`), or,
when brd refused the `subscribe` of that view, subscribes the frame anew: every subscription gets an answer of brd asked
after it. One of another repository subscribes once the view before it answered and was unsubscribed or ended.

#### Scenario: A switch of the repository before the first answer
- **WHEN** the main view picks another root before its first subscription answered
- **THEN** the picked root subscribes once that subscription answered and the page unsubscribed it, and Air refuses
  nothing
  [@test] src/platform/air/shim.test.ts (`A switch of the repository before the first answer: the next repository subscribes once the view of the one before answered and was unsubscribed, so the frame holds one view`)
  [@test] tests/e2e/air/air-page.spec.ts (`a pick of another root before the first subscription of the main view answered subscribes to it once that subscription answered and ended: the frame holds one view`)

#### Scenario: A subscription again
- **WHEN** the main view subscribes again to its repository after `prefix_changed`, holding the subscription it replaces
- **THEN** both share the view of the frame, the new one answers after brd moved the mirror, and the one it replaces
  leaves without an unsubscribe of the frame
  [@test] src/platform/air/shim.test.ts (`A subscription again of the repository the frame holds shares its view: brd subscribes again, moving the mirror to the session of another prefix before it answers, and the subscription it replaces leaves without an unsubscribe of the frame`)

#### Scenario: A subscription while the view of the frame opens
- **GIVEN** the main view subscribes again to a repository without a prefix (a focus after the refusal), and the user
  saves the prefix of the welcome card while that view opens
- **WHEN** brd refuses the view, as it read the config before the prefix
- **THEN** the subscription after Save subscribes the frame anew, and brd answers it with the view of the repository
  [@test] src/platform/air/shim.test.ts (`A subscription asked while the view of the frame opens gets an answer of brd asked after it: the refusal of that view, asked before a prefix was saved, makes it subscribe anew`)

#### Scenario: The link of the view breaks while a subscription joined it
- **GIVEN** a subscription joined the view of the frame while it opened
- **WHEN** the `subscribe` of that view fails without an answer of brd (the link broke)
- **THEN** the joined subscription fails with it and subscribes nothing anew
  [@test] src/platform/air/shim.test.ts (`A subscription that joined a view whose link broke fails with it: only a refusal of brd is an answer of the config as it was`)
