# UX Writing: Toasts, Errors and Labels

**Status:** Active  
**Updated:** 2026-10-18  
**Tags:** [ui, wording, toasts, actions]

## Summary

One vocabulary for the whole app. A toast says what happened, to what, in sentence case; its description is the detail;
an error says what could not be done and why; a toast about an object opens it. Buttons, menu items and toast actions
are Title Case; everything that reads as a sentence is sentence case. [Activity](../../spec/repository/activity.spec.md)
records the toasts in their words, so a toast and its entry take their text from one function. An object's actions
are one list, under one name each, wherever the object is.

## Context

Before this guide the toasts spoke three dialects: "Success / Branch X pushed successfully", "Pushed oauth" and
"`feature-auth`: Changes Amended"; errors were "Failed to …", "… Failed", "Can't …" and "Error"; some error toasts were
sticky and logged, others vanished after 5 s. The user read a toast, then could not find the thing it was about.

## Toasts

- **The title says what happened**, in the past tense, with its object: "Pushed feature-ui", "Amended 3 files into
  feature-auth", "Updated master: rebased 2 commits onto origin/master", "Safe Push started", "auth merged into master".
- **Never "Success" or "Error" as a title**: the icon already says it.
- **The description is the detail**: what the push did ("1 commit · created the remote branch", "1 commit · dropped 2
  remote commits"), what a robot tests ("air-session-runtime → master · profile Large Scale Change"), the amended commit.
  It never repeats the title.
- **No "branch: Title Case" prefixes** ("feature-auth: Changes Amended"): the branch is part of the sentence ("Unapplied
  feature-auth").
- **Name the object the user knows**: the virtual branch, never its ref (`develar/virtual/air-session-runtime`).
- **A toast about an object opens it** with its action:
  - a Patronus robot (started, restarted, canceling, merged): "Open in Patronus" (`robot.url`);
  - a pushed branch: "Open Pull Request", or "Create Pull Request…" without one (GitHub; the dialog asks for its text
    and reviewers);
  - a created pull request: "Open Pull Request";
  - a Space merge request: "Open Merge Request";
  - the installed Agent CLI: "Copy Path".
  A toast with an action stays 10 s (`ACTION_TOAST_TIMEOUT_MS`). A toast with a description and an action, or with two
  actions (the merged robot: Fetch, then "Open in Patronus", `data.secondaryAction` of `src/components/ui/toast.tsx`),
  has its actions below the text, the primary one first: beside it, a button squeezes the title into a narrow column.
  A toast of a title alone keeps its action beside it.
- **A precondition is `info`, not an error**: "No commits to unapply", "No changes to amend", "Nothing to change",
  "master is already up to date with origin/master". Nothing failed; nothing is logged.
- **No toast for feedback in place**: a copy button says "Copied" in place (`CopyButton`, `useCopyToClipboard`), the
  "Copy Prompt for Agent" button too; a menu item copies without a toast; an amend conflict opens its viewer below the
  row and says nothing else.

## Errors

- **Every error goes through `notifyError(title, error)`** (`src/lib/errors.ts`): the toast is sticky, the error is
  logged to the backend log and recorded in Activity. Never a hand-made `toast.add({ type: "error" })`;
  `withRowProcessing` takes an `errorTitle` and calls `notifyError` itself.
- **The title is "Cannot <verb> <object>"**: "Cannot push feature-ui", "Cannot start the Safe Push", "Cannot copy to the
  clipboard". One failure verb: not "Failed to …", "… Failed", "Can't …", "Couldn't …".
- **The description is the reason**, the `message` of the error (`getErrorDetails`), never prefixed with "Error:".
- The same words in a tooltip or an inline error: "Cannot check the update: <reason>".
- **Typing a field refuses says what is wrong**, under the field, without "Cannot …": nothing was attempted ("A prefix
  cannot contain spaces, …", "acme/app/x is not a GitHub repository: use owner/name").

## Help

- **A help popover is read in five seconds**: its title is the name of what it explains, then what it is and what the
  app does with it, in 1–3 short sentences; the keys last. No "automatically" (everything the app does is), no
  benefit talk ("to keep your repository clean"), no muted footnotes, no weight on a word of the content.
- **True to the code**: a number names its constant in a comment beside the text (`ARCHIVE_RETENTION_DAYS`), a key is
  shown only where its handler works.
- **Help repeats no form**: what a form does belongs in its description ("Amends the uncommitted changes to the last
  commit of X, keeping its message and author."), not in a popover beside it.

## Words and punctuation

- **Title Case** for buttons, menu items, card titles, tabs and toast actions: "Create Pull Request", "Open in
  Patronus", "Copy Path", "Install and Restart".
- **Sentence case** for dialog titles, toast titles, descriptions, tooltips, placeholders and help heads, and for the
  label of a switch and the heading of a settings section ("Review requests", "Count failed robots and conflicts",
  "Dock icon"): a switch states a setting, it is not a command. A switch that mirrors a menu item keeps the item's
  Title Case ("Auto-sync on Focus").
- **"…"** (the ellipsis character, never "...") for progress ("Pushing…") and on every command that asks for input
  before it acts ("Unapply…", "Create Pull Request…", "Safe Push With Profile…"). An action that acts at once, or asks
  only to confirm what it has to say first, has none: "Safe Push" starts, or opens a popover only when it cancels a
  running robot, lands more than its row or runs another profile (as macOS: the ellipsis means more input, not a
  confirmation).
- **"canceled"**, "canceling" (US spelling), "Branch Deck".
- **No exclamation marks**.
- **"·" is the only separator** between parts of a line ("3 commits · created the remote branch"), also between the
  items of a line that come from several places (the island header); never "•", "|" or " - ".
- **A list of names in a sentence** names up to four ("auth, session, login and oauth") and from five on the first
  three and the count ("auth, login, oauth and 2 more", `joinNames`): never "and 1 more", where one name would do.
  Every cut list follows the same rule, a dialog's, a prompt's for an agent ("… and 280 more files"), a hover's
  ("Failed: Fast Tests, Smoke, Compilation and Lint", four of four), a chip row's ("+2 branches"), the dropped
  commits of a push ("and 2 more" past ten) and `bd`'s output (`files_omitted`, " (2 more: bd check)" after the
  dangling references it describes): a list one longer than its limit is given whole, the count of the rest is never 1
  (`cutList`; the backend's lists `sync_utils::list_cut::kept`). An item with no name to stand there is said as such,
  never counted alone ("tests and a check without a name", a GitHub check without one), and a text cut to a budget
  sends its one line past it rather than "1 more lines" (the input of the branch naming, "[… 2 more lines omitted]").
  The one exception is a list whose rows a poll keeps
  in place: Your Turn shows three rows, then "Show 1 More", since a fourth row would come and go with the polls.

## Actions

The same object (a branch, a Patronus robot, a pull request) appears in a menu, a popover, a footer and a header list.
Built by hand in each place, the actions drifted apart: the menu named a restart one way, the popover another; the branch menu showed `O` and `M`, which worked only in the preview or with a selection; the robots
indicator could cancel a robot but not restart it.

- **One builder per object**: the actions of an object come from one function returning `MenuItem`s, each with a
  stable `id` (`getRobotActions` in `src/lib/patronus-actions.ts`, `getBranchContextMenuItems` in
  `src/hooks/useBranchContextActions.ts`). A menu renders the list (`MenuItems`); a popover, a footer or an icon row
  renders the same items as buttons, picking them by `id`. A surface never builds its own copy of an action.
- **Object actions stay in every form**: a compact form (a row, a popover) and an expanded one (a preview, a window)
  offer the same actions of the object; only the view modes (Review, Files) change with the form.
- **A key hint only where the key works**: an item shows its key (`kbds`, `<Kbd>`) only on a surface whose handler
  has the key; a key shown in the menu works on the focused row too.
- **What a text names opens from it**: a hover, a notice or a toast that names an object or a move offers it right
  there, from the builder of the object: an issue key is a link to the tracker, "Safe Push" of a robot opens it in
  Patronus, "#12" its pull request, "Blocked by auth" offers "Show auth", "…or Restart Safe Push" the restart. A
  tooltip holds no link (the local `Tooltip` is not hoverable): a hover that names one is a popover
  (`LandingNodeHover`). The node of a branch once said "Safe Push failed on IJAI-4184 as it is: fix it or restart it"
  in a tooltip, nothing of it clickable. A text inside a button (the toggle of an island header) is no place for a
  link: the button that opens its object stands beside it.
- **Disabled with a reason, not hidden**: an action that does not apply now stays in the list, disabled, and says why
  on hover (`description`); it disappears only where it never applies (no pull request, no robot).
- **A step the app can do is part of the action**: a push an action needs, a wait for the sync, a fetch run inside the
  action, which keeps its plain name ("Create Pull Request…", "Safe Push", "Restart Safe Push", "Request
  Review…"); its progress shows in the busy button ("Pushing 45%"). Never disable an action with "push first" or "sync
  and try again", and never narrate the step in a note. Only a consequence a person decides is said, and asked: a push
  that drops someone's remote commits, one that dismisses approvals, a start that cancels a running robot. The row of
  an unpushed branch once offered a disabled "Create Pull Request…" ("Push first: a pull request needs the remote
  branch") beside its own push button.
- **Never offer an action whose failure is known**: disable it with the reason; a wait is a quiet step that opens its
  object. "Safe Push" of a branch whose code owner is still asked is disabled with "Waits for the approval of IJPL
  Code Platform: Ilya Korennoy is requested on #1078", and the row shows the wait, not the doomed action
  ([Next Step](../../spec/branch/next-step.spec.md)). One model of an object decides its next step
  (`getBranchNextStep`), every surface renders it.
- **A state as a label**: the one exception to "a button is a verb". A quiet step names the state it waits in ("Awaiting
  Ilya Korennoy", "Changes Requested", "Draft"), muted (`text-muted-foreground`, `destructive` for changes requested), in
  Title Case as a button; its hover says the whole state and the action it takes ("Open Pull Request #1078" with its
  key), and the chip of the object keeps its own state, so the row doesn't say it twice.
- **The review keys of every object**: D reviews it in its window ("Review Changes": the commit, the branch, the pull
  request; one action, no "Open in Window"), from the row, its menu and its preview (D, the footer button, Enter on a
  file of the tree, which closes the preview); Space previews it, Escape closes the window. On a row D follows the
  pointer as Space does; the keys that act (P, S, O, M) need the focused row. Inside a review J/K are the files, N/P
  the changes, ⌥↑/⌥↓ the commits.
- **No verb on a verb**: an action named by a verb is its own button ("Safe Push", never "Start Safe Push"; "Dry
  Run", never "Run Dry Run"); the toast says what happened ("Safe Push started").
- **One name per action**: "Open Pull Request" (never "Open on GitHub"), "Copy Pull Request Link", "Open in Patronus",
  "{Action} {mode}" for a robot ("Restart Safe Push", also when it pushes first; "Cancel Dry Run"; a bare "Cancel"
  reads like closing the popover), "Safe Push" for the start. Advice in a sentence names the action as its button
  does ("Restart Safe Push to test the current branch.").

## How to

- A success or info toast: `showToast({ title, description, action })` from `src/lib/toast-message.ts`
  (`type: "info"` for a precondition), or `toast.add` from `@/components/ui/toast` for a custom one (a loading toast, a
  custom body).
- A history rewrite with an Undo: `showUndoToast(title, undo)` from `src/lib/command.ts`.
- An inline form's action: `withRowProcessing(key, fn, { success, errorTitle })` or `withRowProgress` when the action
  toasts itself (`src/stores/inline-row-action.ts`).
- A toast that Activity records takes its words from the function the entry uses: `describePush`
  (`src/lib/push-summary.ts`), `describeRobot` (`src/lib/patronus.ts`, through `announceRobot` in
  `src/lib/patronus-actions.ts`), `appliedUpdateToast` (`src/lib/landed.ts`).

## References

- [UI Theme](../design/ui-theme.md): colours, states, the copy button.
- [Activity](../../spec/repository/activity.spec.md): the log in the words of the toasts.
- [Branch row](../../spec/branch/branch-row.spec.md), [Patronus](../../spec/branch/patronus.spec.md): the branch menu, the
  row keys and the robot actions built this way.
