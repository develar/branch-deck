# Proposal

## Why

The places of the starts and the pull request of a branch menu stand from the first view
(`2026-11-11-menu-slots-before-facts`), but two kinds of change still moved the list under a resting pointer:
"Request Review…" came in below "Open Pull Request #7" once GitHub listed it, and any fact that came while a menu was
open changed it in place: a sync that found a conflict put "Fix Conflict…" above the rest, and the open of a closed
pull request came in below "Create Pull Request…". A click aimed at an item landed on the one that moved there.

## What Changes

- "Request Review…" of a branch stands wherever git names the GitHub repository, as Create does: disabled with "Waits
  for GitHub to list the pull requests" before the listing, then enabled on the open pull request, or disabled with
  what comes first ("auth has no open pull request: Create Pull Request… asks for its review", a new blocker
  `noOpenPullRequest`, with why GitHub can't list them where it can't). The same for a whole stack that forks.
- An open menu doesn't move (`frozenMenuGroups` in `MenuItems`, every menu of the page): its items are the ones it
  opened with, each in its place as it changes; one that leaves stays there disabled until the menu closes, one that
  comes shows at the next open. An item that takes the place of another shares its slot (`MenuItem.slot`): "Open Pull
  Request #7" shows where "Create Pull Request…" was. The openings are counted by the menu roots
  (`useMenuOpenings`), so a menu reopened while its popup still fades out starts anew.
- The still-pointer guard of an open submenu (`hoverClosesOnMove`) stays for what the freeze doesn't stop: a scroll
  of a menu that moves the trigger from under a still pointer. Its tests scroll the menu instead of syncing a conflict.
- `SyncOptions::space` is the test seam it is, named `test_patronus`, documented as one, and production is pinned to
  `patronus == on_space`; a rewrite of the URL by `insteadOf` is no way around it, as `remote -v` lists the rewritten
  URL.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/branch-row`: Branch menu: an open menu doesn't move; the review group with Request Review…; the still-pointer
  guard by a scroll.
- `branch/next-step`: Facts without an answer: Request Review… among the places of the menu.
- `branch/pull-requests`: Request Review: its item stands on GitHub, disabled without an open pull request.
- `branch/whole-stack`: The whole stack in the app: an open menu of the island follows the pull requests in its places.

## Impact

- `crates/domain-types/src/actions.rs` (`ActionBlocker::NoOpenPullRequest`), `crates/domain-policy/src/actions.rs`,
  the action catalog; `crates/sync-core/src/{sync,repo_facts}.rs`, `crates/test-server/src/session_routes.rs`.
- `src/lib/menu.ts`, `src/components/shared/MenuItems.tsx`, `src/components/shared/TableContextMenu.tsx`,
  `src/hooks/useMenuOpenings.ts`, `src/components/ui/{context-menu,dropdown-menu}.tsx`,
  `src/presenters/{actions,review}.ts`, `src/hooks/useBranchContextActions.ts`, `src/hooks/useWholeStackActions.ts`.
