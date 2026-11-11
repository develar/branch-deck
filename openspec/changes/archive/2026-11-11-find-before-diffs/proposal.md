# Proposal

## Why

⌘F in a review window did nothing until the diffs of its subject had mounted: the review opened the find of the diffs
(`code.current?.openFind()`), and there were none yet. With the focus in the file tree the key was dropped; with the
focus on the frame of the review, where it is while nothing of the subject shows, the key did not reach the review at
all, as its keys were read one element below the frame. A reviewer who opens a review and presses ⌘F at once saw
nothing happen. The test of ⌘F from the file tree pressed the key as soon as the tree showed and failed under load,
when the diffs came after it.

## What Changes

- ⌘F pressed in a review before its diffs mounted is kept for its subject, and the find bar opens as the diffs mount,
  its field focused. It replaces no place asked of the diffs meanwhile.
- The keys of the review are read at its frame, so ⌘F works while the frame has the focus; the keys of the sidebar
  and the diffs (J, K, N, P, F) stay theirs and are not read from the frame.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-review`: the find opens for a ⌘F pressed before the diffs came.

## Impact

- `src/components/commit/CommitReview.tsx` (`ReviewLayout`, `ReviewRoot`); the e2e of the commit review.
