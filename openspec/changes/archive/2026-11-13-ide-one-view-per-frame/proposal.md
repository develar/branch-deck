# Proposal

## Why

Air holds one view for each page connection, so for each frame of a view of Branch Deck: a `viewOpen` while another
runs gets `-32011 viewOpen` at once, and one while the connection holds a view waits up to 1 s for its end and then gets
the same. The main view subscribes again before it leaves in two cases: a switch of the repository before its first
subscription answered (`openSession`, which leaves a subscription without its answer), and the subscription again after
`prefix_changed` (`resubscribeSession`), which holds the old one until the new one answers. Both failed in the IDE. And
`brd mcp` answered a subscription to a mirror it held from the state of that mirror, so a change of the prefix reached a
view of the IDE only once its session ended. The fake IDE of the E2E suite gave each subscription a channel of its own
and kept no rule of the page connection, so the suite reached neither failure, and each test ended with a restart of its
mirror: the fixture removed the repository while the page still showed it.

## What Changes

- Page (`src/platform/air/shim.ts`): the subscriptions of the page share the one view of the frame. A subscription of
  the repository the frame holds joins it and asks `brd mcp` to subscribe again (`bd_invoke` of `subscribe_view`); a
  subscription of another repository subscribes once the view before it answered and was unsubscribed or ended. A change
  to a version of another epoch is no patch of the view the page holds: the page takes the state as a snapshot.
- `brd mcp` (`crates/mcp-host`): a subscription again to a mirror it holds, by `bd_invoke` of `subscribe_view` or by
  another view, follows the prefix the git config names now: a mirror of another prefix moves to the session of the new
  one in one change that replaces every part, before the answer, and takes the demand a view set last.
- The fake IDE (`tests/e2e/air-host/`): one channel for each mirror, its state by the table rule and one `serverSeq` of
  the host for each change; the one view of a frame and the calls that wait (24 for a view, 36 for the app) as Air
  refuses them, each refusal failing its test; the IDE closes before the repositories of a test go.
- Capability `repository/ide-embedding`: the requirements "Page: The subscriptions of the page share the one view of its
  frame" and "View: A change of the prefix moves the mirror to the session of the new prefix"; the risk of one view for
  each connection is resolved.
