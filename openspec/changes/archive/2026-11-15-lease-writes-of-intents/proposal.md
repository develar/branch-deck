# The intents and `brd` write the files of a lease only under the lease they began under

## Why

The review of e74634614 found the class open past the session's run: Update Project, its undo and the dismissal of what
landed wrote the landed state at any time (`LeaseGuard::any`), also when asked on the view of the lease before or when
the slot was leased again while they ran; `brd -C <slot>` from a directory outside the slot, which the process check of
a return does not see, wrote with `any` too; a write that passed the check of its guard could land after both forgets;
and the scan did not see a path of a file of the lease built at run time (`join("branch-deck").join("landed.json")`).

## What Changes

- Update Project, the undo of a rewrite and the dismissal of what landed name the epoch of the view they were asked on
  (`Intent::UpdateProject::epoch`, `Intent::DismissLanded::epoch`, `RewriteIntent::Undo::epoch`, as `TableWitness::epoch`
  of a rewrite); one asked on a view of another epoch, or on a slot that holds another lease than its session opened
  under, is refused (`plan_changed`, `operations::witness::under_its_lease`, `Sessions::lease_guard`), and each writes
  the files of the lease under the guard of the lease of the view.
- `brd` reads the lease of the slot as a command starts (`RepoContext::lease`, `lease_pool::epoch::lease_guard`) and its
  sync, update and undo write under it; `any` stays only for a worktree that is no slot.
- The check of a guard and the write hold the lock of the lease (`lease.lock`), which the forget takes before it
  removes.
- `LeasePaths` builds the directory of the lease and every path in it; the scan fails a name of a file of the lease
  beside `branch-deck` anywhere else, however the path is put together.
- Capabilities `cli/lease` and `repository/onboarding`.
