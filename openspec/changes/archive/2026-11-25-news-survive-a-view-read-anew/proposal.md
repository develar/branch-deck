# Proposal

## Why

A news of a session that came while a window read its view anew was lost: the mirror of `brd mcp` caught up the
patches that came during its `get_snapshot` without telling their news, and the main window took the view of a session
opened anew in place (a slot of a lease pool leased again) as the view of another session, whose news it never tells.
So the failure of a robot that came during a re-read was no notification of the IDE and no toast (seen on 2026-11-25 in
`air-page.spec.ts:316`, where a patch the mirror could not read made it read anew under load).

## What Changes

- The news of a session are numbered under a line (`RepositoryView::news_line`, the epoch of the open it began at); a
  session opened anew in place numbers on under the line of the open before it.
- A window (the mirror of `brd mcp`, the main window's store) tells each news past the last one it told of the line of
  its view, from a patch or from a view read anew with the patches that came meanwhile, in any epoch of the line; none
  of its first view of a line (its first view, the session of another prefix, a session that opened as another).

Capabilities: `repository/ide-embedding`, `repository/backend-protocol`, `branch/patronus`.
