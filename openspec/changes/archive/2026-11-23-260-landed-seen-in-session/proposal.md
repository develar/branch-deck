# Proposal

## Why

The Landed island ("Landed in origin/master") hid what the user saw by a rule of the page (#260): each window kept
when it saw the island (`seenAt`, only while it had focus) and, after a sync it heard end, asked `hide_landed` once it
was seen 10 s ago. Another window and the IDE never knew it was seen, so two surfaces of one repository disagreed, and a
rule of the domain lived in TypeScript.

## What Changes

- The page reports a fact only: a window with the focus shows the island up to `landedAt` on the view of an epoch
  (`landed_seen`, which replaces `hide_landed`; the session takes the window from the call).
- The rule is `domain_policy::landed_island`: a sighting keeps its time for the same `landedAt`, and the first
  successful sync at least 10 s after a sighting by a window that has the focus as it ends hides up to the newest
  `landedAt` so seen.
- The session keeps the sightings of its windows, applies the rule at the end of its successful sync and writes
  `landedHiddenUntil` itself under the lease it opened under; every window and the IDE hear the change. A session that
  knew no lease of its slot keeps the sightings across its reopen under the lease read (what `lease_read_again` gave the
  page before); a session of another known lease drops them with its report.
- The page loses `seenAt`, `hideAfterSync` and `ISLAND_MIN_SEEN_MS`; `useLandedIsland` still reads `landedHiddenUntil`.

Capabilities: `repository/landed-commits`.
