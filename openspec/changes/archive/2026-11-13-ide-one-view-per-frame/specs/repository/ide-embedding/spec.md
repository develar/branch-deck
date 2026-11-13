## ADDED Requirements

### Requirement: View: A change of the prefix moves the mirror to the session of the new prefix

A subscription again to a mirror brd holds, by `bd_invoke` of `subscribe_view` or by another view, MUST take the session
of the prefix the git config names now. A mirror of another prefix moves to it in one change that replaces every part,
its `version` of another epoch, before the answer, and no patch of the old session reaches it after.

#### Scenario: A prefix set in a terminal
- **WHEN** the prefix of the git config changes, and the main view subscribes again after its sync answered
  `prefix_changed`
- **THEN** one change of every part moves the mirror to the session of the new prefix before the answer, and the main
  view shows its branches without an end of the mirror
  [@test] crates/test-server/src/mcp_corpus_test.rs (`a_change_of_the_prefix_moves_the_mirror_to_the_session_of_the_new_prefix`)
  [@test] crates/mcp-host/src/mirror_test.rs (`every_part_replaces_the_state_with_the_view_and_its_version_last`)
  [@test] tests/e2e/air/air-page.spec.ts (`a prefix set in a terminal that a sync finds moves the main view to the session of the new prefix: the frame keeps its one view, which brd moves before it answers the subscription again`)

### Requirement: Page: The subscriptions of the page share the one view of its frame

Air holds one view for each frame, so the shim MUST keep at most one `x-air-app/subscribe` of a frame. A subscription of
the repository the frame holds joins its view and asks brd to subscribe again (`bd_invoke` of `subscribe_view`); one of
another repository subscribes once the view before it answered and was unsubscribed or ended.

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
