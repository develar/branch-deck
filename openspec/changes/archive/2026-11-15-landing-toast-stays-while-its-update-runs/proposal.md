# The toast of a push or a merge into the target stays while its update runs

## Why

The toast of a push or a merge into the target follows the update after it in place ("Rebuilding on the new master…",
then what the update did). It had the 5 s of a toast without an action, so an update that took longer (under load, a
sync that waited) closed it, and the end of the update opened a second toast beside the first as it slid out:
github-stacks.spec.ts:63 found two toasts "Merged #7 of login into master · 2 commits" at a load of 160 (a strict mode
violation), and a user saw the toast go and another come for one landing.

## What Changes

- While the update after the push or the merge runs, its toast stays (`timeout: 0` of `followPush`); its end takes the
  place of "Rebuilding on the new master…" and has the whole time of a toast.
- Capability `branch/landing-routes`.
